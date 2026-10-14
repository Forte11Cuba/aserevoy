//! Runs the effects returned by the domain. Only publishing to the channel
//! can fail the operation; a message that cannot be delivered (for example,
//! the user blocked the bot) is logged and the rest still runs.

use anyhow::Context;
use teloxide::prelude::*;
use teloxide::types::{FileId, InputFile, MessageId};

use super::dialog::{Dialog, Republish, Spot};
use super::{App, chat, keyboards, local_date, local_time, now, rating, support};
use crate::db;
use crate::db::moderation::BlockReason;
use crate::domain::effect::{Alert, Effect, Notice, Place};
use crate::domain::reputation::{CANCELLATION_BLOCK_SECS, Reputation, Role, Shown, cancellation_blocks};
use crate::domain::ids::UserId;
use crate::domain::request::Request;
use crate::i18n::{self, Lang, Post, Text};

/// The channel is shared by all drivers, so it uses the default language.
const CHANNEL_LANG: Lang = Lang::Es;

pub async fn apply(bot: &Bot, app: &App, request: &mut Request, effects: Vec<Effect>) -> anyhow::Result<()> {
    for effect in effects {
        match effect {
            Effect::PublishToChannel { .. } => publish(bot, app, request).await?,
            Effect::UpdateChannelPost { .. } => {
                if let Some(msg) = request.channel_msg_id {
                    let edit = bot
                        .edit_message_text(app.channel, MessageId(msg), channel_text(app, request).await?)
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
            Effect::ChannelTaken { .. } => {
                let metric = db::metrics::assigned(&app.pool, &local_date(now()), request.price, request.suggested).await;
                log_failure(metric, "metrics");
                if let Some(msg) = request.channel_msg_id.take() {
                    log_failure(bot.delete_message(app.channel, MessageId(msg)).await, "delete taken post");
                    log_failure(db::requests::clear_channel_msg(&app.pool, &request.id).await, "clear post");
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
                let lang = app.lang_of(to).await;
                let (approx, note) = match place {
                    Place::Pickup => (request.pickup_approx, request.reference.as_deref()),
                    Place::Dropoff => (request.dropoff_approx, request.dropoff_note.as_deref()),
                };
                if approx {
                    // No exact location: the client's description instead of a pin.
                    let text = i18n::approx_point(lang, place, note.unwrap_or_default(), app.zones.municipality(point));
                    log_failure(bot.send_message(chat(to), text).await, "send description");
                    continue;
                }
                // A venue carries a title, so the driver can tell pickup from destination.
                let title = i18n::place(lang, place);
                let address = app.zones.municipality(point).unwrap_or(title);
                let sent = bot.send_venue(chat(to), point.lat, point.lon, title, address).await;
                log_failure(sent, "send location");
            }
            Effect::ShowOffer { driver, .. } => log_failure(show_offer(bot, app, request, driver).await, "show offer"),
            Effect::HideOffer { client, msg } => {
                log_failure(bot.delete_message(chat(client), MessageId(msg)).await, "hide offer");
            }
            Effect::WithdrawDriverOffers { driver } => {
                for removed in db::counteroffers::withdraw_driver(&app.pool, driver).await? {
                    if let Some(msg) = removed.client_msg_id {
                        log_failure(bot.delete_message(chat(removed.client), MessageId(msg)).await, "hide offer");
                    }
                }
            }
            Effect::CountCancellation { client } => {
                log_failure(count_cancellation(bot, app, client).await, "count cancellation");
            }
            Effect::StartRating { client, driver } => {
                log_failure(rating::start(bot, app, client, driver).await, "start rating");
            }
            Effect::Alert { alert } => log_failure(send_alert(bot, app, &alert).await, "alert"),
            Effect::Forget { request: id } => {
                db::requests::forget(&app.pool, &id).await?;
                log_failure(db::metrics::closed(&app.pool, &local_date(now()), request.state).await, "metrics");
            }
        }
    }
    Ok(())
}

/// What drivers see of a request: zones, distance, price and the client's reputation.
pub async fn post_of<'a>(app: &'a App, request: &'a Request) -> anyhow::Result<Post<'a>> {
    Ok(Post {
        request: &request.id,
        km: request.km,
        price: request.price,
        payment: request.payment,
        from: app.zones.municipality(request.pickup),
        to: app.zones.municipality(request.dropoff),
        from_corner: (!request.pickup_approx).then(|| app.streets.nearest_corner(request.pickup)).flatten(),
        to_corner: (!request.dropoff_approx).then(|| app.streets.nearest_corner(request.dropoff)).flatten(),
        approx: request.pickup_approx || request.dropoff_approx,
        client: Some(shown(app, request.client, Role::Client).await?),
    })
}

async fn channel_text(app: &App, request: &Request) -> anyhow::Result<String> {
    Ok(i18n::channel_post(CHANNEL_LANG, &post_of(app, request).await?))
}

/// What others see of someone's reputation in a role ("New" if none yet).
pub async fn shown(app: &App, user: UserId, role: Role) -> anyhow::Result<Shown> {
    let now = now();
    let rep = db::reputation::get(&app.pool, user, role).await?;
    Ok(rep.unwrap_or(Reputation { first_use: now, ..Reputation::default() }).shown(now))
}

/// The user's current @username, looked up now (never stored).
async fn username(bot: &Bot, user: UserId) -> Option<String> {
    bot.get_chat(chat(user)).await.ok().and_then(|c| c.username().map(str::to_owned))
}

async fn publish(bot: &Bot, app: &App, request: &mut Request) -> anyhow::Result<()> {
    // The buttons are added only once the post id is saved, so a quick
    // acceptance can always mark the post.
    let msg = bot
        .send_message(app.channel, channel_text(app, request).await?)
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
    log_failure(db::metrics::published(&app.pool, &local_date(now()), request.km, request.price).await, "metrics");
    Ok(())
}

/// Sends the client one message with this driver's counteroffer, and records
/// it so it can be deleted later. If the offer changed while sending, the
/// message is already stale and goes at once.
async fn show_offer(bot: &Bot, app: &App, request: &Request, driver: UserId) -> anyhow::Result<()> {
    let Some(offer) = request.counteroffers.iter().find(|o| o.driver == driver) else {
        return Ok(());
    };
    let profile = db::drivers::get(&app.pool, &app.cipher, driver).await?.context("driver has no profile")?.profile;
    let client = request.client;
    let lang = app.lang_of(client).await;
    let photo = InputFile::file_id(FileId(profile.photo_file_id.clone()));
    let sent = bot
        .send_photo(chat(client), photo)
        .caption(i18n::offer_card(lang, &request.id, &profile, offer.amount, &shown(app, driver, Role::Driver).await?))
        .reply_markup(keyboards::offer(lang, &request.id, driver))
        .await?;
    if !db::counteroffers::set_msg(&app.pool, &request.id, driver, offer.expires_at, sent.id.0).await? {
        log_failure(bot.delete_message(chat(client), sent.id).await, "hide stale offer");
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
                .caption(i18n::confirm_driver(lang, id, &driver.profile, &shown(app, driver.id, Role::Driver).await?))
                .reply_markup(keyboards::confirm_driver(lang, id, driver.id))
                .await?;
        }
        Notice::DriverConfirmed { request: id, driver } => {
            let driver = driver_profile(*driver).await?;
            let contact = username(bot, driver.id).await;
            bot.send_message(chat(to), i18n::driver_confirmed(lang, id, &driver.profile, contact.as_deref()))
                .reply_markup(keyboards::client_trip(lang, id))
                .await?;
        }
        Notice::DriverAssigned { request: id, driver } => {
            let driver = driver_profile(*driver).await?;
            let photo = InputFile::file_id(FileId(driver.profile.photo_file_id.clone()));
            let rep = shown(app, driver.id, Role::Driver).await?;
            let contact = username(bot, driver.id).await;
            bot.send_photo(chat(to), photo)
                .caption(i18n::driver_assigned(lang, id, &driver.profile, &rep, contact.as_deref()))
                .reply_markup(keyboards::client_trip(lang, id))
                .await?;
        }
        Notice::TripAssigned { request: id, client } => {
            // Looked up now instead of stored: we keep no client data.
            let username = username(bot, *client).await;
            let text = i18n::trip_assigned(
                lang,
                id,
                request.price,
                request.payment,
                request.reference.as_deref(),
                username.as_deref(),
                request.client_phone.as_deref(),
            );
            bot.send_message(chat(to), text).reply_markup(keyboards::driver_trip(lang, id)).await?;
        }
        Notice::AwaitingClient { request: id } => {
            bot.send_message(chat(to), i18n::notice(lang, notice))
                .reply_markup(keyboards::awaiting_client(lang, id))
                .await?;
        }
        Notice::DriverWithdrew { .. } => {
            bot.send_message(chat(to), i18n::notice(lang, notice)).await?;
        }
        // The trip's buttons again, on the latest message: the first ones
        // are far up the chat by now.
        Notice::ClientOnTheWay { request: id } => {
            bot.send_message(chat(to), i18n::notice(lang, notice))
                .reply_markup(keyboards::driver_trip(lang, id))
                .await?;
        }
        Notice::DriverArrived { request: id } => {
            bot.send_message(chat(to), i18n::notice(lang, notice))
                .reply_markup(keyboards::client_trip(lang, id))
                .await?;
        }
        Notice::OfferOutbid { .. }
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

/// A cancellation against the client; the third in 7 days blocks them for
/// 24 hours and alerts the admins.
async fn count_cancellation(bot: &Bot, app: &App, client: UserId) -> anyhow::Result<()> {
    let now = now();
    let recent = db::moderation::count_cancellation(&app.pool, client, now).await?;
    if !cancellation_blocks(recent) {
        return Ok(());
    }
    let until = now + CANCELLATION_BLOCK_SECS;
    // Blocked by the system: no staff member, so the bot's own "id" 0.
    db::moderation::block(&app.pool, client, Role::Client, Some(until), BlockReason::Cancellations, UserId(0), now)
        .await?;
    tracing::info!("client blocked for cancellations");
    let lang = app.lang_of(client).await;
    let _ = bot.send_message(chat(client), i18n::blocked(lang, Some(&local_time(until)))).await;
    let is_driver = app.driver_status(client).await?.is_some();
    support::alert(bot, app, i18n::alert_cancellations, None, |lang| keyboards::moderate(lang, client, is_driver)).await
}

async fn send_alert(bot: &Bot, app: &App, alert: &Alert) -> anyhow::Result<()> {
    match alert {
        Alert::UnsafeTrip { request, client, .. } => {
            let is_driver = app.driver_status(*client).await?.is_some();
            let text = |lang| i18n::alert_unsafe(lang, request);
            support::alert(bot, app, text, None, |lang| keyboards::moderate(lang, *client, is_driver)).await
        }
    }
}

/// Keeps the trip (not the phone) for the "Publish again" button, unless the
/// client is already in another conversation. It goes with the dialog's TTL.
async fn offer_republish(app: &App, request: &Request) -> anyhow::Result<()> {
    if db::dialogs::get::<Dialog>(&app.pool, request.client).await?.is_some() {
        return Ok(());
    }
    let republish = Republish {
        pickup: Spot { point: request.pickup, approx: request.pickup_approx, note: request.reference.clone() },
        dropoff: Spot { point: request.dropoff, approx: request.dropoff_approx, note: request.dropoff_note.clone() },
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
