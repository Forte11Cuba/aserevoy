//! The auction around an open request: the client raises the price or says
//! they are still waiting; drivers counteroffer from the channel, and the
//! client accepts or rejects each counteroffer.

use teloxide::prelude::*;
use teloxide::types::MessageId;

use super::client::Ctx;
use super::dialog::Dialog;
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
    db::dialogs::set(&cx.app.pool, cx.user, &Dialog::Raise { request: id.as_str().to_owned() }, now()).await?;
    cx.bot
        .send_message(chat(cx.user), i18n::raise_prompt(cx.lang, request.price))
        .reply_markup(keyboards::raise_options(id, request.price))
        .await?;
    Ok(None)
}

/// A typed amount after "How much do you raise it to?". The problem, if
/// any, is said as a message (there is no button to alert on).
pub async fn raise_typed(cx: &Ctx<'_>, request: &str, amount: Cup) -> anyhow::Result<()> {
    let Some(id) = RequestId::parse(request) else {
        return Ok(());
    };
    match raise(cx, &id, amount).await? {
        Some(problem) => {
            cx.bot.send_message(chat(cx.user), problem).await?;
        }
        None => db::dialogs::clear(&cx.app.pool, cx.user).await?,
    }
    Ok(())
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
    if let Some(Dialog::Raise { .. }) = db::dialogs::get(&cx.app.pool, cx.user).await? {
        db::dialogs::clear(&cx.app.pool, cx.user).await?;
    }
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
    // The amount starts at the client's price; ✅ Send and ❌ Cancel below.
    let dialog = Dialog::Counteroffer { request: request.id.as_str().to_owned(), amount: request.price };
    db::dialogs::set(&cx.app.pool, cx.user, &dialog, now()).await?;
    cx.bot
        .send_message(chat(cx.user), i18n::text(cx.lang, Text::CounterofferHow))
        .reply_markup(keyboards::counteroffer_send(cx.lang))
        .await?;
    let prompt = i18n::counteroffer_prompt(cx.lang, &effects::post_of(cx.app, &request).await?, request.price);
    cx.bot.send_message(chat(cx.user), prompt).reply_markup(keyboards::counteroffer_steps(&request.id)).await?;
    Ok(())
}

/// `cad:<id>:<delta>`: a +/− button. The amount never goes below the
/// client's current price; the message is edited to show it.
pub async fn adjust_counteroffer(
    cx: &Ctx<'_>,
    id: &RequestId,
    delta: Cup,
    pressed: Option<MessageId>,
) -> anyhow::Result<Option<String>> {
    let Some(Dialog::Counteroffer { request, amount }) = db::dialogs::get(&cx.app.pool, cx.user).await? else {
        return Ok(Some(i18n::domain_error(cx.lang, &DomainError::NotOpen)));
    };
    if request != id.as_str() {
        return Ok(Some(i18n::domain_error(cx.lang, &DomainError::NotOpen)));
    }
    let Some(open) = db::requests::get(&cx.app.pool, id).await?.filter(|r| r.state == State::Open) else {
        return Ok(Some(i18n::domain_error(cx.lang, &DomainError::NotOpen)));
    };
    // Never below the client's offer: a − that would go under it stops there
    // and says why.
    let wanted = amount.max(open.price) + delta;
    let amount = wanted.max(open.price);
    db::dialogs::set(&cx.app.pool, cx.user, &Dialog::Counteroffer { request, amount }, now()).await?;
    if let Some(msg) = pressed {
        let prompt = i18n::counteroffer_prompt(cx.lang, &effects::post_of(cx.app, &open).await?, amount);
        // Unchanged text (already at the client's price) is an error: ignored.
        let _ = cx.bot.edit_message_text(chat(cx.user), msg, prompt).reply_markup(keyboards::counteroffer_steps(id)).await;
    }
    Ok((wanted < open.price).then(|| i18n::counteroffer_too_low(cx.lang, open.price)))
}

/// ✅ Send: the amount the buttons reached.
pub async fn send_counteroffer(cx: &Ctx<'_>, request: &str, amount: Cup) -> anyhow::Result<()> {
    let Some(id) = RequestId::parse(request) else {
        return Ok(());
    };
    let price = db::requests::get(&cx.app.pool, &id).await?.map(|r| r.price);
    if price.is_some_and(|price| amount <= price) {
        cx.bot.send_message(chat(cx.user), i18n::text(cx.lang, Text::RaiseFirst)).await?;
        return Ok(());
    }
    if let Some(problem) = counteroffer(cx, &id, amount).await? {
        cx.bot.send_message(chat(cx.user), problem).await?;
    }
    Ok(())
}

/// A message while making a counteroffer: the amount, typed, sent at once.
pub async fn counteroffer_typed(cx: &Ctx<'_>, request: &str, msg: &Message) -> anyhow::Result<()> {
    let amount = msg.text().map(|t| t.trim().trim_end_matches("CUP").trim().parse::<Cup>());
    let (Some(id), Some(Ok(amount))) = (RequestId::parse(request), amount) else {
        cx.bot.send_message(chat(cx.user), i18n::text(cx.lang, Text::AmountNotNumber)).await?;
        return Ok(());
    };
    if let Some(problem) = counteroffer(cx, &id, amount).await? {
        cx.bot.send_message(chat(cx.user), problem).await?;
    }
    Ok(())
}

/// The driver sends a counteroffer (`cof:<id>:<amount>`, or typed). On
/// success the confirmation comes with the menu back; otherwise the problem
/// is returned (an alert, or a message for a typed amount).
pub async fn counteroffer(cx: &Ctx<'_>, id: &RequestId, amount: Cup) -> anyhow::Result<Option<String>> {
    if let Some(alert) = driver_can_bid(cx).await? {
        return Ok(Some(alert));
    }
    let Some(mut request) = db::requests::get(&cx.app.pool, id).await? else {
        return Ok(Some(i18n::domain_error(cx.lang, &DomainError::NotOpen)));
    };
    let now = now();
    let effects = match request.counteroffer(cx.user, amount, now) {
        Ok(effects) => effects,
        Err(DomainError::PriceNotHigher { current }) => return Ok(Some(i18n::counteroffer_too_low(cx.lang, current))),
        Err(e) => return Ok(Some(i18n::domain_error(cx.lang, &e))),
    };
    let expires_at = now + COUNTEROFFER_SECS;
    if !db::counteroffers::upsert(&cx.app.pool, id, cx.user, amount, expires_at).await? {
        return Ok(Some(i18n::domain_error(cx.lang, &DomainError::NotOpen)));
    }
    tracing::info!(request = %id, "counteroffer");
    effects::apply(cx.bot, cx.app, &mut request, effects).await?;
    db::dialogs::clear(&cx.app.pool, cx.user).await?;
    let menu = cx.app.menu(cx.user, cx.lang).await?;
    let sent = i18n::counteroffer_sent(cx.lang, amount);
    cx.bot.send_message(chat(cx.user), sent).reply_markup(menu).await?;
    Ok(None)
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
