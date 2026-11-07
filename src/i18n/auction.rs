//! Confirming a driver, counteroffers and raising the price.

use crate::domain::driver::Profile;
use crate::domain::fare::Cup;
use crate::domain::ids::RequestId;
use crate::domain::reputation::Shown;
use crate::domain::vehicle::{Vehicle, VehicleKind};
use super::{Lang, Post, channel_post, reputation, vehicle_lines, vehicle_name};

/// To the client when a driver accepts: who wants the trip, without the
/// phone until they confirm.
pub fn confirm_driver(
    lang: Lang,
    request: &RequestId,
    p: &Profile,
    (kind, vehicle): (VehicleKind, Option<&Vehicle>),
    driver: &Shown,
) -> String {
    let rep = reputation(lang, driver);
    let name = full_name_line(p);
    let bike = vehicle_lines(lang, kind, vehicle);
    match lang {
        Lang::Es => format!(
            "{} Un chofer quiere hacer tu viaje {request}:\n\n👤 {}{name}\n{bike}\n{rep}\n\nTienes 5 min para confirmarlo o cancelar.",
            kind.emoji(),
            p.alias
        ),
        Lang::En => format!(
            "{} A driver wants to take your trip {request}:\n\n👤 {}{name}\n{bike}\n{rep}\n\nYou have 5 min to confirm or cancel.",
            kind.emoji(),
            p.alias
        ),
    }
}

/// The driver's full name for the client, for safety (none for drivers from
/// before it was asked).
fn full_name_line(p: &Profile) -> String {
    p.full_name.as_deref().map(|n| format!("\n🪪 {n}")).unwrap_or_default()
}

/// One counteroffer, as its own message to the client.
pub fn offer_card(
    lang: Lang,
    request: &RequestId,
    p: &Profile,
    (kind, vehicle): (VehicleKind, Option<&Vehicle>),
    amount: Cup,
    driver: &Shown,
) -> String {
    let rep = reputation(lang, driver);
    let bike = vehicle_name(lang, kind, vehicle);
    match lang {
        Lang::Es => format!(
            "💬 Contraoferta para tu viaje {request}\n\n💰 {amount} CUP\n👤 {}\n{bike}\n{rep}\n\nVale 5 min.",
            p.alias
        ),
        Lang::En => format!(
            "💬 Counteroffer for your trip {request}\n\n💰 {amount} CUP\n👤 {}\n{bike}\n{rep}\n\nValid for 5 min.",
            p.alias
        ),
    }
}

/// To a driver who opened "Counteroffer" from the channel.
/// The request, and the counteroffer so far (it starts at the client's price).
pub fn counteroffer_prompt(lang: Lang, post: &Post, amount: Cup) -> String {
    let price = post.price;
    let post = channel_post(lang, post);
    match lang {
        Lang::Es => format!(
            "{post}\n\n💬 Tu contraoferta: {amount} CUP (el cliente ofrece {price} CUP)\nℹ️ No puedes contraofertar menos que la oferta del cliente: tiene que ser más de {price} CUP."
        ),
        Lang::En => format!(
            "{post}\n\n💬 Your counteroffer: {amount} CUP (the client offers {price} CUP)\nℹ️ You can't counteroffer less than the client's offer: it must be more than {price} CUP."
        ),
    }
}

/// A counteroffer at or below the client's offer, typed or reached with −.
pub fn counteroffer_too_low(lang: Lang, price: Cup) -> String {
    match lang {
        Lang::Es => format!("No puedes contraofertar menos que la oferta del cliente. Tiene que ser más de {price} CUP."),
        Lang::En => format!("You can't counteroffer less than the client's offer. It must be more than {price} CUP."),
    }
}

/// "+50", "−500", "+1k", "+10k": what a counteroffer button adds.
pub fn amount_step(delta: Cup) -> String {
    let sign = if delta < 0 { "−" } else { "+" };
    let n = delta.abs();
    if n >= 1000 && n % 1000 == 0 { format!("{sign}{}k", n / 1000) } else { format!("{sign}{n}") }
}

pub fn counteroffer_sent(lang: Lang, amount: Cup) -> String {
    match lang {
        Lang::Es => format!("💬 Contraoferta de {amount} CUP enviada. Vale 5 min; te aviso si el cliente la acepta."),
        Lang::En => format!("💬 Counteroffer of {amount} CUP sent. It's valid for 5 min; I'll tell you if the client accepts."),
    }
}

/// 5 min with no driver: suggest a higher price.
pub fn suggest_raise(lang: Lang, request: &RequestId, price: Cup, suggested: Cup, offers: usize, lowest: Option<Cup>) -> String {
    let mut text = match lang {
        Lang::Es => format!("⏳ Han pasado 5 min y ningún chofer ha tomado tu viaje {request}."),
        Lang::En => format!("⏳ 5 min have passed and no driver has taken your trip {request}."),
    };
    if let Some(lowest) = lowest {
        text.push_str(&match (lang, offers) {
            (Lang::Es, 1) => format!("\nTienes 1 contraoferta en el chat ({lowest} CUP): puedes aceptarla."),
            (Lang::Es, n) => format!("\nTienes {n} contraofertas en el chat (desde {lowest} CUP): puedes aceptar una."),
            (Lang::En, 1) => format!("\nYou have 1 counteroffer in the chat ({lowest} CUP): you can accept it."),
            (Lang::En, n) => format!("\nYou have {n} counteroffers in the chat (from {lowest} CUP): you can accept one."),
        });
    }
    text.push_str(&match (lang, price < suggested) {
        (Lang::Es, true) => format!(
            "\n\nOfreces {price} CUP y el precio sugerido es {suggested} CUP. Si lo subes, es más fácil que un chofer lo acepte. Selecciona uno o escribe uno tú."
        ),
        (Lang::En, true) => format!(
            "\n\nYou offer {price} CUP and the suggested price is {suggested} CUP. If you raise it, a driver is more likely to accept. Pick one or type your own."
        ),
        (Lang::Es, false) => {
            format!("\n\nOfreces {price} CUP. Si lo subes un poco, es más fácil que un chofer lo acepte. Selecciona uno o escribe uno tú.")
        }
        (Lang::En, false) => {
            format!("\n\nYou offer {price} CUP. If you raise it a little, a driver is more likely to accept. Pick one or type your own.")
        }
    });
    text
}

/// What a raise-price message (the 5-minute suggestion, "how much do you
/// raise it to?") becomes once a price was picked.
pub fn raised_to(lang: Lang, price: Cup) -> String {
    match lang {
        Lang::Es => format!("✅ Subiste a {price} CUP."),
        Lang::En => format!("✅ You raised it to {price} CUP."),
    }
}

pub fn keep_price(lang: Lang, price: Cup) -> String {
    match lang {
        Lang::Es => format!("Mantener {price} CUP"),
        Lang::En => format!("Keep {price} CUP"),
    }
}

pub fn raise_prompt(lang: Lang, price: Cup) -> String {
    match lang {
        Lang::Es => format!("Ahora ofreces {price} CUP. ¿A cuánto lo subes?\nSelecciona uno o escribe uno tú."),
        Lang::En => format!("You're offering {price} CUP. How much do you raise it to?\nPick one or type your own."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counteroffer_steps_are_short() {
        assert_eq!(amount_step(50), "+50");
        assert_eq!(amount_step(-500), "−500");
        assert_eq!(amount_step(1000), "+1k");
        assert_eq!(amount_step(-1000), "−1k");
        assert_eq!(amount_step(10000), "+10k");
    }
}
