//! Runs the effects returned by the domain. Only publishing to the channel
//! can fail the operation; a message that cannot be delivered (for example,
//! the user blocked the bot) is logged and the rest still runs.

use anyhow::Context;
use teloxide::prelude::*;
use teloxide::types::{FileId, InputFile, MessageId};

use super::{App, chat, keyboards};
use crate::db;
use crate::domain::effect::{Effect, Notice};
use crate::domain::ids::UserId;
use crate::domain::request::Request;
use crate::i18n::{self, Lang, Text};

/// The channel is shared by all drivers, so it uses the default language.
const CHANNEL_LANG: Lang = Lang::Es;

pub async fn apply(bot: &Bot, app: &App, request: &mut Request, effects: Vec<Effect>) -> anyhow::Result<()> {
    for effect in effects {
        match effect {
            Effect::PublishToChannel { .. } => publish(bot, app, request).await?,
            Effect::MarkChannelTaken { .. } => {
                if let Some(msg) = request.channel_msg_id {
                    let text = i18n::channel_taken(CHANNEL_LANG, &request.id);
                    log_failure(bot.edit_message_text(app.channel, MessageId(msg), text).await, "mark taken");
                }
            }
            Effect::DeleteFromChannel { .. } => {
                if let Some(msg) = request.channel_msg_id {
                    log_failure(bot.delete_message(app.channel, MessageId(msg)).await, "delete post");
                }
            }
            Effect::Notify { to, notice } => {
                log_failure(notify(bot, app, request, to, &notice).await, "notify");
            }
            Effect::SendLocation { to, point, place } => {
                // A venue carries a title, so the driver can tell pickup from destination.
                let title = i18n::place(app.lang_of(to).await, place);
                let address = app.neighborhoods.nearest(point).unwrap_or(title);
                let sent = bot.send_venue(chat(to), point.lat, point.lon, title, address).await;
                log_failure(sent, "send location");
            }
            Effect::Forget { request: id } => db::requests::forget(&app.pool, &id).await?,
        }
    }
    Ok(())
}

async fn publish(bot: &Bot, app: &App, request: &mut Request) -> anyhow::Result<()> {
    let zones = &app.neighborhoods;
    let text = i18n::channel_post(
        CHANNEL_LANG,
        &request.id,
        request.km,
        request.price,
        request.payment,
        zones.nearest(request.pickup),
        zones.nearest(request.dropoff),
    );
    // The Accept button is added only once the post id is saved, so a quick
    // acceptance can always mark the post as taken.
    let msg = bot.send_message(app.channel, text).await.context("cannot post to the offers channel")?;
    let shown = async {
        db::requests::set_channel_msg(&app.pool, &request.id, msg.id.0).await?;
        bot.edit_message_reply_markup(app.channel, msg.id)
            .reply_markup(keyboards::accept(CHANNEL_LANG, &request.id))
            .await?;
        anyhow::Ok(())
    };
    if let Err(e) = shown.await {
        log_failure(bot.delete_message(app.channel, msg.id).await, "delete post");
        return Err(e.context("cannot add the Accept button"));
    }
    request.channel_msg_id = Some(msg.id.0);
    tracing::info!(request = %request.id, "published");
    Ok(())
}

async fn notify(bot: &Bot, app: &App, request: &Request, to: UserId, notice: &Notice) -> anyhow::Result<()> {
    let lang = app.lang_of(to).await;
    match notice {
        Notice::Published { request: id } => {
            bot.send_message(chat(to), i18n::notice(lang, notice))
                .reply_markup(keyboards::open_request(lang, id))
                .await?;
            // The ride form's keyboard is still showing: bring the menu back.
            bot.send_message(chat(to), i18n::text(lang, Text::Menu))
                .reply_markup(app.menu(to, lang).await?)
                .await?;
        }
        Notice::DriverAssigned { request: id, driver } => {
            let driver = db::drivers::get(&app.pool, &app.cipher, *driver)
                .await?
                .context("assigned driver has no profile")?;
            let caption = i18n::driver_assigned(lang, id, &driver.profile);
            let photo = InputFile::file_id(FileId(driver.profile.photo_file_id.clone()));
            bot.send_photo(chat(to), photo)
                .caption(caption)
                .reply_markup(keyboards::assigned_trip(lang, id))
                .await?;
        }
        Notice::TripAssigned { request: id, client } => {
            // Looked up now instead of stored: we keep no client data.
            let username = bot.get_chat(chat(*client)).await.ok().and_then(|c| c.username().map(str::to_owned));
            let text = i18n::trip_assigned(
                lang,
                id,
                request.price,
                request.payment,
                request.reference.as_deref(),
                username.as_deref(),
                request.client_phone.as_deref(),
            );
            bot.send_message(chat(to), text).reply_markup(keyboards::assigned_trip(lang, id)).await?;
        }
        Notice::Completed { .. }
        | Notice::CancelledByClient { .. }
        | Notice::CancelledByDriver { .. }
        | Notice::Expired { .. } => {
            bot.send_message(chat(to), i18n::notice(lang, notice))
                .reply_markup(app.menu(to, lang).await?)
                .await?;
        }
    }
    Ok(())
}

fn log_failure<T, E: std::fmt::Display>(result: Result<T, E>, what: &str) {
    if let Err(e) = result {
        tracing::warn!(error = %e, "{what} failed");
    }
}
