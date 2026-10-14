//! Accepting an offer from the channel, and closing a trip (Completed or
//! Cancel). The database decides races: `try_assign` picks one winner and
//! `close` lets only one of two concurrent closes apply its effects.

use teloxide::prelude::*;

use super::{App, chat, effects, now};
use crate::db;
use crate::domain::driver::DriverStatus;
use crate::domain::ids::{RequestId, UserId};
use crate::domain::request::{DomainError, State};
use crate::i18n::{self, Lang, Text};

/// ✅ Accept in the channel. Returns the alert shown to the driver.
pub async fn accept(bot: &Bot, app: &App, driver: UserId, lang: Lang, id: &RequestId) -> anyhow::Result<String> {
    if app.driver_status(driver).await? != Some(DriverStatus::Active) {
        return Ok(i18n::text(lang, Text::NotActiveDriver).into());
    }
    let Some(mut request) = db::requests::get(&app.pool, id).await? else {
        return Ok(i18n::text(lang, Text::AlreadyTaken).into());
    };
    let effects = match request.accept(driver, now()) {
        Ok(effects) => effects,
        Err(DomainError::NotOpen) => return Ok(i18n::text(lang, Text::AlreadyTaken).into()),
        Err(e) => return Ok(i18n::domain_error(lang, &e)),
    };
    if db::requests::driver_busy(&app.pool, driver).await? {
        return Ok(i18n::text(lang, Text::DriverBusy).into());
    }
    if !db::requests::try_assign(&app.pool, id, driver, now()).await? {
        return Ok(i18n::text(lang, Text::AlreadyTaken).into());
    }
    tracing::info!(request = %id, "assigned");
    effects::apply(bot, app, &mut request, effects).await?;
    Ok(i18n::text(lang, Text::AcceptedAlert).into())
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

/// ❌ Cancel: the client while open or assigned, the driver once assigned.
pub async fn cancel(bot: &Bot, app: &App, by: UserId, lang: Lang, id: &RequestId) -> anyhow::Result<Option<String>> {
    let Some(mut request) = db::requests::get(&app.pool, id).await? else {
        return Ok(Some(i18n::text(lang, Text::AlreadyClosed).into()));
    };
    let before = request.state;
    let effects = match request.cancel(by) {
        Ok(effects) => effects,
        Err(e) => return Ok(Some(i18n::domain_error(lang, &e))),
    };
    if !db::requests::close(&app.pool, id, before).await? {
        // It changed meanwhile (for example, a driver took it): try again.
        return Ok(Some(i18n::text(lang, Text::SomethingFailed).into()));
    }
    tracing::info!(request = %id, "cancelled");
    effects::apply(bot, app, &mut request, effects).await?;
    bot.send_message(chat(by), i18n::you_cancelled(lang, id)).reply_markup(app.menu(by, lang).await?).await?;
    Ok(None)
}
