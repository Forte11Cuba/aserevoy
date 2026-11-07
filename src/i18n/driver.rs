//! Driver profile, application, channel access and stats.

use crate::domain::driver::{DriverStatus, Profile};
use crate::domain::fare::Cup;
use crate::domain::reputation::{Reputation, Role, Tag};
use crate::domain::request::UnixTime;
use crate::domain::vehicle::Vehicle;
use super::{Lang, vehicle_lines};
use super::history::stats;

pub fn profile_card(lang: Lang, p: &Profile, status: Option<DriverStatus>) -> String {
    let status = status.map(|s| match (lang, s) {
        (Lang::Es, DriverStatus::Pending) => "⏳ Pendiente de aprobación",
        (Lang::En, DriverStatus::Pending) => "⏳ Waiting for approval",
        (Lang::Es, DriverStatus::Active) => "🟢 Activo",
        (Lang::En, DriverStatus::Active) => "🟢 Active",
        (Lang::Es, DriverStatus::Blocked) => "⛔ Bloqueado",
        (Lang::En, DriverStatus::Blocked) => "⛔ Blocked",
    });
    let name = p.full_name.as_deref().map(|n| format!("\n🪪 {n}")).unwrap_or_default();
    let mut card = match lang {
        Lang::Es => format!("👤 {}{name}\n📞 {}", p.alias, p.phone),
        Lang::En => format!("👤 {}{name}\n📞 {}", p.alias, p.phone),
    };
    if let Some(status) = status {
        card.push('\n');
        card.push_str(status);
    }
    card
}

/// Under the driver's own profile: the name waiting for approval.
pub fn name_pending(lang: Lang, name: &str) -> String {
    match lang {
        Lang::Es => format!("\n⏳ Nombre en revisión: {name}"),
        Lang::En => format!("\n⏳ Name under review: {name}"),
    }
}

/// To staff: a driver's new full name, to approve or reject.
pub fn name_change(lang: Lang, alias: &str, old: Option<&str>, new: &str) -> String {
    match (lang, old) {
        (Lang::Es, Some(old)) => format!("🪪 {alias} quiere cambiar su nombre completo:\n{old} → {new}"),
        (Lang::Es, None) => format!("🪪 {alias} añadió su nombre completo: {new}"),
        (Lang::En, Some(old)) => format!("🪪 {alias} wants to change their full name:\n{old} → {new}"),
        (Lang::En, None) => format!("🪪 {alias} added their full name: {new}"),
    }
}

pub fn photo_changed(lang: Lang, alias: &str) -> String {
    match lang {
        Lang::Es => format!("📷 {alias} cambió su foto. Esta es la nueva."),
        Lang::En => format!("📷 {alias} changed their photo. This is the new one."),
    }
}

/// "📊 My stats": what the bot keeps about a driver, all aggregate.
/// A driver's stats, with what they made per month (the last 6) and this
/// year. `months` are `(YYYY-MM, trips, earned)`, the most recent first;
/// `this_month` is the current `YYYY-MM`.
pub fn driver_stats(
    lang: Lang,
    rep: Option<&Reputation>,
    tags: &[(Tag, i64)],
    months: &[(String, i64, Cup)],
    this_month: &str,
    now: UnixTime,
) -> String {
    let mut text = stats(lang, Role::Driver, rep, tags, None, now);
    let year = &this_month[..4.min(this_month.len())];
    let (year_trips, year_earned) =
        months.iter().filter(|(m, _, _)| m.starts_with(year)).fold((0, 0), |(t, e), (_, mt, me)| (t + mt, e + me));
    text.push_str(match lang {
        Lang::Es => "\n\n💰 Lo que hiciste por mes:",
        Lang::En => "\n\n💰 What you made per month:",
    });
    if !months.iter().any(|(m, _, _)| m == this_month) {
        text.push_str(&format!("\n{}: 0 · 0 CUP", month_name(lang, this_month)));
    }
    for (month, trips, earned) in months.iter().take(6) {
        let trips = match (lang, trips) {
            (Lang::Es, 1) => "1 viaje".to_owned(),
            (Lang::Es, n) => format!("{n} viajes"),
            (Lang::En, 1) => "1 trip".to_owned(),
            (Lang::En, n) => format!("{n} trips"),
        };
        text.push_str(&format!("\n{}: {trips} · {earned} CUP", month_name(lang, month)));
    }
    text.push_str(&match lang {
        Lang::Es => format!("\n\n📆 En {year}: {year_trips} viajes · {year_earned} CUP"),
        Lang::En => format!("\n\n📆 In {year}: {year_trips} trips · {year_earned} CUP"),
    });
    text
}

/// "2026-10" → "octubre 2026".
fn month_name(lang: Lang, month: &str) -> String {
    const ES: [&str; 12] =
        ["enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre", "octubre", "noviembre", "diciembre"];
    const EN: [&str; 12] =
        ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    let (year, m) = month.split_once('-').unwrap_or((month, ""));
    match m.parse::<usize>().ok().filter(|m| (1..=12).contains(m)) {
        Some(m) if lang == Lang::Es => format!("{} {year}", ES[m - 1]),
        Some(m) => format!("{} {year}", EN[m - 1]),
        None => month.to_owned(),
    }
}

/// To admins: a new driver application to approve or reject.
pub fn application(lang: Lang, p: &Profile, vehicle: Option<&Vehicle>, invited: bool) -> String {
    let mut card = profile_card(lang, p, None);
    if let Some(v) = vehicle {
        card.push_str(&format!("\n{}", vehicle_lines(lang, v.kind, Some(v))));
    }
    match (lang, invited) {
        (Lang::Es, true) => format!("🆕 Solicitud de chofer (por invitación)\n\n{card}"),
        (Lang::Es, false) => format!("🆕 Solicitud de chofer\n\n{card}"),
        (Lang::En, true) => format!("🆕 Driver application (invited)\n\n{card}"),
        (Lang::En, false) => format!("🆕 Driver application\n\n{card}"),
    }
}

/// "📢 Offers channel" for a driver already in it: the link again, in case
/// they can't find the channel among their chats.
pub fn in_channel(lang: Lang, channel_link: &str) -> String {
    match lang {
        Lang::Es => format!(
            "✅ Ya estás en el canal de ofertas. Ahí te llegan las solicitudes de viaje.\n\nSi no lo encuentras entre tus chats, ábrelo aquí: {channel_link}"
        ),
        Lang::En => format!(
            "✅ You're already in the offers channel. Trip requests reach you there.\n\nIf you can't find it among your chats, open it here: {channel_link}"
        ),
    }
}

/// "📢 Offers channel" for an active driver not in it yet.
pub fn channel_invite(lang: Lang, channel_link: &str) -> String {
    match lang {
        Lang::Es => format!(
            "📢 Aún no estás en el canal de ofertas, y ahí es donde llegan los viajes.\n\nÚnete aquí: {channel_link}\nCuando pidas unirte, te apruebo enseguida."
        ),
        Lang::En => format!(
            "📢 You're not in the offers channel yet, and that's where trips arrive.\n\nJoin here: {channel_link}\nWhen you ask to join, I'll approve you right away."
        ),
    }
}

pub fn approved(lang: Lang, channel_link: &str) -> String {
    match lang {
        Lang::Es => format!(
            "🎉 ¡Ya eres chofer de Asere Voy!\n\nÚnete al canal de ofertas: {channel_link}\nCuando pidas unirte, te apruebo enseguida."
        ),
        Lang::En => format!(
            "🎉 You're now an Asere Voy driver!\n\nJoin the offers channel: {channel_link}\nWhen you ask to join, I'll approve you right away."
        ),
    }
}

pub fn invite_created(lang: Lang, link: &str) -> String {
    match lang {
        Lang::Es => format!("🔗 Enlace de invitación para un chofer (un solo uso, vence en 48 h):\n{link}"),
        Lang::En => format!("🔗 Driver invitation link (single use, expires in 48 h):\n{link}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drivers_see_what_they_made_per_month_and_year() {
        let months = vec![("2026-10".to_owned(), 2, 1200), ("2026-09".to_owned(), 1, 1000), ("2025-12".to_owned(), 4, 9000)];
        let text = driver_stats(Lang::Es, None, &[], &months, "2026-10", 0);
        assert!(text.contains("octubre 2026: 2 viajes · 1200 CUP\nseptiembre 2026: 1 viaje · 1000 CUP"), "{text}");
        assert!(text.contains("📆 En 2026: 3 viajes · 2200 CUP"), "{text}");
    }
}
