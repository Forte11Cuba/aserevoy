//! The auction around an open request: the client raises the price or says
//! they are still waiting; drivers counteroffer from the channel, and the
//! client accepts or rejects each counteroffer.

use teloxide::prelude::*;

use super::client::Ctx;
use super::{chat, effects, keyboards, now};
use crate::db;
use crate::domain::driver::DriverStatus;
use crate::domain::fare::Cup;
use crate::domain::ids::{RequestId, UserId};
use crate::domain::request::{COUNTEROFFER_SECS, DomainError, Request, State};
use crate::i18n::{self, Text};

/// The request if `cx.user` is its client and it is still open; otherwise the
/// alert to show.
async fn own_open_request(cx: &Ctx<'_>, id: &RequestId) -> anyhow::Result<Result<Request, String>> {
    match db::requests::get(&cx.app.pool, id).await? {
        Some(r) if r.client != cx.user => Ok(Err(i18n::domain_error(cx.lang, &DomainError::NotParticipant))),
        Some(r) if r.state == State::Open => Ok(Ok(r)),
        _ => Ok(Err(i18n::domain_error(cx.lang, &DomainError::NotOpen))),
    }
}

/// ⬆️ Raise price: offers the new amounts.
pub async fn ask_raise(cx: &Ctx<'_>, id: &RequestId) -> anyhow::Result<Option<String>> {
    let request = match own_open_request(cx, id).await? {
        Ok(r) => r,
        Err(alert) => return Ok(Some(alert)),
    };
    cx.bot
        .send_message(chat(cx.user), i18n::raise_prompt(cx.lang, request.price))
        .reply_markup(keyboards::raise_options(id, request.price))
        .await?;
    Ok(None)
}

/// `rai:<id>:<price>`: the client picked a new price.
pub async fn raise(cx: &Ctx<'_>, id: &RequestId, price: Cup) -> anyhow::Result<Option<String>> {
    let Some(mut request) = db::requests::get(&cx.app.pool, id).await? else {
        return Ok(Some(i18n::domain_error(cx.lang, &DomainError::NotOpen)));
    };
    let effects = match request.raise_price(cx.user, price, now()) {
        Ok(effects) => effects,
        Err(e) => return Ok(Some(i18n::domain_error(cx.lang, &e))),
    };
    if !db::requests::raise_price(&cx.app.pool, id, price).await? {
        return Ok(Some(i18n::domain_error(cx.lang, &DomainError::NotOpen)));
    }
    tracing::info!(request = %id, "price raised");
    effects::apply(cx.bot, cx.app, &mut request, effects).await?;
    Ok(None)
}

/// ✅ Yes, still waiting.
pub async fn keep_waiting(cx: &Ctx<'_>, id: &RequestId) -> anyhow::Result<Option<String>> {
    let Some(mut request) = db::requests::get(&cx.app.pool, id).await? else {
        return Ok(Some(i18n::domain_error(cx.lang, &DomainError::NotOpen)));
    };
    if let Err(e) = request.keep_waiting(cx.user, now()) {
        return Ok(Some(i18n::domain_error(cx.lang, &e)));
    }
    if !db::requests::keep_waiting(&cx.app.pool, id).await? {
        return Ok(Some(i18n::domain_error(cx.lang, &DomainError::NotOpen)));
    }
    cx.bot.send_message(chat(cx.user), i18n::text(cx.lang, Text::KeptWaiting)).await?;
    Ok(None)
}

/// Whether the driver may take or counteroffer a request now; the alert otherwise.
async fn driver_can_bid(cx: &Ctx<'_>) -> anyhow::Result<Option<String>> {
    if cx.app.driver_status(cx.user).await? != Some(DriverStatus::Active) {
        return Ok(Some(i18n::text(cx.lang, Text::NotActiveDriver).into()));
    }
    if db::requests::driver_busy(&cx.app.pool, cx.user).await? {
        return Ok(Some(i18n::text(cx.lang, Text::DriverBusy).into()));
    }
    Ok(None)
}

/// `/start co_<id>`: a driver opened 💬 Counteroffer in the channel.
pub async fn start_counteroffer(cx: &Ctx<'_>, id: &str) -> anyhow::Result<()> {
    let say = |text: String| async move {
        cx.bot.send_message(chat(cx.user), text).await?;
        anyhow::Ok(())
    };
    if let Some(alert) = driver_can_bid(cx).await? {
        return say(alert).await;
    }
    let request = match RequestId::parse(id) {
        Some(id) => db::requests::get(&cx.app.pool, &id).await?,
        None => None,
    };
    let Some(request) = request.filter(|r| r.state == State::Open && now() < r.expires_at) else {
        return say(i18n::domain_error(cx.lang, &DomainError::NotOpen)).await;
    };
    if request.client == cx.user {
        return say(i18n::domain_error(cx.lang, &DomainError::OwnRequest)).await;
    }
    let prompt = i18n::counteroffer_prompt(cx.lang, &effects::post_of(cx.app, &request).await?);
    cx.bot
        .send_message(chat(cx.user), prompt)
        .reply_markup(keyboards::counteroffer_amounts(&request.id, request.price))
        .await?;
    Ok(())
}

/// `cof:<id>:<amount>`: the driver sends a counteroffer.
pub async fn counteroffer(cx: &Ctx<'_>, id: &RequestId, amount: Cup) -> anyhow::Result<String> {
    if let Some(alert) = driver_can_bid(cx).await? {
        return Ok(alert);
    }
    let Some(mut request) = db::requests::get(&cx.app.pool, id).await? else {
        return Ok(i18n::domain_error(cx.lang, &DomainError::NotOpen));
    };
    let now = now();
    let effects = match request.counteroffer(cx.user, amount, now) {
        Ok(effects) => effects,
        Err(e) => return Ok(i18n::domain_error(cx.lang, &e)),
    };
    let expires_at = now + COUNTEROFFER_SECS;
    if !db::counteroffers::upsert(&cx.app.pool, id, cx.user, amount, expires_at).await? {
        return Ok(i18n::domain_error(cx.lang, &DomainError::NotOpen));
    }
    tracing::info!(request = %id, "counteroffer");
    effects::apply(cx.bot, cx.app, &mut request, effects).await?;
    Ok(i18n::text(cx.lang, Text::CounterofferSent).into())
}

/// `coa:<id>:<driver>`: the client accepts a counteroffer.
pub async fn accept_offer(cx: &Ctx<'_>, id: &RequestId, driver: UserId) -> anyhow::Result<Option<String>> {
    let Some(mut request) = db::requests::get(&cx.app.pool, id).await? else {
        return Ok(Some(i18n::domain_error(cx.lang, &DomainError::NotOpen)));
    };
    if cx.app.driver_status(driver).await? != Some(DriverStatus::Active) {
        return Ok(Some(i18n::domain_error(cx.lang, &DomainError::OfferGone)));
    }
    let now = now();
    let effects = match request.accept_counteroffer(cx.user, driver, now) {
        Ok(effects) => effects,
        Err(e) => return Ok(Some(i18n::domain_error(cx.lang, &e))),
    };
    if !db::requests::assign_counteroffer(&cx.app.pool, id, driver, request.price, now).await? {
        // The offer expired, the driver got busy, or the request changed.
        return Ok(Some(i18n::domain_error(cx.lang, &DomainError::OfferGone)));
    }
    tracing::info!(request = %id, "assigned by counteroffer");
    effects::apply(cx.bot, cx.app, &mut request, effects).await?;
    Ok(None)
}

/// `cor:<id>:<driver>`: the client rejects a counteroffer.
pub async fn reject_offer(cx: &Ctx<'_>, id: &RequestId, driver: UserId) -> anyhow::Result<Option<String>> {
    let Some(mut request) = db::requests::get(&cx.app.pool, id).await? else {
        return Ok(Some(i18n::domain_error(cx.lang, &DomainError::NotOpen)));
    };
    let effects = match request.reject_counteroffer(cx.user, driver) {
        Ok(effects) => effects,
        Err(e) => return Ok(Some(i18n::domain_error(cx.lang, &e))),
    };
    if !db::counteroffers::delete(&cx.app.pool, id, driver).await? {
        return Ok(Some(i18n::domain_error(cx.lang, &DomainError::OfferGone)));
    }
    effects::apply(cx.bot, cx.app, &mut request, effects).await?;
    Ok(None)
}
