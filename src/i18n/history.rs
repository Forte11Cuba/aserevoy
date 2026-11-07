//! Trip history and stats.

use crate::domain::fare::Cup;
use crate::domain::reputation::{Reputation, Role, Tag};
use crate::domain::request::{PaymentMethod, UnixTime};
use crate::domain::vehicle::VehicleKind;
use super::{Lang, days_using, payment, tag};

/// One trip of "📅 My trips".
pub struct HistoryLine<'a> {
    pub number: i64,
    pub request: &'a str,
    pub when: String,
    pub origin: &'a str,
    pub destination: &'a str,
    pub price: Cup,
    pub pay: PaymentMethod,
    /// Empty when the client gave no name.
    pub counterpart: &'a str,
    pub stars: Option<i64>,
    pub kind: VehicleKind,
}

/// "📅 My trips": a page of trips, the most recent first, with the user's
/// trip number and the request id.
pub fn history(lang: Lang, role: Role, trips: &[HistoryLine], total: i64) -> String {
    let title = match (lang, role) {
        (Lang::Es, Role::Client) => "📅 Tus viajes como cliente",
        (Lang::En, Role::Client) => "📅 Your trips as a client",
        (Lang::Es, Role::Driver) => "📅 Tus viajes como chofer",
        (Lang::En, Role::Driver) => "📅 Your trips as a driver",
    };
    if trips.is_empty() {
        return match lang {
            Lang::Es => format!("{title}\n\nNo tienes viajes en los últimos 45 días."),
            Lang::En => format!("{title}\n\nNo trips in the last 45 days."),
        };
    }
    let mut text = match lang {
        Lang::Es => format!("{title} ({total}, últimos 45 días)"),
        Lang::En => format!("{title} ({total}, last 45 days)"),
    };
    for t in trips {
        let who = match (lang, role, t.counterpart.is_empty()) {
            (_, _, false) => t.counterpart.to_owned(),
            (Lang::Es, Role::Driver, true) => "Cliente".to_owned(),
            (Lang::En, Role::Driver, true) => "Client".to_owned(),
            (Lang::Es, Role::Client, true) => "Chofer".to_owned(),
            (Lang::En, Role::Client, true) => "Driver".to_owned(),
        };
        let stars = match (lang, t.stars) {
            (Lang::Es, Some(n)) => format!(" · tu nota: {}", "⭐".repeat(n.clamp(0, 5) as usize)),
            (Lang::En, Some(n)) => format!(" · your rating: {}", "⭐".repeat(n.clamp(0, 5) as usize)),
            (_, None) => String::new(),
        };
        let money = match (lang, role) {
            (Lang::Es, Role::Client) => format!("💰 Pagaste {} CUP · {}", t.price, payment(lang, t.pay)),
            (Lang::En, Role::Client) => format!("💰 You paid {} CUP · {}", t.price, payment(lang, t.pay)),
            (Lang::Es, Role::Driver) => format!("💰 Cobraste {} CUP · {}", t.price, payment(lang, t.pay)),
            (Lang::En, Role::Driver) => format!("💰 You charged {} CUP · {}", t.price, payment(lang, t.pay)),
        };
        let trip = match lang {
            Lang::Es => "Viaje",
            Lang::En => "Trip",
        };
        text.push_str(&format!(
            "\n\n{} {trip} nº {} · {} · {}\n📍 {}\n🏁 {}\n{money}\n👤 {who}{stars}",
            t.kind.emoji(),
            t.number,
            t.request,
            t.when,
            t.origin,
            t.destination
        ));
    }
    text
}

/// "⭐ My reputation" (🔧 Settings): the same as a client, with the
/// cancellations that count against them.
pub fn client_stats(lang: Lang, rep: Option<&Reputation>, tags: &[(Tag, i64)], cancellations: i64, now: UnixTime) -> String {
    stats(lang, Role::Client, rep, tags, Some(cancellations), now)
}

pub(super) fn stats(
    lang: Lang,
    role: Role,
    rep: Option<&Reputation>,
    tags: &[(Tag, i64)],
    cancellations: Option<i64>,
    now: UnixTime,
) -> String {
    let title = match (lang, role) {
        (Lang::Es, Role::Driver) => "📊 Tus estadísticas como chofer",
        (Lang::En, Role::Driver) => "📊 Your stats as a driver",
        (Lang::Es, Role::Client) => "⭐ Tu reputación como cliente",
        (Lang::En, Role::Client) => "⭐ Your reputation as a client",
    };
    let Some(rep) = rep else {
        return match lang {
            Lang::Es => format!("{title}\n\nAún no tienes viajes."),
            Lang::En => format!("{title}\n\nNo trips yet."),
        };
    };
    let stars = match (lang, rep.average()) {
        (Lang::Es, Some(avg)) => format!("⭐ {avg:.1} de 5 ({} calificaciones)", rep.stars_count),
        (Lang::En, Some(avg)) => format!("⭐ {avg:.1} out of 5 ({} ratings)", rep.stars_count),
        (Lang::Es, None) => "⭐ Aún sin calificaciones".into(),
        (Lang::En, None) => "⭐ No ratings yet".into(),
    };
    let trips = match lang {
        Lang::Es => format!("🏁 {} viajes completados", rep.trips),
        Lang::En => format!("🏁 {} trips completed", rep.trips),
    };
    let mut text = format!("{title}\n\n{stars}\n{trips}\n📅 {}", days_using(lang, rep.shown(now).days));
    if let Some(n) = cancellations {
        text.push_str(&match lang {
            Lang::Es => format!("\n❌ Cancelaciones (30 días): {n}"),
            Lang::En => format!("\n❌ Cancellations (30 days): {n}"),
        });
    }
    let tagged: Vec<String> = tags.iter().filter(|(_, n)| *n > 0).map(|(t, n)| format!("{} × {n}", tag(lang, *t))).collect();
    if !tagged.is_empty() {
        text.push_str(match (lang, role) {
            (Lang::Es, Role::Driver) => "\n\nLo que dicen los clientes:\n",
            (Lang::En, Role::Driver) => "\n\nWhat clients say:\n",
            (Lang::Es, Role::Client) => "\n\nLo que dicen los choferes:\n",
            (Lang::En, Role::Client) => "\n\nWhat drivers say:\n",
        });
        text.push_str(&tagged.join("\n"));
    }
    text
}
