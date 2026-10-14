//! Runs the effects returned by the domain. Only publishing to the channel
//! can fail the operation; a message that cannot be delivered (for example,
//! the user blocked the bot) is logged and the rest still runs.

use anyhow::Context;
use teloxide::prelude::*;
use teloxide::types::{FileId, InputFile, MessageId};

use super::dialog::{Dialog, Republish};
use super::{App, chat, keyboards, now};
use crate::db;
use crate::domain::effect::{Effect, Notice};
use crate::domain::ids::UserId;
use crate::domain::request::Request;
use crate::i18n::{self, Lang, OfferLine, Text};

/// The channel is shared by all drivers, so it uses the default language.
const CHANNEL_LANG: Lang = Lang::Es;

pub async fn apply(bot: &Bot, app: &App, request: &mut Request, effects: Vec<Effect>) -> anyhow::Result<()> {
    for effect in effects {
        match effect {
            Effect::PublishToChannel { .. } => publish(bot, app, request).await?,
            Effect::UpdateChannelPost { .. } => {
                if let Some(msg) = request.channel_msg_id {
                    let edit = bot
                        .edit_message_text(app.channel, MessageId(msg), channel_text(app, request))
                        .reply_markup(keyboards::accept(CHANNEL_LANG, &request.id, &app.bot_username))
                        .await;
                    log_failure(edit, "update post");
                }
            }
            Effect::MarkChannelConfirming { .. } => {
                if let Some(msg) = request.channel_msg_id {
                    let text = i18n::channel_confirming(CHANNEL_LANG, &request.id);
                    log_failure(bot.edit_message_text(app.channel, MessageId(msg), text).await, "mark confirming");
                }
            }
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
                let address = app.zones.municipality(point).unwrap_or(title);
                let sent = bot.send_venue(chat(to), point.lat, point.lon, title, address).await;
                log_failure(sent, "send location");
            }
            Effect::RefreshOffers { .. } => log_failure(refresh_offers(bot, app, request).await, "refresh offers"),
            Effect::WithdrawDriverOffers { driver } => {
                log_failure(withdraw_offers(bot, app, request, driver).await, "withdraw offers");
            }
            Effect::Forget { request: id } => db::requests::forget(&app.pool, &id).await?,
        }
    }
    Ok(())
}

fn channel_text(app: &App, request: &Request) -> String {
    i18n::channel_post(
        CHANNEL_LANG,
        &request.id,
        request.km,
        request.price,
        request.payment,
        app.zones.municipality(request.pickup),
        app.zones.municipality(request.dropoff),
    )
}

async fn publish(bot: &Bot, app: &App, request: &mut Request) -> anyhow::Result<()> {
    // The buttons are added only once the post id is saved, so a quick
    // acceptance can always mark the post.
    let msg = bot
        .send_message(app.channel, channel_text(app, request))
        .await
        .context("cannot post to the offers channel")?;
    let shown = async {
        db::requests::set_channel_msg(&app.pool, &request.id, msg.id.0).await?;
        bot.edit_message_reply_markup(app.channel, msg.id)
            .reply_markup(keyboards::accept(CHANNEL_LANG, &request.id, &app.bot_username))
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

/// Shows the client the valid counteroffers in one message, edited in place;
/// deletes it when none are left.
pub async fn refresh_offers(bot: &Bot, app: &App, request: &mut Request) -> anyhow::Result<()> {
    let client = request.client;
    let lang = app.lang_of(client).await;
    let now = now();
    let mut drivers = Vec::new();
    for offer in request.counteroffers.iter().filter(|o| o.expires_at > now) {
        if let Some(d) = db::drivers::get(&app.pool, &app.cipher, offer.driver).await? {
            drivers.push((offer, d.profile));
        }
    }

    if drivers.is_empty() {
        if let Some(msg) = request.offers_msg_id.take() {
            log_failure(bot.delete_message(chat(client), MessageId(msg)).await, "delete offers");
            db::requests::set_offers_msg(&app.pool, &request.id, None).await?;
        }
        return Ok(());
    }

    let lines: Vec<OfferLine> = drivers
        .iter()
        .map(|(o, p)| OfferLine {
            alias: &p.alias,
            bike_model: &p.bike_model,
            amount: o.amount,
            minutes_left: (o.expires_at - now + 59) / 60,
        })
        .collect();
    let text = i18n::offers_list(lang, &request.id, request.price, &lines);
    let buttons: Vec<_> = drivers.iter().map(|(o, p)| (o.driver, o.amount, p.alias.as_str())).collect();
    let markup = keyboards::offers(lang, &request.id, &buttons);

    if let Some(msg) = request.offers_msg_id {
        let edited =
            bot.edit_message_text(chat(client), MessageId(msg), text.clone()).reply_markup(markup.clone()).await;
        match edited {
            Ok(_) => return Ok(()),
            // Same content as before: nothing to do.
            Err(e) if e.to_string().contains("not modified") => return Ok(()),
            // The message is gone or too old to edit: send a new one.
            Err(_) => {}
        }
    }
    let sent = bot.send_message(chat(client), text).reply_markup(markup).await?;
    request.offers_msg_id = Some(sent.id.0);
    db::requests::set_offers_msg(&app.pool, &request.id, Some(sent.id.0)).await?;
    Ok(())
}

/// The driver is busy now: their counteroffers on other requests go, and
/// those clients' lists are updated.
async fn withdraw_offers(bot: &Bot, app: &App, current: &Request, driver: UserId) -> anyhow::Result<()> {
    for id in db::counteroffers::withdraw_driver(&app.pool, driver).await? {
        if id == current.id {
            continue;
        }
        if let Some(mut other) = db::requests::get(&app.pool, &id).await? {
            refresh_offers(bot, app, &mut other).await?;
        }
    }
    Ok(())
}

async fn notify(bot: &Bot, app: &App, request: &Request, to: UserId, notice: &Notice) -> anyhow::Result<()> {
    let lang = app.lang_of(to).await;
    let driver_profile = |driver: UserId| async move {
        db::drivers::get(&app.pool, &app.cipher, driver).await?.context("driver has no profile")
    };
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
        Notice::PriceRaised { request: id } => {
            bot.send_message(chat(to), i18n::notice(lang, notice))
                .reply_markup(keyboards::open_request(lang, id))
                .await?;
        }
        Notice::StillWaiting { request: id } => {
            bot.send_message(chat(to), i18n::notice(lang, notice))
                .reply_markup(keyboards::still_waiting(lang, id))
                .await?;
        }
        Notice::ConfirmDriver { request: id, driver } => {
            let driver = driver_profile(*driver).await?;
            let photo = InputFile::file_id(FileId(driver.profile.photo_file_id.clone()));
            bot.send_photo(chat(to), photo)
                .caption(i18n::confirm_driver(lang, id, &driver.profile))
                .reply_markup(keyboards::confirm_driver(lang, id))
                .await?;
        }
        Notice::DriverConfirmed { request: id, driver } => {
            let driver = driver_profile(*driver).await?;
            bot.send_message(chat(to), i18n::driver_confirmed(lang, id, &driver.profile))
                .reply_markup(keyboards::assigned_trip(lang, id))
                .await?;
        }
        Notice::DriverAssigned { request: id, driver } => {
            let driver = driver_profile(*driver).await?;
            let photo = InputFile::file_id(FileId(driver.profile.photo_file_id.clone()));
            bot.send_photo(chat(to), photo)
                .caption(i18n::driver_assigned(lang, id, &driver.profile))
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
        Notice::AwaitingClient { .. }
        | Notice::ClientDeclined { .. }
        | Notice::ClientDidNotConfirm { .. }
        | Notice::CounterofferRejected { .. }
        | Notice::RequestGone { .. } => {
            bot.send_message(chat(to), i18n::notice(lang, notice)).await?;
        }
        Notice::Completed { .. } | Notice::CancelledByClient { .. } => {
            bot.send_message(chat(to), i18n::notice(lang, notice))
                .reply_markup(app.menu(to, lang).await?)
                .await?;
        }
        Notice::CancelledByDriver { .. }
        | Notice::ConfirmTimedOut { .. }
        | Notice::DeclinedDriver { .. }
        | Notice::Expired { .. } => {
            offer_republish(app, request).await?;
            let text = format!("{}\n\n{}", i18n::notice(lang, notice), i18n::republish_hint(lang));
            bot.send_message(chat(to), text).reply_markup(keyboards::republish(lang)).await?;
        }
    }
    Ok(())
}

/// Keeps the trip (not the phone) for the "Publish again" button, unless the
/// client is already in another conversation. It goes with the dialog's TTL.
async fn offer_republish(app: &App, request: &Request) -> anyhow::Result<()> {
    if db::dialogs::get::<Dialog>(&app.pool, request.client).await?.is_some() {
        return Ok(());
    }
    let republish = Republish {
        pickup: request.pickup,
        reference: request.reference.clone(),
        dropoff: request.dropoff,
        km: request.km,
        price: request.price,
        payment: request.payment,
    };
    db::dialogs::set(&app.pool, request.client, &Dialog::Republish(republish), now()).await
}

fn log_failure<T, E: std::fmt::Display>(result: Result<T, E>, what: &str) {
    if let Err(e) = result {
        tracing::warn!(error = %e, "{what} failed");
    }
}
