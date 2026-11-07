//! The offer posted in the channel.

use crate::domain::fare::Cup;
use crate::domain::ids::RequestId;
use crate::domain::reputation::Shown;
use crate::domain::request::PaymentMethod;
use crate::domain::vehicle::VehicleKind;
use super::{Lang, distance, payment, reputation, vehicle_kind};

/// What the channel shows of a request.
pub struct Post<'a> {
    pub request: &'a RequestId,
    pub vehicle_kind: VehicleKind,
    pub km: f64,
    /// By road; 0 when unknown.
    pub minutes: f64,
    pub price: Cup,
    /// What the app suggests for this trip, to compare with `price`.
    pub suggested: Cup,
    pub payment: PaymentMethod,
    /// Municipalities of pickup and destination, when known.
    pub from: Option<&'a str>,
    pub to: Option<&'a str>,
    /// Nearest corners, for exact points ("23 y L").
    pub from_corner: Option<String>,
    pub to_corner: Option<String>,
    /// A point is a written description: distance and price are estimates.
    pub approx: bool,
    /// The client's reputation.
    pub client: Option<Shown>,
    /// The client's cancellations of the last 30 days.
    pub cancellations: Option<i64>,
}

/// The offer as drivers see it in the channel: approximate zones (the
/// municipality of each pin), never the exact location or the contact.
pub fn channel_post(lang: Lang, p: &Post) -> String {
    let (request, km, price) = (p.request, p.km, p.price);
    let pay = payment(lang, p.payment);
    // The kind first, so drivers of other kinds skip it (decision 119), and
    // the request's id beside it (decision 131).
    let mut post = format!("{} · 🆔 {request}\n", vehicle_kind(lang, p.vehicle_kind));
    if p.from.is_some() || p.to.is_some() {
        let zone = |m: Option<&str>, corner: &Option<String>| match corner {
            Some(c) => format!("{} ({c})", m.unwrap_or("?")),
            None => m.unwrap_or("?").to_owned(),
        };
        // Pickup, an arrow down, and the finish flag for the destination.
        post.push_str(&format!("📍 {}\n⬇️\n🏁 {}\n", zone(p.from, &p.from_corner), zone(p.to, &p.to_corner)));
    }
    post.push_str(&format!("{}\n💰 {price} CUP · {pay}", distance(lang, km, p.minutes, p.approx)));
    post.push_str(&format!("\n{}", against_suggested(lang, price, p.suggested, p.approx)));
    if p.approx {
        post.push_str(match lang {
            Lang::Es => "\n📝 Dirección escrita, sin ubicación exacta",
            Lang::En => "\n📝 Written address, no exact location",
        });
    }
    if let Some(client) = &p.client {
        post.push_str(&format!("\n👤 {}", reputation(lang, client)));
    }
    if let Some(n) = p.cancellations {
        post.push_str(&match lang {
            Lang::Es => format!("\n❌ Cancelaciones (30 días): {n}"),
            Lang::En => format!("\n❌ Cancellations (30 days): {n}"),
        });
    }
    post
}

/// How the offer compares with the suggested price, for drivers:
/// "🔴⬇️ 200 CUP menos que el precio sugerido (800 CUP)", "🟢⬆️ …más…", or
/// "⚪🟰 Es el precio sugerido". "≈" when a point is approximate.
fn against_suggested(lang: Lang, price: Cup, suggested: Cup, approx: bool) -> String {
    let about = if approx { "≈" } else { "" };
    let diff = price - suggested;
    match (lang, diff.signum()) {
        (Lang::Es, -1) => format!("🔴⬇️ {} CUP menos que el precio sugerido ({about}{suggested} CUP)", -diff),
        (Lang::En, -1) => format!("🔴⬇️ {} CUP below the suggested price ({about}{suggested} CUP)", -diff),
        (Lang::Es, 1) => format!("🟢⬆️ {diff} CUP más que el precio sugerido ({about}{suggested} CUP)"),
        (Lang::En, 1) => format!("🟢⬆️ {diff} CUP above the suggested price ({about}{suggested} CUP)"),
        (Lang::Es, _) => "⚪🟰 Es el precio sugerido".to_owned(),
        (Lang::En, _) => "⚪🟰 It's the suggested price".to_owned(),
    }
}

pub fn channel_confirming(lang: Lang, request: &RequestId) -> String {
    match lang {
        Lang::Es => format!("⏳ Solicitud {request}: un chofer espera la confirmación del cliente."),
        Lang::En => format!("⏳ Request {request}: a driver is waiting for the client's confirmation."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_offer_shows_pickup_then_destination_and_the_client() {
        let id = RequestId::parse("K7X2").unwrap();
        let post = Post {
            request: &id,
            vehicle_kind: VehicleKind::Car,
            km: 3.5,
            minutes: 9.4,
            price: 600,
            suggested: 800,
            payment: PaymentMethod::Cash,
            from: Some("Plaza de la Revolución"),
            to: Some("Habana del Este"),
            from_corner: Some("23 y L".into()),
            to_corner: None,
            approx: false,
            client: Some(Shown { stars: Some(4.7), trips: 23, days: 120 }),
            cancellations: Some(1),
        };
        let text = channel_post(Lang::Es, &post);
        assert!(text.starts_with("🚗 Carro · 🆔 K7X2\n📍 Plaza"), "{text}");
        assert!(text.contains("📍 Plaza de la Revolución (23 y L)\n⬇️\n🏁 Habana del Este\n"), "{text}");
        assert!(text.contains("💰 600 CUP · 💵 Efectivo\n🔴⬇️ 200 CUP menos que el precio sugerido (800 CUP)\n"), "{text}");
        assert_eq!(against_suggested(Lang::Es, 1100, 800, true), "🟢⬆️ 300 CUP más que el precio sugerido (≈800 CUP)");
        assert_eq!(against_suggested(Lang::Es, 800, 800, false), "⚪🟰 Es el precio sugerido");
        assert!(text.contains("👤 ⭐4.7 · 23 viajes\n📅 120 días usando Asere Voy\n❌ Cancelaciones (30 días): 1"), "{text}");
    }
}
