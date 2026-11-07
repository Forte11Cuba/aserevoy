//! Asking for a ride: price, addresses, summary.

use crate::domain::effect::Place;
use crate::domain::fare::Cup;
use crate::domain::ids::RequestId;
use crate::domain::request::PaymentMethod;
use crate::domain::vehicle::VehicleKind;
use super::{Button, Lang, button, distance, vehicle_kind};

pub fn payment(lang: Lang, p: PaymentMethod) -> &'static str {
    match p {
        PaymentMethod::Cash => button(lang, Button::Cash),
        PaymentMethod::Transfer => button(lang, Button::Transfer),
        PaymentMethod::Bitcoin => button(lang, Button::Bitcoin),
    }
}

pub fn already_active(lang: Lang, request: &RequestId) -> String {
    match lang {
        Lang::Es => format!("Ya tienes la solicitud {request} en curso."),
        Lang::En => format!("You already have request {request} in progress."),
    }
}

/// The offer being adjusted, with the trip, the suggested price and the minimum.
pub fn price_prompt(
    lang: Lang,
    km: f64,
    minutes: f64,
    suggested: Cup,
    minimum: Cup,
    offer: Cup,
    approx: bool,
) -> String {
    // The whole offer over the km (the base fare included): easy to compare.
    let per_km = if km > 0.0 { (offer as f64 / km).round() as Cup } else { offer };
    let distance = distance(lang, km, minutes, approx);
    match lang {
        Lang::Es => format!(
            "{distance}\n💰 Tu oferta: {offer} CUP (≈{per_km} CUP por km)\n(sugerido {suggested} CUP · mínimo {minimum} CUP)\n\nAjústala con los botones o escribe un monto."
        ),
        Lang::En => format!(
            "{distance}\n💰 Your offer: {offer} CUP (≈{per_km} CUP per km)\n(suggested {suggested} CUP · minimum {minimum} CUP)\n\nAdjust it with the buttons or type an amount."
        ),
    }
}

/// Before the price, when several kinds can do the trip (decision 117).
pub fn ask_vehicle(lang: Lang, km: f64, minutes: f64, approx: bool) -> String {
    let distance = distance(lang, km, minutes, approx);
    match lang {
        Lang::Es => format!("{distance}\n\n¿En qué quieres ir? Cada opción tiene su precio sugerido."),
        Lang::En => format!("{distance}\n\nWhat do you want to go in? Each option has its suggested price."),
    }
}

pub fn offer_button(lang: Lang, offer: Cup) -> String {
    match lang {
        Lang::Es => format!("✅ Ofrecer {offer} CUP"),
        Lang::En => format!("✅ Offer {offer} CUP"),
    }
}

pub fn reparto_not_found(lang: Lang, municipality: &str) -> String {
    match lang {
        Lang::Es => format!("No encontré ese reparto en {municipality}. Escríbelo de otra forma o envía tu ubicación 📍."),
        Lang::En => format!("I couldn't find that neighborhood in {municipality}. Type it another way or send your location 📍."),
    }
}

/// The last look before publishing.
/// What the client checks before publishing.
pub struct Summary<'a> {
    /// Only when the client picked it, so a pilot with motos alone sees no change.
    pub kind: Option<VehicleKind>,
    pub from: &'a str,
    pub to: &'a str,
    pub km: f64,
    pub approx: bool,
    pub price: Cup,
    pub payment: PaymentMethod,
}

pub fn ride_summary(lang: Lang, s: &Summary) -> String {
    let Summary { kind, from, to, km, approx, price, payment: pay } = *s;
    let about = if approx { "≈" } else { "" };
    let per_km = if km > 0.0 { (price as f64 / km).round() as Cup } else { price };
    let pay = payment(lang, pay);
    let kind = kind.map(|k| format!("{}\n", vehicle_kind(lang, k))).unwrap_or_default();
    match lang {
        Lang::Es => format!(
            "📋 Revisa tu solicitud\n\n{kind}📍 Recogida: {from}\n🏁 Destino: {to}\n📏 {about}{km:.1} km · 💰 {price} CUP (≈{per_km} CUP por km) · {pay}\n\nSi está bien, publícala; si no, vuelve atrás."
        ),
        Lang::En => format!(
            "📋 Check your request\n\n{kind}📍 Pickup: {from}\n🏁 Destination: {to}\n📏 {about}{km:.1} km · 💰 {price} CUP (≈{per_km} CUP per km) · {pay}\n\nIf it's right, publish it; if not, go back."
        ),
    }
}

/// Several places for a written address, numbered like their buttons.
pub fn choose_address(lang: Lang, labels: &[&str]) -> String {
    let head = match lang {
        Lang::Es => "Encontré varios lugares con ese nombre. ¿Cuál es?",
        Lang::En => "I found several places by that name. Which one is it?",
    };
    let list: Vec<String> = labels.iter().enumerate().map(|(i, l)| format!("{}. {l}", i + 1)).collect();
    format!("{head}\n\n{}", list.join("\n"))
}

/// "Is it here?" under the place found for a written address.
pub fn confirm_written(lang: Lang, pickup: bool, label: &str) -> String {
    match (lang, pickup) {
        (Lang::Es, true) => format!("📍 {label}\n¿Es aquí la recogida?"),
        (Lang::En, true) => format!("📍 {label}\nIs this the pickup point?"),
        (Lang::Es, false) => format!("🏁 {label}\n¿Es aquí el destino?"),
        (Lang::En, false) => format!("🏁 {label}\nIs this the destination?"),
    }
}

/// The client's description placed at a reparto, to confirm.
pub fn confirm_approx(lang: Lang, pickup: bool, note: &str, municipality: &str) -> String {
    match (lang, pickup) {
        (Lang::Es, true) => format!(
            "📍 {note}, {municipality}\nSin ubicación exacta: el punto del mapa es el centro del reparto. ¿Te recogemos por aquí?"
        ),
        (Lang::En, true) => format!(
            "📍 {note}, {municipality}\nNo exact location: the point on the map is the middle of the reparto. Is the pickup around here?"
        ),
        (Lang::Es, false) => format!(
            "🏁 {note}, {municipality}\nSin ubicación exacta: el punto del mapa es el centro del reparto. ¿El destino es por aquí?"
        ),
        (Lang::En, false) => format!(
            "🏁 {note}, {municipality}\nNo exact location: the point on the map is the middle of the reparto. Is the destination around here?"
        ),
    }
}

/// To the driver, for a point without an exact location.
pub fn approx_point(lang: Lang, place: Place, note: &str, municipality: Option<&str>) -> String {
    let municipality = municipality.unwrap_or("?");
    match (lang, place) {
        (Lang::Es, Place::Pickup) => format!("📍 Recogida (sin ubicación exacta, en {municipality}):\n{note}"),
        (Lang::En, Place::Pickup) => format!("📍 Pickup (no exact location, in {municipality}):\n{note}"),
        (Lang::Es, Place::Dropoff) => format!("🏁 Destino (sin ubicación exacta, en {municipality}):\n{note}"),
        (Lang::En, Place::Dropoff) => format!("🏁 Destination (no exact location, in {municipality}):\n{note}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_are_listed_in_full_and_numbered() {
        let text = choose_address(Lang::Es, &["Avenida Cuarta (92) entre 21 D (12) y 21 C (13) en Habana del Este", "78 en Habana del Este"]);
        assert!(text.contains("\n1. Avenida Cuarta (92) entre 21 D (12) y 21 C (13) en Habana del Este\n2. 78"), "{text}");
    }

    #[test]
    fn a_described_point_is_confirmed_with_its_reparto() {
        let text = confirm_approx(Lang::Es, true, "Calle 5 y 6 alamar (Alamar)", "Habana del Este");
        assert!(text.starts_with("📍 Calle 5 y 6 alamar (Alamar), Habana del Este\n"), "{text}");
        assert!(text.ends_with("¿Te recogemos por aquí?"), "{text}");
    }

    #[test]
    fn the_summary_shows_everything_before_publishing() {
        let summary = Summary {
            kind: None,
            from: "Plaza (23 y L)",
            to: "Centro Habana",
            km: 3.5,
            approx: false,
            price: 600,
            payment: PaymentMethod::Cash,
        };
        let text = ride_summary(Lang::Es, &summary);
        assert!(text.contains("Recogida: Plaza (23 y L)") && text.contains("Destino: Centro Habana"), "{text}");
        assert!(text.contains("3.5 km · 💰 600 CUP (≈171 CUP por km)"), "{text}");
        assert!(text.starts_with("📋 Revisa tu solicitud\n\n📍"), "{text}");
        let text = ride_summary(Lang::Es, &Summary { kind: Some(VehicleKind::Car), ..summary });
        assert!(text.starts_with("📋 Revisa tu solicitud\n\n🚗 Carro\n📍 Recogida"), "{text}");
    }

    #[test]
    fn the_offer_shows_its_price_per_km() {
        let text = price_prompt(Lang::Es, 52.8, 68.0, 8010, 6410, 8010, true);
        assert!(text.contains("8010 CUP (≈152 CUP por km)"), "{text}");
        assert!(text.contains("📏 ≈52.8 km · ⏱ unos 68 min"), "{text}");
    }
}
