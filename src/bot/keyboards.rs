//! Keyboards. Callback data stays under Telegram's 64-byte limit by using
//! short prefixes and ids (`acc:AB7K`).

use teloxide::types::{ButtonRequest, InlineKeyboardButton, InlineKeyboardMarkup, KeyboardButton, KeyboardMarkup};

use crate::domain::fare::Cup;
use crate::domain::ids::{RequestId, UserId};
use crate::domain::reputation::Tag;
use crate::domain::request::CancelReason;
use crate::i18n::{self, Button, Lang, button};

fn reply(lang: Lang, b: Button) -> KeyboardButton {
    KeyboardButton::new(button(lang, b))
}

fn inline(lang: Lang, b: Button, data: impl Into<String>) -> InlineKeyboardButton {
    InlineKeyboardButton::callback(button(lang, b), data.into())
}

pub fn main_menu(lang: Lang, is_driver: bool, is_staff: bool, is_superadmin: bool) -> KeyboardMarkup {
    let profile = if is_driver { Button::MyProfile } else { Button::ApplyDriver };
    let mut rows = vec![vec![reply(lang, Button::RequestRide)], vec![reply(lang, profile)]];
    if is_staff {
        let mut row = vec![reply(lang, Button::InviteDriver)];
        if is_superadmin {
            row.push(reply(lang, Button::Admins));
        }
        rows.push(row);
    }
    rows.push(vec![reply(lang, Button::Support), reply(lang, Button::SwitchLang)]);
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

/// Steps to adjust the offer, and the button to make it.
pub fn price_adjust(lang: Lang, offer: Cup) -> InlineKeyboardMarkup {
    let step = |delta: Cup| {
        let label = if delta > 0 { format!("+{delta}") } else { format!("−{}", -delta) };
        InlineKeyboardButton::callback(label, format!("padj:{delta}"))
    };
    InlineKeyboardMarkup::new(vec![
        vec![step(-1000), step(-500), step(-100)],
        vec![step(100), step(500), step(1000)],
        vec![InlineKeyboardButton::callback(i18n::offer_button(lang, offer), "pok")],
    ])
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

/// Under one counteroffer: accept it or reject it (the amount is in the message).
pub fn offer(lang: Lang, request: &RequestId, driver: UserId) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        inline(lang, Button::Accept, format!("coa:{request}:{}", driver.0)),
        inline(lang, Button::Reject, format!("cor:{request}:{}", driver.0)),
    ]])
}

/// For the driver waiting for the client's confirmation.
pub fn awaiting_client(lang: Lang, request: &RequestId) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![inline(lang, Button::Withdraw, format!("wd:{request}"))]])
}

/// The places found for a written address, and "none of these".
pub fn candidates(lang: Lang, labels: &[&str]) -> InlineKeyboardMarkup {
    let mut rows: Vec<_> = labels
        .iter()
        .enumerate()
        .map(|(i, label)| vec![InlineKeyboardButton::callback(*label, format!("cand:{i}"))])
        .collect();
    rows.push(vec![inline(lang, Button::NoneOfThese, "cand:none")]);
    InlineKeyboardMarkup::new(rows)
}

/// The municipalities, two per row, for a description without a pin.
pub fn municipalities(names: &[&str]) -> InlineKeyboardMarkup {
    let buttons: Vec<_> =
        names.iter().enumerate().map(|(i, n)| InlineKeyboardButton::callback(*n, format!("mun:{i}"))).collect();
    InlineKeyboardMarkup::new(buttons.chunks(2).map(<[_]>::to_vec).collect::<Vec<_>>())
}

pub fn republish(lang: Lang) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![inline(lang, Button::Republish, "rep")]])
}

/// For the client once the trip is assigned.
pub fn client_trip(lang: Lang, request: &RequestId) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            inline(lang, Button::OnMyWay, format!("omw:{request}")),
            inline(lang, Button::ShareTrip, format!("shr:{request}")),
        ],
        vec![
            inline(lang, Button::Complete, format!("fin:{request}")),
            inline(lang, Button::CancelTrip, format!("can:{request}")),
        ],
    ])
}

/// For the driver once the trip is assigned.
pub fn driver_trip(lang: Lang, request: &RequestId) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![inline(lang, Button::Arrived, format!("arr:{request}"))],
        vec![
            inline(lang, Button::Complete, format!("fin:{request}")),
            inline(lang, Button::CancelTrip, format!("can:{request}")),
        ],
    ])
}

/// Why cancel an assigned trip; replaces the trip's buttons until one is picked.
pub fn cancel_reasons(lang: Lang, request: &RequestId, reasons: &[CancelReason]) -> InlineKeyboardMarkup {
    let mut rows: Vec<_> = reasons
        .iter()
        .map(|r| vec![InlineKeyboardButton::callback(i18n::cancel_reason(lang, *r), format!("cr:{request}:{}", r.code()))])
        .collect();
    rows.push(vec![inline(lang, Button::Back, format!("cb:{request}"))]);
    InlineKeyboardMarkup::new(rows)
}

/// ⭐1 to ⭐5 for a pending rating.
pub fn stars(code: &str) -> InlineKeyboardMarkup {
    let row = (1..=5).map(|n| InlineKeyboardButton::callback("⭐".repeat(n), format!("rs:{code}:{n}"))).collect::<Vec<_>>();
    InlineKeyboardMarkup::new(vec![row[..3].to_vec(), row[3..].to_vec()])
}

/// Optional tags (✓ when chosen) and Done.
pub fn rating_tags(lang: Lang, code: &str, tags: &[Tag], chosen: i64) -> InlineKeyboardMarkup {
    let mut rows: Vec<_> = tags
        .iter()
        .map(|t| {
            let mark = if chosen & t.bit() != 0 { "✓ " } else { "" };
            vec![InlineKeyboardButton::callback(format!("{mark}{}", i18n::tag(lang, *t)), format!("rtg:{code}:{}", t.code()))]
        })
        .collect();
    rows.push(vec![inline(lang, Button::RatingDone, format!("rdn:{code}"))]);
    InlineKeyboardMarkup::new(rows)
}

/// Staff actions on a person (support code lookup, alert).
pub fn moderate(lang: Lang, user: UserId, is_driver: bool) -> InlineKeyboardMarkup {
    let mut rows = vec![
        vec![
            inline(lang, Button::Block24h, format!("adm:b24:{}", user.0)),
            inline(lang, Button::Block, format!("adm:bl:{}", user.0)),
        ],
        vec![inline(lang, Button::Unblock, format!("adm:ub:{}", user.0))],
    ];
    if is_driver {
        rows[1].push(inline(lang, Button::RemoveDriver, format!("adm:rd:{}", user.0)));
    }
    InlineKeyboardMarkup::new(rows)
}

/// Opens the chat with the human support account.
pub fn support_link(lang: Lang, username: &str) -> InlineKeyboardMarkup {
    let url = reqwest::Url::parse(&format!("https://t.me/{username}"));
    let rows = url.map(|url| vec![vec![InlineKeyboardButton::url(button(lang, Button::WriteSupport), url)]]);
    InlineKeyboardMarkup::new(rows.unwrap_or_default())
}

/// Under the @username guide: check again.
pub fn username_set(lang: Lang) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![inline(lang, Button::UsernameSet, "unm")]])
}

pub fn accept_terms(lang: Lang) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![inline(lang, Button::AcceptTerms, "terms")]])
}

/// The superadmin's list of admins: remove each, or invite a new one.
pub fn admins(lang: Lang, admins: &[(UserId, String)]) -> InlineKeyboardMarkup {
    let mut rows: Vec<_> = admins
        .iter()
        .map(|(id, name)| {
            vec![InlineKeyboardButton::callback(format!("{} {name}", button(lang, Button::RemoveAdmin)), format!("rma:{}", id.0))]
        })
        .collect();
    rows.push(vec![inline(lang, Button::InviteAdmin, "ina")]);
    InlineKeyboardMarkup::new(rows)
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
