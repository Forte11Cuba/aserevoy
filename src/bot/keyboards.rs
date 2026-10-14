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

/// Under the offer in the channel.
pub fn accept(lang: Lang, request: &RequestId) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![inline(lang, Button::Accept, format!("acc:{request}"))]])
}

/// For the client while the request is open.
pub fn open_request(lang: Lang, request: &RequestId) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![inline(lang, Button::CancelTrip, format!("can:{request}"))]])
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
