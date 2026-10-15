//! Accepting an offer from the channel, the client confirming the driver, and
//! closing a trip (Completed or Cancel). The database decides races:
//! `try_reserve` picks one driver and `close` lets only one of two concurrent
//! closes apply its effects.

use teloxide::prelude::*;
use teloxide::types::MessageId;

use super::{App, chat, effects, keyboards, local_time, now};
use crate::db;
use crate::domain::driver::DriverStatus;
use crate::domain::effect::Notice;
use crate::domain::ids::{RequestId, UserId};
use crate::domain::request::{CancelReason, DomainError, State};
use crate::i18n::{self, Button, Lang, Text};

/// ✅ Accept in the channel: reserves the request while the client confirms.
/// Returns the alert shown to the driver.
pub async fn accept(bot: &Bot, app: &App, driver: UserId, lang: Lang, id: &RequestId) -> anyhow::Result<String> {
    if app.driver_status(driver).await? != Some(DriverStatus::Active) {
        return Ok(i18n::text(lang, Text::NotActiveDriver).into());
    }
    let Some(mut request) = db::requests::get(&app.pool, id).await? else {
        return Ok(i18n::text(lang, Text::AlreadyTaken).into());
    };
    let now = now();
    let effects = match request.accept(driver, now) {
        Ok(effects) => effects,
        Err(DomainError::NotOpen) => return Ok(i18n::text(lang, Text::AlreadyTaken).into()),
        Err(e) => return Ok(i18n::domain_error(lang, &e)),
    };
    if db::requests::driver_busy(&app.pool, driver).await? {
        return Ok(i18n::text(lang, Text::DriverBusy).into());
    }
    let confirm_by = request.confirm_by.expect("accepting sets the deadline");
    if !db::requests::try_reserve(&app.pool, id, driver, confirm_by, now).await? {
        return Ok(i18n::text(lang, Text::AlreadyTaken).into());
    }
    tracing::info!(request = %id, "reserved");
    effects::apply(bot, app, &mut request, effects).await?;
    Ok(i18n::text(lang, Text::AcceptedAlert).into())
}

/// ✅ Confirm (`cok`) or ❌ cancel (`cno`) on the card of `driver`. If that
/// driver withdrew meanwhile, nothing happens: the request is back in the
/// channel (or held by another driver, whose own card the client gets).
pub async fn confirm_or_decline(
    bot: &Bot,
    app: &App,
    by: UserId,
    lang: Lang,
    id: &RequestId,
    driver: UserId,
    confirmed: bool,
) -> anyhow::Result<Option<String>> {
    match db::requests::get(&app.pool, id).await? {
        Some(r) if r.client == by && r.state != State::Assigned && r.driver != Some(driver) => {
            Ok(Some(i18n::notice(lang, &Notice::DriverWithdrew { request: id.clone() })))
        }
        _ if confirmed => confirm(bot, app, by, lang, id, driver).await,
        _ => cancel(bot, app, by, lang, id, None).await,
    }
}

async fn confirm(
    bot: &Bot,
    app: &App,
    by: UserId,
    lang: Lang,
    id: &RequestId,
    driver: UserId,
) -> anyhow::Result<Option<String>> {
    let Some(mut request) = db::requests::get(&app.pool, id).await? else {
        return Ok(Some(i18n::text(lang, Text::AlreadyClosed).into()));
    };
    let now = now();
    let effects = match request.confirm(by, now) {
        Ok(effects) => effects,
        Err(e) => return Ok(Some(i18n::domain_error(lang, &e))),
    };
    // Only the driver on the card the client pressed.
    if !db::requests::confirm(&app.pool, id, driver, now).await? {
        return Ok(Some(i18n::text(lang, Text::AlreadyClosed).into()));
    }
    tracing::info!(request = %id, "assigned");
    effects::apply(bot, app, &mut request, effects).await?;
    Ok(None)
}

/// 🚪 Withdraw: the reserved driver gives the request back to the channel.
pub async fn withdraw(bot: &Bot, app: &App, by: UserId, lang: Lang, id: &RequestId) -> anyhow::Result<Option<String>> {
    let Some(mut request) = db::requests::get(&app.pool, id).await? else {
        return Ok(Some(i18n::text(lang, Text::AlreadyClosed).into()));
    };
    let effects = match request.withdraw(by) {
        Ok(effects) => effects,
        Err(e) => return Ok(Some(i18n::domain_error(lang, &e))),
    };
    if !db::requests::release(&app.pool, id, by).await? {
        // The client confirmed or cancelled at the same moment.
        return Ok(Some(i18n::domain_error(lang, &DomainError::NotOpen)));
    }
    tracing::info!(request = %id, "driver withdrew");
    effects::apply(bot, app, &mut request, effects).await?;
    bot.send_message(chat(by), i18n::text(lang, Text::Withdrawn)).await?;
    Ok(None)
}

/// 🏁 Completed, by the client or the driver.
pub async fn complete(bot: &Bot, app: &App, by: UserId, lang: Lang, id: &RequestId) -> anyhow::Result<Option<String>> {
    let Some(mut request) = db::requests::get(&app.pool, id).await? else {
        return Ok(Some(i18n::text(lang, Text::AlreadyClosed).into()));
    };
    let effects = match request.complete(by) {
        Ok(effects) => effects,
        Err(e) => return Ok(Some(i18n::domain_error(lang, &e))),
    };
    if !db::requests::close(&app.pool, id, State::Assigned).await? {
        return Ok(Some(i18n::text(lang, Text::AlreadyClosed).into()));
    }
    tracing::info!(request = %id, "completed");
    effects::apply(bot, app, &mut request, effects).await?;
    bot.send_message(chat(by), i18n::you_completed(lang, id)).reply_markup(app.menu(by, lang).await?).await?;
    Ok(None)
}

/// ❌ Cancel pressed. An assigned trip first asks for the reason, replacing the
/// trip's buttons (`pressed`) with the reasons of the presser's side; otherwise
/// it cancels at once. Returns `Ok(None)` when the pressed buttons are spent.
/// A button of the trip menu below. They carry no request id: a user has at
/// most one trip at a time, looked up now. With none, the menu is put right.
pub async fn from_menu(bot: &Bot, app: &App, by: UserId, lang: Lang, pressed: Button) -> anyhow::Result<()> {
    let trip = db::requests::active_for(&app.pool, by).await?.filter(|r| r.state == State::Assigned);
    let Some(mut trip) = trip else {
        let menu = app.menu(by, lang).await?;
        bot.send_message(chat(by), i18n::text(lang, Text::NoActiveTrip)).reply_markup(menu).await?;
        return Ok(());
    };
    let id = trip.id.clone();
    let problem = match pressed {
        Button::OnMyWay | Button::Arrived => {
            let problem = on_my_way_or_arrived(bot, app, by, lang, &id, pressed == Button::Arrived).await?;
            if problem.is_none() {
                bot.send_message(chat(by), i18n::text(lang, Text::Notified)).await?;
            }
            problem
        }
        Button::ShareTrip => share(bot, app, by, lang, &id).await?,
        Button::Complete => complete(bot, app, by, lang, &id).await?,
        Button::CancelTrip => {
            let reasons: &[CancelReason] =
                if by == trip.client { &CancelReason::FOR_CLIENT } else { &CancelReason::FOR_DRIVER };
            bot.send_message(chat(by), i18n::text(lang, Text::ChooseCancelReason))
                .reply_markup(keyboards::cancel_reasons(lang, &id, reasons))
                .await?;
            None
        }
        Button::MyTrip => {
            effects::resend_trip(bot, app, &mut trip, by).await?;
            None
        }
        _ => None,
    };
    if let Some(problem) = problem {
        bot.send_message(chat(by), problem).await?;
    }
    Ok(())
}

pub async fn cancel_pressed(
    bot: &Bot,
    app: &App,
    by: UserId,
    lang: Lang,
    id: &RequestId,
    pressed: Option<MessageId>,
) -> anyhow::Result<Option<String>> {
    if let Some(request) = db::requests::get(&app.pool, id).await?
        && request.state == State::Assigned
    {
        let reasons: &[CancelReason] = if by == request.client {
            &CancelReason::FOR_CLIENT
        } else if request.driver == Some(by) {
            &CancelReason::FOR_DRIVER
        } else {
            return Ok(Some(i18n::domain_error(lang, &DomainError::NotParticipant)));
        };
        if let Some(msg) = pressed {
            bot.edit_message_reply_markup(chat(by), msg).reply_markup(keyboards::cancel_reasons(lang, id, reasons)).await?;
        }
        return Ok(Some(i18n::text(lang, Text::ChooseCancelReason).into()));
    }
    let result = cancel(bot, app, by, lang, id, None).await?;
    if result.is_none()
        && let Some(msg) = pressed
    {
        let _ = bot.edit_message_reply_markup(chat(by), msg).await;
    }
    Ok(result)
}

/// ↩️ Back from the reasons: the trip's buttons again.
pub async fn cancel_back(bot: &Bot, app: &App, by: UserId, lang: Lang, id: &RequestId, pressed: Option<MessageId>) -> anyhow::Result<()> {
    if let (Some(msg), Some(request)) = (pressed, db::requests::get(&app.pool, id).await?) {
        let markup = if by == request.client { keyboards::client_trip(lang, id) } else { keyboards::driver_trip(lang, id) };
        bot.edit_message_reply_markup(chat(by), msg).reply_markup(markup).await?;
    }
    Ok(())
}

/// ❌ Cancel: the client while open, confirming (turning the driver down) or
/// assigned; the driver once assigned. Assigned trips need a reason.
pub async fn cancel(
    bot: &Bot,
    app: &App,
    by: UserId,
    lang: Lang,
    id: &RequestId,
    reason: Option<CancelReason>,
) -> anyhow::Result<Option<String>> {
    let Some(mut request) = db::requests::get(&app.pool, id).await? else {
        return Ok(Some(i18n::text(lang, Text::AlreadyClosed).into()));
    };
    let before = request.state;
    let effects = match request.cancel(by, reason, now()) {
        Ok(effects) => effects,
        Err(e) => return Ok(Some(i18n::domain_error(lang, &e))),
    };
    if !db::requests::close(&app.pool, id, before).await? {
        // It changed meanwhile (for example, a driver took it): try again.
        return Ok(Some(i18n::text(lang, Text::SomethingFailed).into()));
    }
    tracing::info!(request = %id, reason = reason.map(CancelReason::code), "cancelled");
    effects::apply(bot, app, &mut request, effects).await?;
    // Turning a driver down already told the client (with "Publish again").
    if before != State::Confirming {
        bot.send_message(chat(by), i18n::you_cancelled(lang, id)).reply_markup(app.menu(by, lang).await?).await?;
    }
    Ok(None)
}

/// 🚶 On my way (client) or 📍 Arrived (driver): tells the other side once;
/// the other side's notice is the confirmation.
pub async fn on_my_way_or_arrived(
    bot: &Bot,
    app: &App,
    by: UserId,
    lang: Lang,
    id: &RequestId,
    arrived: bool,
) -> anyhow::Result<Option<String>> {
    let Some(mut request) = db::requests::get(&app.pool, id).await? else {
        return Ok(Some(i18n::text(lang, Text::AlreadyClosed).into()));
    };
    let now = now();
    let result = if arrived { request.arrived(by, now) } else { request.on_my_way(by, now) };
    let effects = match result {
        Ok(effects) => effects,
        Err(e) => return Ok(Some(i18n::domain_error(lang, &e))),
    };
    let saved = if arrived {
        db::requests::mark_arrived(&app.pool, id, by, now).await?
    } else {
        db::requests::mark_on_my_way(&app.pool, id, by, now).await?
    };
    if !saved {
        return Ok(Some(i18n::domain_error(lang, &DomainError::AlreadyDone)));
    }
    effects::apply(bot, app, &mut request, effects).await?;
    Ok(None)
}

/// 📤 Share trip: a text for the client to forward; nothing is stored.
pub async fn share(bot: &Bot, app: &App, by: UserId, lang: Lang, id: &RequestId) -> anyhow::Result<Option<String>> {
    let request = db::requests::get(&app.pool, id).await?;
    let Some((request, driver)) = request.and_then(|r| r.driver.filter(|_| r.client == by && r.state == State::Assigned).map(|d| (r, d)))
    else {
        return Ok(Some(i18n::text(lang, Text::AlreadyClosed).into()));
    };
    let Some(driver) = db::drivers::get(&app.pool, &app.cipher, driver).await? else {
        return Ok(Some(i18n::text(lang, Text::SomethingFailed).into()));
    };
    let departure = local_time(request.assigned_at.unwrap_or_else(now));
    bot.send_message(chat(by), i18n::share_trip(lang, id, &driver.profile, &departure)).await?;
    Ok(None)
}
