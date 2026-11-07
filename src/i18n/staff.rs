//! Support, alerts and the admin panel.

use crate::db::metrics::Day;
use crate::domain::driver::{DriverStatus, Profile};
use crate::domain::fare::{FareTable, Fares};
use crate::domain::ids::RequestId;
use crate::domain::reputation::{Role, Shown};
use crate::domain::vehicle::{Vehicle, VehicleId, VehicleKind};
use super::{Lang, profile_card, reputation, reputation_inline, vehicle_button, vehicle_kind};

/// To staff: someone wants to be a manager (not available yet).
pub fn manager_interest(lang: Lang, username: Option<&str>) -> String {
    let who = username.map(|u| format!("@{u}")).unwrap_or_else(|| "?".into());
    match lang {
        Lang::Es => format!("🏢 {who} quiere ser gestor (próximamente). Escríbele cuando esté disponible."),
        Lang::En => format!("🏢 {who} wants to be a manager (coming soon). Write to them when it's available."),
    }
}

pub fn blocked(lang: Lang, until: Option<&str>) -> String {
    match (lang, until) {
        (Lang::Es, Some(t)) => format!("⛔ Tu cuenta está bloqueada hasta el {t}. Si crees que es un error, escribe a 💬 Soporte."),
        (Lang::En, Some(t)) => format!("⛔ Your account is blocked until {t}. If you think it's a mistake, write to 💬 Support."),
        (Lang::Es, None) => "⛔ Tu cuenta está bloqueada. Si crees que es un error, escribe a 💬 Soporte.".into(),
        (Lang::En, None) => "⛔ Your account is blocked. If you think it's a mistake, write to 💬 Support.".into(),
    }
}

/// 💬 Support: the support account to write to, and the code to include.
pub fn support(lang: Lang, username: &str, code: &str) -> String {
    match lang {
        Lang::Es => format!(
            "💬 Escribe a nuestro soporte, @{username}, e incluye tu código {code} para que podamos ayudarte. Si es sobre un viaje, añade también su código."
        ),
        Lang::En => format!(
            "💬 Message our support, @{username}, and include your code {code} so we can help you. If it's about a trip, add its code too."
        ),
    }
}

/// `/caso <code>`: who the person is, for staff.
pub fn case_summary(
    lang: Lang,
    code: &str,
    driver: Option<DriverStatus>,
    as_client: Option<&Shown>,
    as_driver: Option<&Shown>,
    blocked: Option<Option<String>>,
) -> String {
    let none = match lang {
        Lang::Es => "sin viajes",
        Lang::En => "no trips",
    };
    let rep = |s: Option<&Shown>| s.map(|s| reputation_inline(lang, s)).unwrap_or_else(|| none.into());
    let mut lines = vec![match lang {
        Lang::Es => format!("🔎 Código {code}"),
        Lang::En => format!("🔎 Code {code}"),
    }];
    lines.push(match lang {
        Lang::Es => format!("Como cliente: {}", rep(as_client)),
        Lang::En => format!("As a client: {}", rep(as_client)),
    });
    if let Some(status) = driver {
        let status = match (lang, status) {
            (Lang::Es, DriverStatus::Pending) => "pendiente",
            (Lang::En, DriverStatus::Pending) => "pending",
            (Lang::Es, DriverStatus::Active) => "activo",
            (Lang::En, DriverStatus::Active) => "active",
            (Lang::Es, DriverStatus::Blocked) => "bloqueado",
            (Lang::En, DriverStatus::Blocked) => "blocked",
        };
        lines.push(match lang {
            Lang::Es => format!("Chofer ({status}): {}", rep(as_driver)),
            Lang::En => format!("Driver ({status}): {}", rep(as_driver)),
        });
    }
    if let Some(until) = blocked {
        lines.push(match (lang, until) {
            (Lang::Es, Some(t)) => format!("⛔ Bloqueado hasta el {t}"),
            (Lang::En, Some(t)) => format!("⛔ Blocked until {t}"),
            (Lang::Es, None) => "⛔ Bloqueado".into(),
            (Lang::En, None) => "⛔ Blocked".into(),
        });
    }
    lines.join("\n")
}

pub fn alert_unsafe(lang: Lang, request: &RequestId) -> String {
    match lang {
        Lang::Es => format!("⚠️ Un chofer canceló el viaje {request} porque se sintió inseguro. Los botones actúan sobre el cliente."),
        Lang::En => format!("⚠️ A driver cancelled trip {request} because they felt unsafe. The buttons act on the client."),
    }
}

pub fn alert_cancellations(lang: Lang) -> String {
    match lang {
        Lang::Es => "⛔ Un cliente acumuló 3 cancelaciones en 7 días y quedó bloqueado 24 horas.".into(),
        Lang::En => "⛔ A client reached 3 cancellations in 7 days and is blocked for 24 hours.".into(),
    }
}

pub fn alert_low_rating(lang: Lang, role: Role, shown: &Shown) -> String {
    let rep = reputation_inline(lang, shown);
    match (lang, role) {
        (Lang::Es, Role::Client) => format!("📉 Un cliente bajó de 3.5 estrellas ({rep})."),
        (Lang::En, Role::Client) => format!("📉 A client fell below 3.5 stars ({rep})."),
        (Lang::Es, Role::Driver) => format!("📉 Un chofer bajó de 3.5 estrellas ({rep})."),
        (Lang::En, Role::Driver) => format!("📉 A driver fell below 3.5 stars ({rep})."),
    }
}

/// The drivers list header: which ones and how many.
pub fn drivers_list(lang: Lang, filter: crate::bot::panel::Filter, total: i64) -> String {
    use crate::bot::panel::Filter;
    let which = match (lang, filter) {
        (Lang::Es, Filter::All) => "todos",
        (Lang::En, Filter::All) => "all",
        (Lang::Es, Filter::Active) => "activos",
        (Lang::En, Filter::Active) => "active",
        (Lang::Es, Filter::Pending) => "pendientes de aprobar",
        (Lang::En, Filter::Pending) => "waiting for approval",
        (Lang::Es, Filter::Blocked) => "bloqueados",
        (Lang::En, Filter::Blocked) => "blocked",
    };
    match lang {
        Lang::Es => format!("🧢 Choferes ({which}): {total}\nToca uno para ver su ficha y actuar sobre él."),
        Lang::En => format!("🧢 Drivers ({which}): {total}\nTap one to see their profile and act on it."),
    }
}

pub fn status_mark(status: DriverStatus) -> &'static str {
    match status {
        DriverStatus::Pending => "⏳",
        DriverStatus::Active => "🟢",
        DriverStatus::Blocked => "⛔",
    }
}

/// A driver's card for staff: the profile (with phone), @username and reputation.
pub fn driver_admin_card(
    lang: Lang,
    p: &Profile,
    status: DriverStatus,
    shown: Option<&Shown>,
    username: Option<&str>,
) -> String {
    let mut card = profile_card(lang, p, Some(status));
    if let Some(u) = username {
        card.push_str(&format!("\n💬 @{u}"));
    }
    let rep = match (lang, shown) {
        (_, Some(s)) => reputation(lang, s),
        (Lang::Es, None) => "sin viajes".into(),
        (Lang::En, None) => "no trips".into(),
    };
    card.push_str(&format!("\n{rep}"));
    card
}

/// One kind's fares as the panel shows them, with what the buttons change.
pub fn fares_panel(lang: Lang, kind: VehicleKind, f: &Fares) -> String {
    let kind_name = vehicle_kind(lang, kind);
    // What to type for this kind: the moto needs no word.
    let word = match (lang, kind) {
        (_, VehicleKind::Moto) => "",
        (Lang::Es, VehicleKind::Car) => "carro ",
        (Lang::En, VehicleKind::Car) => "car ",
        (Lang::Es, VehicleKind::Tricycle) => "triciclo ",
        (Lang::En, VehicleKind::Tricycle) => "tricycle ",
    };
    match lang {
        Lang::Es => format!(
            "💰 Tarifas de {kind_name} (precio sugerido = base + precio por km × km)\n\nBase: {} CUP\nPor km: {} CUP\nMínimo: {} % del precio sugerido\n\nCámbialas con los botones o escribe un valor exacto: /tarifa {word}base 120, /tarifa {word}km 160 o /tarifa {word}minimo 75. Afectan a las solicitudes nuevas.",
            f.base, f.per_km, f.min_percent
        ),
        Lang::En => format!(
            "💰 {kind_name} fares (suggested price = base + price per km × km)\n\nBase: {} CUP\nPer km: {} CUP\nMinimum: {} % of the suggested price\n\nChange them with the buttons or type an exact value: /tarifa {word}base 120, /tarifa {word}km 160 or /tarifa {word}minimo 75. They apply to new requests.",
            f.base, f.per_km, f.min_percent
        ),
    }
}

/// The fares of the three kinds: `/tarifas`, or the panel's first step
/// (`pick`), where a kind is chosen to change them.
pub fn fares(lang: Lang, table: &FareTable, pick: bool) -> String {
    let mut text = match lang {
        Lang::Es => "💰 Tarifas (precio sugerido = base + precio por km × km)".to_owned(),
        Lang::En => "💰 Fares (suggested price = base + price per km × km)".to_owned(),
    };
    for kind in VehicleKind::ALL {
        let f = table.of(kind);
        text.push_str(&match lang {
            Lang::Es => format!(
                "\n\n{}\nBase: {} CUP · Por km: {} CUP · Mínimo: {} %",
                vehicle_kind(lang, kind),
                f.base,
                f.per_km,
                f.min_percent
            ),
            Lang::En => format!(
                "\n\n{}\nBase: {} CUP · Per km: {} CUP · Minimum: {} %",
                vehicle_kind(lang, kind),
                f.base,
                f.per_km,
                f.min_percent
            ),
        });
    }
    text.push_str(match (lang, pick) {
        (Lang::Es, true) => "\n\nElige el tipo para cambiar sus tarifas.",
        (Lang::En, true) => "\n\nPick the kind to change its fares.",
        (Lang::Es, false) => "\n\nPara cambiarlas (superadmins): /tarifa [carro|triciclo] base|km|minimo <valor>; sin tipo, la moto.",
        (Lang::En, false) => "\n\nTo change them (superadmins): /tarifa [carro|triciclo] base|km|minimo <value>; without a kind, the moto.",
    });
    text
}

/// The driver's vehicles, at the end of their card for staff.
pub fn driver_vehicles(lang: Lang, vehicles: &[Vehicle], active: Option<VehicleId>) -> String {
    let mut text = match lang {
        Lang::Es => "\n\n🚘 Vehículos:".to_owned(),
        Lang::En => "\n\n🚘 Vehicles:".to_owned(),
    };
    if vehicles.is_empty() {
        text.push_str(match lang {
            Lang::Es => " ninguno",
            Lang::En => " none",
        });
    }
    for v in vehicles {
        text.push_str(&format!("\n{}", vehicle_button(v, Some(v.id) == active)));
    }
    text
}

/// `/metricas`: one block per day, newest first.
pub fn metrics(lang: Lang, days: &[Day]) -> String {
    if days.is_empty() {
        return match lang {
            Lang::Es => "📊 Todavía no hay métricas.".into(),
            Lang::En => "📊 No metrics yet.".into(),
        };
    }
    let num = |v: Option<f64>, digits: usize| v.map(|v| format!("{v:.digits$}")).unwrap_or_else(|| "–".into());
    let mut lines = vec![match lang {
        Lang::Es => "📊 Métricas (últimos días)".to_owned(),
        Lang::En => "📊 Metrics (last days)".to_owned(),
    }];
    for d in days {
        lines.push(match lang {
            Lang::Es => format!(
                "\n{}\nPublicadas {} · asignadas {} · completadas {} · canceladas {} · expiradas {}\nMedia: {} km · {} CUP · final/sugerido {}",
                d.day, d.published, d.assigned, d.completed, d.cancelled, d.expired,
                num(d.avg_km(), 1), num(d.avg_price(), 0), num(d.final_over_suggested(), 2)
            ),
            Lang::En => format!(
                "\n{}\nPublished {} · assigned {} · completed {} · cancelled {} · expired {}\nAverage: {} km · {} CUP · final/suggested {}",
                d.day, d.published, d.assigned, d.completed, d.cancelled, d.expired,
                num(d.avg_km(), 1), num(d.avg_price(), 0), num(d.final_over_suggested(), 2)
            ),
        });
    }
    lines.join("\n")
}

pub fn admin_invite_created(lang: Lang, link: &str) -> String {
    match lang {
        Lang::Es => format!("🔗 Enlace para nombrar un admin (un solo uso, vence en 48 h):\n{link}"),
        Lang::En => format!("🔗 Link to appoint an admin (single use, expires in 48 h):\n{link}"),
    }
}

/// Superadmins (read-only: they come from `.env`) and admins.
pub fn admins_list(lang: Lang, superadmins: &[String], admins: usize) -> String {
    let supers = superadmins.iter().map(|n| format!("• {n}")).collect::<Vec<_>>().join("\n");
    match (lang, admins) {
        (Lang::Es, 0) => format!("⭐ Superadmins (en .env, no se cambian desde el bot):\n{supers}\n\n👮 Admins: ninguno todavía."),
        (Lang::En, 0) => format!("⭐ Superadmins (in .env, not changed from the bot):\n{supers}\n\n👮 Admins: none yet."),
        (Lang::Es, n) => format!(
            "⭐ Superadmins (en .env, no se cambian desde el bot):\n{supers}\n\n👮 Admins: {n}\nToca uno para ver su ficha y actuar sobre él."
        ),
        (Lang::En, n) => format!(
            "⭐ Superadmins (in .env, not changed from the bot):\n{supers}\n\n👮 Admins: {n}\nTap one to see their card and act on it."
        ),
    }
}

/// One admin, for a superadmin.
pub fn admin_card(lang: Lang, name: &str, appointed_by: &str, since: &str) -> String {
    match lang {
        Lang::Es => format!("👮 {name}\nAdmin desde el {since}, nombrado por {appointed_by}."),
        Lang::En => format!("👮 {name}\nAdmin since {since}, appointed by {appointed_by}."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fares_show_the_three_kinds() {
        let moto = Fares { base: 100, per_km: 150, min_percent: 80 };
        let table = FareTable { car: Fares { per_km: 250, ..moto.clone() }, tricycle: moto.clone(), moto };
        let text = fares(Lang::Es, &table, false);
        assert!(text.contains("🏍 Moto\nBase: 100 CUP · Por km: 150 CUP"), "{text}");
        assert!(text.contains("🚗 Carro\nBase: 100 CUP · Por km: 250 CUP"), "{text}");
        assert!(text.contains("🛺 Triciclo\n"), "{text}");
        assert!(fares_panel(Lang::Es, VehicleKind::Car, &table.car).contains("/tarifa carro km 160"));
        assert!(fares_panel(Lang::Es, VehicleKind::Moto, &table.moto).contains("/tarifa km 160"));
    }
}
