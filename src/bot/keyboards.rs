//! Keyboards. Callback data stays under Telegram's 64-byte limit by using
//! short prefixes and ids (`acc:AB7K`).

use teloxide::types::{ButtonRequest, InlineKeyboardButton, InlineKeyboardMarkup, KeyboardButton, KeyboardMarkup};

use crate::domain::fare::Cup;
use crate::domain::ids::{RequestId, UserId};
use crate::i18n::{Button, Lang, button};

fn reply(lang: Lang, b: Button) -> KeyboardButton {
    KeyboardButton::new(button(lang, b))
}

fn inline(lang: Lang, b: Button, data: impl Into<String>) -> InlineKeyboardButton {
    InlineKeyboardButton::callback(button(lang, b), data.into())
}

pub fn main_menu(lang: Lang, is_driver: bool, is_staff: bool) -> KeyboardMarkup {
    let profile = if is_driver { Button::MyProfile } else { Button::ApplyDriver };
    let mut rows = vec![vec![reply(lang, Button::RequestRide)], vec![reply(lang, profile)]];
    if is_staff {
        rows.push(vec![reply(lang, Button::InviteDriver)]);
    }
    rows.push(vec![reply(lang, Button::SwitchLang)]);
    KeyboardMarkup::new(rows).resize_keyboard().persistent()
}

/// While a conversation runs: optional extra buttons plus Cancel.
pub fn step(lang: Lang, extra: &[KeyboardButton]) -> KeyboardMarkup {
    let mut rows: Vec<Vec<KeyboardButton>> = extra.iter().cloned().map(|b| vec![b]).collect();
    rows.push(vec![reply(lang, Button::Cancel)]);
    KeyboardMarkup::new(rows).resize_keyboard()
}

pub fn skip(lang: Lang) -> KeyboardButton {
    reply(lang, Button::Skip)
}

pub fn send_location(lang: Lang) -> KeyboardButton {
    reply(lang, Button::SendMyLocation).request(ButtonRequest::Location)
}

pub fn share_phone(lang: Lang) -> KeyboardButton {
    reply(lang, Button::SharePhone).request(ButtonRequest::Contact)
}

/// "Is it here?" under a pin. `which` is `pick` or `drop`.
pub fn confirm_point(lang: Lang, which: &str) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        inline(lang, Button::ItsHere, format!("{which}:ok")),
        inline(lang, Button::Fix, format!("{which}:fix")),
    ]])
}

/// The suggested price and two higher ones.
pub fn prices(suggested: Cup) -> InlineKeyboardMarkup {
    let row = [suggested, suggested + 50, suggested + 100]
        .into_iter()
        .map(|p| InlineKeyboardButton::callback(format!("{p} CUP"), format!("price:{p}")))
        .collect::<Vec<_>>();
    InlineKeyboardMarkup::new(vec![row])
}

pub fn payment(lang: Lang) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        inline(lang, Button::Cash, "pay:cash"),
        inline(lang, Button::Transfer, "pay:transfer"),
    ]])
}

pub fn payment_methods(lang: Lang) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![inline(lang, Button::Cash, "pm:cash"), inline(lang, Button::Transfer, "pm:transfer")],
        vec![inline(lang, Button::BothPayments, "pm:both")],
    ])
}

/// Under the offer in the channel: Accept is a callback (first one wins);
/// Counteroffer opens the private chat with the bot.
pub fn accept(lang: Lang, request: &RequestId, bot_username: &str) -> InlineKeyboardMarkup {
    let mut row = vec![inline(lang, Button::Accept, format!("acc:{request}"))];
    if let Ok(url) = reqwest::Url::parse(&format!("https://t.me/{bot_username}?start=co_{request}")) {
        row.push(InlineKeyboardButton::url(button(lang, Button::Counteroffer), url));
    }
    InlineKeyboardMarkup::new(vec![row])
}

/// For the client while the request is open.
pub fn open_request(lang: Lang, request: &RequestId) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        inline(lang, Button::RaisePrice, format!("up:{request}")),
        inline(lang, Button::CancelTrip, format!("can:{request}")),
    ]])
}

/// "Still waiting?" at 30 minutes.
pub fn still_waiting(lang: Lang, request: &RequestId) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![inline(lang, Button::KeepWaiting, format!("kw:{request}"))],
        vec![
            inline(lang, Button::RaisePrice, format!("up:{request}")),
            inline(lang, Button::CancelTrip, format!("can:{request}")),
        ],
    ])
}

/// New prices the client can raise to.
pub fn raise_options(request: &RequestId, price: Cup) -> InlineKeyboardMarkup {
    let row = [50, 100, 150]
        .into_iter()
        .map(|step| InlineKeyboardButton::callback(format!("{} CUP", price + step), format!("rai:{request}:{}", price + step)))
        .collect::<Vec<_>>();
    InlineKeyboardMarkup::new(vec![row])
}

/// For the client when a driver accepted: confirm them or cancel. The card
/// names its driver, so a stale one (that driver withdrew, maybe another
/// took the request) cannot cancel the request or confirm someone unseen.
pub fn confirm_driver(lang: Lang, request: &RequestId, driver: UserId) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        inline(lang, Button::Confirm, format!("cok:{request}:{}", driver.0)),
        inline(lang, Button::CancelTrip, format!("cno:{request}:{}", driver.0)),
    ]])
}

/// Amounts a driver can counteroffer, above the client's price.
pub fn counteroffer_amounts(request: &RequestId, price: Cup) -> InlineKeyboardMarkup {
    let row = [50, 100, 150]
        .into_iter()
        .map(|step| InlineKeyboardButton::callback(format!("{} CUP", price + step), format!("cof:{request}:{}", price + step)))
        .collect::<Vec<_>>();
    InlineKeyboardMarkup::new(vec![row])
}

/// Under one counteroffer: accept it or reject it.
pub fn offer(lang: Lang, request: &RequestId, driver: UserId, amount: Cup) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback(
            format!("{} {amount} CUP", button(lang, Button::Accept)),
            format!("coa:{request}:{}", driver.0),
        ),
        inline(lang, Button::Reject, format!("cor:{request}:{}", driver.0)),
    ]])
}

/// For the driver waiting for the client's confirmation.
pub fn awaiting_client(lang: Lang, request: &RequestId) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![inline(lang, Button::Withdraw, format!("wd:{request}"))]])
}

pub fn republish(lang: Lang) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![inline(lang, Button::Republish, "rep")]])
}

/// For both sides once the trip is assigned.
pub fn assigned_trip(lang: Lang, request: &RequestId) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        inline(lang, Button::Complete, format!("fin:{request}")),
        inline(lang, Button::CancelTrip, format!("can:{request}")),
    ]])
}

pub fn review_application(lang: Lang, driver: UserId) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        inline(lang, Button::Approve, format!("dok:{}", driver.0)),
        inline(lang, Button::Reject, format!("dno:{}", driver.0)),
    ]])
}

pub fn edit_profile(lang: Lang) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![inline(lang, Button::EditProfile, "dedit")]])
}
