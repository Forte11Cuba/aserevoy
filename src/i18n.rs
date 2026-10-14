//! User-facing texts in Spanish (default) and English. Every text is a `match`
//! over `Lang`, so the compiler rejects a message missing a translation.

use crate::domain::effect::Notice;
use crate::domain::request::DomainError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    #[default]
    Es,
    En,
}

impl Lang {
    /// Picks English only when Telegram says the user's app is in English.
    pub fn from_telegram(language_code: Option<&str>) -> Lang {
        match language_code {
            Some(code) if code.to_ascii_lowercase().starts_with("en") => Lang::En,
            _ => Lang::Es,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Lang::Es => "es",
            Lang::En => "en",
        }
    }

    pub fn parse(s: &str) -> Option<Lang> {
        match s {
            "es" => Some(Lang::Es),
            "en" => Some(Lang::En),
            _ => None,
        }
    }
}

/// Base text of a notice. The bot appends details such as the driver's card
/// or the client's contact when it renders `DriverAssigned` / `TripAssigned`.
pub fn notice(lang: Lang, notice: &Notice) -> String {
    use Notice::*;
    match (lang, notice) {
        (Lang::Es, Published { request }) => {
            format!("📣 Tu solicitud {request} ya está publicada. Te aviso cuando un chofer la acepte.")
        }
        (Lang::En, Published { request }) => {
            format!("📣 Your request {request} is live. I'll let you know when a driver accepts it.")
        }
        (Lang::Es, DriverAssigned { request, .. }) => format!("✅ ¡Un chofer aceptó tu viaje {request}!"),
        (Lang::En, DriverAssigned { request, .. }) => format!("✅ A driver accepted your trip {request}!"),
        (Lang::Es, TripAssigned { request, .. }) => format!("🛵 El viaje {request} es tuyo."),
        (Lang::En, TripAssigned { request, .. }) => format!("🛵 Trip {request} is yours."),
        (Lang::Es, Completed { request }) => format!("🏁 Viaje {request} completado. ¡Gracias!"),
        (Lang::En, Completed { request }) => format!("🏁 Trip {request} completed. Thanks!"),
        (Lang::Es, CancelledByClient { request }) => format!("❌ El cliente canceló el viaje {request}."),
        (Lang::En, CancelledByClient { request }) => format!("❌ The client cancelled trip {request}."),
        (Lang::Es, CancelledByDriver { request }) => {
            format!("❌ El chofer canceló el viaje {request}. ¿Quieres publicarlo de nuevo al mismo precio?")
        }
        (Lang::En, CancelledByDriver { request }) => {
            format!("❌ The driver cancelled trip {request}. Publish it again at the same price?")
        }
        (Lang::Es, Expired { request }) => {
            format!("⌛ Nadie tomó la solicitud {request} a tiempo. ¿Quieres publicarla de nuevo?")
        }
        (Lang::En, Expired { request }) => {
            format!("⌛ Nobody took request {request} in time. Publish it again?")
        }
    }
}

pub fn domain_error(lang: Lang, error: &DomainError) -> String {
    use DomainError::*;
    match (lang, error) {
        (Lang::Es, PriceBelowMinimum { minimum, .. }) => {
            format!("El precio mínimo para este viaje es {minimum} CUP.")
        }
        (Lang::En, PriceBelowMinimum { minimum, .. }) => {
            format!("The minimum price for this trip is {minimum} CUP.")
        }
        (Lang::Es, NotOpen) => "Esta solicitud ya no está disponible.".into(),
        (Lang::En, NotOpen) => "This request is no longer available.".into(),
        (Lang::Es, NotAssigned) => "Este viaje no está asignado.".into(),
        (Lang::En, NotAssigned) => "This trip is not assigned.".into(),
        (Lang::Es, OwnRequest) => "No puedes aceptar tu propia solicitud.".into(),
        (Lang::En, OwnRequest) => "You can't accept your own request.".into(),
        (Lang::Es, NotParticipant) => "Solo el cliente o el chofer del viaje pueden hacer esto.".into(),
        (Lang::En, NotParticipant) => "Only the trip's client or driver can do this.".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spanish_is_the_default() {
        assert_eq!(Lang::from_telegram(None), Lang::Es);
        assert_eq!(Lang::from_telegram(Some("es")), Lang::Es);
        assert_eq!(Lang::from_telegram(Some("pt-br")), Lang::Es);
        assert_eq!(Lang::from_telegram(Some("en")), Lang::En);
        assert_eq!(Lang::from_telegram(Some("EN-us")), Lang::En);
    }

    #[test]
    fn lang_round_trips_through_its_code() {
        for lang in [Lang::Es, Lang::En] {
            assert_eq!(Lang::parse(lang.as_str()), Some(lang));
        }
    }
}
