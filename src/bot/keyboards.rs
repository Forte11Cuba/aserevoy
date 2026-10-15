//! Keyboards. Callback data stays under Telegram's 64-byte limit by using
//! short prefixes and ids (`acc:AB7K`).

use teloxide::types::{ButtonRequest, InlineKeyboardButton, InlineKeyboardMarkup, KeyboardButton, KeyboardMarkup};

use crate::domain::fare::Cup;
use crate::domain::driver::DriverStatus;
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

pub fn main_menu(lang: Lang, is_driver: bool, is_staff: bool) -> KeyboardMarkup {
    let profile = if is_driver { Button::DriverMenu } else { Button::WorkWithUs };
    let mut rows = vec![vec![reply(lang, Button::RequestRide)], vec![reply(lang, profile)]];
    if is_staff {
        rows.push(vec![reply(lang, Button::AdminPanel)]);
    }
    rows.push(vec![reply(lang, Button::HowItWorks)]);
    rows.push(vec![reply(lang, Button::Support), reply(lang, Button::Settings)]);
    KeyboardMarkup::new(rows).resize_keyboard().persistent()
}

/// While a form runs: optional extra buttons, then Back (to the previous
/// step) on the left and Cancel (the whole form) on the right.
pub fn step(lang: Lang, extra: &[KeyboardButton]) -> KeyboardMarkup {
    let mut rows: Vec<Vec<KeyboardButton>> = extra.iter().cloned().map(|b| vec![b]).collect();
    rows.push(vec![reply(lang, Button::StepBack), reply(lang, Button::Cancel)]);
    KeyboardMarkup::new(rows).resize_keyboard()
}

/// Under the summary: Back on the left, Publish on the right.
pub fn summary(lang: Lang) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![inline(lang, Button::StepBack, "sback"), inline(lang, Button::Publish, "pub")]])
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
    // The same steps as a driver's counteroffer.
    let step = |delta: Cup| InlineKeyboardButton::callback(i18n::amount_step(delta), format!("padj:{delta}"));
    InlineKeyboardMarkup::new(vec![
        vec![step(-100), step(-1000), step(-5000), step(-10000)],
        vec![step(100), step(1000), step(5000), step(10000)],
        vec![InlineKeyboardButton::callback(i18n::offer_button(lang, offer), "pok")],
    ])
}

/// One method per row, so the labels read well on a phone.
pub fn payment(lang: Lang) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![inline(lang, Button::Cash, "pay:cash")],
        vec![inline(lang, Button::Transfer, "pay:transfer")],
        vec![inline(lang, Button::Bitcoin, "pay:bitcoin")],
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

/// Steps for the counteroffer: each tap adds to (or takes from) the amount
/// so far; it never goes below the client's price.
pub fn counteroffer_steps(request: &RequestId) -> InlineKeyboardMarkup {
    let step = |delta: Cup| InlineKeyboardButton::callback(i18n::amount_step(delta), format!("cad:{request}:{delta}"));
    InlineKeyboardMarkup::new(vec![
        vec![step(-100), step(-1000), step(-5000), step(-10000)],
        vec![step(100), step(1000), step(5000), step(10000)],
    ])
}

/// Below, while a driver makes a counteroffer: send the amount, or leave.
pub fn counteroffer_send(lang: Lang) -> KeyboardMarkup {
    KeyboardMarkup::new(vec![vec![reply(lang, Button::SendOffer), reply(lang, Button::Cancel)]]).resize_keyboard()
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
/// One button per place: its number, then municipality, reparto and the
/// rest ("1 · Playa · Miramar · 12 entre …"). Telegram shows a single line
/// and cuts the end, so what tells them apart goes first; the message lists
/// them in full.
pub fn candidates(lang: Lang, shorts: &[&str]) -> InlineKeyboardMarkup {
    let mut rows: Vec<Vec<_>> = shorts
        .iter()
        .enumerate()
        .map(|(i, short)| vec![InlineKeyboardButton::callback(format!("{} · {short}", i + 1), format!("cand:{i}"))])
        .collect();
    rows.push(vec![inline(lang, Button::NoneOfThese, "cand:none")]);
    InlineKeyboardMarkup::new(rows)
}

/// Repartos per page of the list (two per row).
pub const REPARTOS_PER_PAGE: usize = 12;

/// One page of repartos to choose from, two per row, with ◀ / ▶ when there
/// are more pages. Buttons carry the index in the whole list.
pub fn repartos(lang: Lang, names: &[&str], page: usize) -> InlineKeyboardMarkup {
    let pages = names.len().div_ceil(REPARTOS_PER_PAGE).max(1);
    let page = page.min(pages - 1);
    let start = page * REPARTOS_PER_PAGE;
    let buttons: Vec<_> = names
        .iter()
        .enumerate()
        .skip(start)
        .take(REPARTOS_PER_PAGE)
        .map(|(i, n)| InlineKeyboardButton::callback(*n, format!("zone:{i}")))
        .collect();
    let mut rows: Vec<Vec<_>> = buttons.chunks(2).map(<[_]>::to_vec).collect();
    if pages > 1 {
        let mut nav = Vec::new();
        if page > 0 {
            nav.push(inline(lang, Button::PrevPage, format!("zpg:{}", page - 1)));
        }
        nav.push(InlineKeyboardButton::callback(format!("{}/{pages}", page + 1), format!("zpg:{page}")));
        if page + 1 < pages {
            nav.push(inline(lang, Button::NextPage, format!("zpg:{}", page + 1)));
        }
        rows.push(nav);
    }
    rows.push(vec![inline(lang, Button::StepBack, "dback")]);
    InlineKeyboardMarkup::new(rows)
}

/// The municipalities, two per row, for a description without a pin, and
/// Back (to the choices, or to typing the address).
pub fn municipalities(lang: Lang, names: &[&str]) -> InlineKeyboardMarkup {
    let buttons: Vec<_> =
        names.iter().enumerate().map(|(i, n)| InlineKeyboardButton::callback(*n, format!("mun:{i}"))).collect();
    let mut rows: Vec<Vec<_>> = buttons.chunks(2).map(<[_]>::to_vec).collect();
    rows.push(vec![inline(lang, Button::StepBack, "dback")]);
    InlineKeyboardMarkup::new(rows)
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

/// ⚙️ Administration as the bottom keyboard: drivers and invitations for
/// every staff member; admins, fares and metrics for superadmins; and back to
/// the main menu. Each section answers with a message of its own.
pub fn admin_menu(lang: Lang, superadmin: bool) -> KeyboardMarkup {
    let mut rows = vec![vec![reply(lang, Button::PanelDrivers), reply(lang, Button::InviteDriver)]];
    if superadmin {
        rows.push(vec![reply(lang, Button::PanelAdmins), reply(lang, Button::PanelFares)]);
        rows.push(vec![reply(lang, Button::PanelMetrics)]);
    }
    rows.push(vec![reply(lang, Button::MainMenu)]);
    KeyboardMarkup::new(rows).resize_keyboard().persistent()
}

/// The drivers list: one button per driver, the filters and the pages.
pub fn drivers_list(
    lang: Lang,
    drivers: &[crate::db::drivers::Listed],
    filter: crate::bot::panel::Filter,
    page: i64,
    pages: i64,
) -> InlineKeyboardMarkup {
    use crate::bot::panel::Filter;
    let mut rows: Vec<Vec<InlineKeyboardButton>> = drivers
        .chunks(2)
        .map(|pair| {
            pair.iter()
                .map(|d| {
                    let label = format!("{} {}", i18n::status_mark(d.status), d.alias);
                    InlineKeyboardButton::callback(label, format!("ap:d:{}", d.id.0))
                })
                .collect()
        })
        .collect();
    let code = filter.code();
    if pages > 1 {
        let mut nav = Vec::new();
        if page > 0 {
            nav.push(inline(lang, Button::PrevPage, format!("ap:drv:{code}:{}", page - 1)));
        }
        nav.push(InlineKeyboardButton::callback(format!("{}/{pages}", page + 1), format!("ap:drv:{code}:{page}")));
        if page + 1 < pages {
            nav.push(inline(lang, Button::NextPage, format!("ap:drv:{code}:{}", page + 1)));
        }
        rows.push(nav);
    }
    let filter_button = |f: Filter, b: Button| {
        let mark = if f == filter { "• " } else { "" };
        InlineKeyboardButton::callback(format!("{mark}{}", button(lang, b)), format!("ap:drv:{}:0", f.code()))
    };
    rows.push(vec![filter_button(Filter::All, Button::FilterAll), filter_button(Filter::Active, Button::FilterActive)]);
    rows.push(vec![
        filter_button(Filter::Pending, Button::FilterPending),
        filter_button(Filter::Blocked, Button::FilterBlocked),
    ]);
    InlineKeyboardMarkup::new(rows)
}

/// Under a driver's card in the panel: what fits their status, and back.
pub fn driver_admin(lang: Lang, driver: UserId, status: DriverStatus) -> InlineKeyboardMarkup {
    let id = driver.0;
    let mut rows = vec![match status {
        DriverStatus::Pending => {
            vec![inline(lang, Button::Approve, format!("dok:{id}")), inline(lang, Button::Reject, format!("dno:{id}"))]
        }
        DriverStatus::Active => vec![
            inline(lang, Button::Block, format!("adm:bl:{id}")),
            inline(lang, Button::RemoveDriver, format!("adm:rd:{id}")),
        ],
        DriverStatus::Blocked => vec![
            inline(lang, Button::Unblock, format!("adm:ub:{id}")),
            inline(lang, Button::RemoveDriver, format!("adm:rd:{id}")),
        ],
    }];
    rows.push(vec![inline(lang, Button::ToList, "ap:drv:all:0")]);
    InlineKeyboardMarkup::new(rows)
}

/// Steps to change each fare.
pub fn fares_panel(lang: Lang) -> InlineKeyboardMarkup {
    let step = |key: &str, delta: i64, unit: &str| {
        let label = if delta > 0 { format!("+{delta}{unit}") } else { format!("−{}{unit}", -delta) };
        InlineKeyboardButton::callback(label, format!("ap:fa:{key}:{delta}"))
    };
    let title = |text: String| InlineKeyboardButton::callback(text, "ap:fare");
    let (base, km, min) = match lang {
        Lang::Es => ("Base", "Por km", "Mínimo"),
        Lang::En => ("Base", "Per km", "Minimum"),
    };
    InlineKeyboardMarkup::new(vec![
        vec![title(base.into())],
        vec![step("base", -50, ""), step("base", -10, ""), step("base", 10, ""), step("base", 50, "")],
        vec![title(km.into())],
        vec![step("km", -50, ""), step("km", -10, ""), step("km", 10, ""), step("km", 50, "")],
        vec![title(min.into())],
        vec![step("minimo", -5, "%"), step("minimo", 5, "%")],
    ])
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

/// "🏍 Driver" below: stats and profile, and back to the main menu.
pub fn driver_menu(lang: Lang) -> KeyboardMarkup {
    KeyboardMarkup::new(vec![
        vec![reply(lang, Button::MyStats), reply(lang, Button::MyProfile)],
        vec![reply(lang, Button::MainMenu)],
    ])
    .resize_keyboard()
    .persistent()
}

/// Under the driver's profile: one button per field (`df:<field>`).
pub fn profile_fields(lang: Lang) -> InlineKeyboardMarkup {
    let field = |b: Button, code: &str| inline(lang, b, format!("df:{code}"));
    InlineKeyboardMarkup::new(vec![
        vec![field(Button::FieldAlias, "al"), field(Button::FieldFullName, "fn"), field(Button::FieldPhone, "ph")],
        vec![field(Button::FieldPlate, "pl"), field(Button::FieldModel, "bm"), field(Button::FieldColor, "bc")],
        vec![field(Button::FieldPhoto, "pp"), field(Button::FieldBikePhoto, "bp")],
    ])
}

/// Under "❓ How it works": one button per guide, the open one marked.
pub fn guides(lang: Lang, open: Option<i18n::Guide>) -> InlineKeyboardMarkup {
    let rows = i18n::Guide::ALL
        .into_iter()
        .map(|g| {
            let label = i18n::guide_button(lang, g);
            let label = if Some(g) == open { format!("• {label}") } else { label.to_owned() };
            vec![InlineKeyboardButton::callback(label, format!("help:{}", g.code()))]
        })
        .collect::<Vec<_>>();
    InlineKeyboardMarkup::new(rows)
}

/// "🔧 Settings": my details and the language.
pub fn settings(lang: Lang) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![inline(lang, Button::MyDetails, "set:me")],
        vec![inline(lang, Button::Language, "set:lang")],
    ])
}

/// Under "👤 My details": a button per detail, and delete them all.
pub fn client_details(lang: Lang, any: bool) -> InlineKeyboardMarkup {
    let mut rows = vec![vec![
        inline(lang, Button::DetailName, "set:name"),
        inline(lang, Button::DetailPhone, "set:phone"),
        inline(lang, Button::DetailPhoto, "set:photo"),
    ]];
    if any {
        rows.push(vec![inline(lang, Button::DeleteDetails, "set:del")]);
    }
    InlineKeyboardMarkup::new(rows)
}

/// "🤝 Work with us": apply as a driver, or as a manager (coming soon).
pub fn work_with_us(lang: Lang) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![inline(lang, Button::ApplyDriver, "work:drv")],
        vec![inline(lang, Button::ApplyManager, "work:mgr")],
    ])
}

/// For staff: approve or reject a driver's new full name.
pub fn review_name(lang: Lang, driver: UserId) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        inline(lang, Button::Approve, format!("dna:{}", driver.0)),
        inline(lang, Button::Reject, format!("dnr:{}", driver.0)),
    ]])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn callbacks(markup: &InlineKeyboardMarkup) -> Vec<String> {
        markup
            .inline_keyboard
            .iter()
            .flatten()
            .filter_map(|b| match &b.kind {
                teloxide::types::InlineKeyboardButtonKind::CallbackData(data) => Some(data.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn repartos_come_in_pages_of_twelve() {
        let names: Vec<String> = (0..65).map(|i| format!("Reparto {i}")).collect();
        let names: Vec<&str> = names.iter().map(String::as_str).collect();

        let first = callbacks(&repartos(Lang::Es, &names, 0));
        assert_eq!(first.iter().filter(|c| c.starts_with("zone:")).count(), 12);
        assert!(first.contains(&"zpg:1".to_owned()) && !first.iter().any(|c| c == "zpg:-1"));

        // The last page has the remaining 5, and points back.
        let last = callbacks(&repartos(Lang::Es, &names, 5));
        assert_eq!(last.iter().filter(|c| c.starts_with("zone:")).count(), 5);
        assert!(last.contains(&"zone:64".to_owned()) && last.contains(&"zpg:4".to_owned()));

        // A small municipality: one page, no navigation, and Back.
        let few = callbacks(&repartos(Lang::Es, &names[..6], 0));
        assert_eq!(few.len(), 7);
        assert_eq!(few.last().map(String::as_str), Some("dback"));
    }
}
