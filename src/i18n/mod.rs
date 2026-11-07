//! User-facing texts in Spanish (default) and English. Every text is a `match`
//! over `Lang`, so the compiler rejects a message missing a translation.

mod buttons;
mod texts;
mod ride;
mod channel;
mod trip;
mod auction;
mod driver;
mod vehicles;
mod history;
mod help;
mod staff;

pub use buttons::*;
pub use texts::*;
pub use ride::*;
pub use channel::*;
pub use trip::*;
pub use auction::*;
pub use driver::*;
pub use vehicles::*;
pub use history::*;
pub use help::*;
pub use staff::*;

use crate::domain::effect::Place;
use crate::domain::reputation::{Shown, Tag};

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

    pub fn other(self) -> Lang {
        match self {
            Lang::Es => Lang::En,
            Lang::En => Lang::Es,
        }
    }
}

/// Title of a pin sent to the driver.
pub fn place(lang: Lang, place: Place) -> &'static str {
    match (lang, place) {
        (Lang::Es, Place::Pickup) => "📍 Recogida",
        (Lang::En, Place::Pickup) => "📍 Pickup",
        (Lang::Es, Place::Dropoff) => "🏁 Destino",
        (Lang::En, Place::Dropoff) => "🏁 Destination",
    }
}


/// "📏 3.5 km · ⏱ unos 9 min"; "≈" for an approximate point, and
/// no time when it is unknown.
pub fn distance(lang: Lang, km: f64, minutes: f64, approx: bool) -> String {
    let about = if approx { "≈" } else { "" };
    let mut line = format!("📏 {about}{km:.1} km");
    if minutes > 0.0 {
        let minutes = minutes.round().max(1.0);
        line.push_str(&match lang {
            Lang::Es => format!(" · ⏱ unos {minutes} min"),
            Lang::En => format!(" · ⏱ about {minutes} min"),
        });
    }
    line
}

/// The reputation at the end of a card, in two lines: "⭐4.7 · 23 viajes"
/// and "📅 120 días usando Asere Voy". Stars from the first rating.
pub fn reputation(lang: Lang, shown: &Shown) -> String {
    format!("{}\n📅 {}", stars_and_trips(lang, shown), days_using(lang, shown.days))
}

/// The same in one line, for staff messages.
pub(super) fn reputation_inline(lang: Lang, shown: &Shown) -> String {
    format!("{} · {}", stars_and_trips(lang, shown), days_using(lang, shown.days))
}

fn stars_and_trips(lang: Lang, shown: &Shown) -> String {
    let stars = match (lang, shown.stars) {
        (_, Some(avg)) => format!("⭐{avg:.1}"),
        (Lang::Es, None) => "⭐ sin calificaciones".to_owned(),
        (Lang::En, None) => "⭐ no ratings yet".to_owned(),
    };
    let trips = match (lang, shown.trips) {
        (Lang::Es, 1) => "1 viaje".to_owned(),
        (Lang::Es, n) => format!("{n} viajes"),
        (Lang::En, 1) => "1 trip".to_owned(),
        (Lang::En, n) => format!("{n} trips"),
    };
    format!("{stars} · {trips}")
}

pub(super) fn days_using(lang: Lang, days: i64) -> String {
    match (lang, days) {
        (Lang::Es, 1) => "1 día usando Asere Voy".to_owned(),
        (Lang::Es, n) => format!("{n} días usando Asere Voy"),
        (Lang::En, 1) => "1 day using Asere Voy".to_owned(),
        (Lang::En, n) => format!("{n} days using Asere Voy"),
    }
}

pub fn tag(lang: Lang, tag: Tag) -> &'static str {
    match (lang, tag) {
        (Lang::Es, Tag::Punctual) => "⏱ Puntual",
        (Lang::En, Tag::Punctual) => "⏱ Punctual",
        (Lang::Es, Tag::Kind) => "😊 Amable",
        (Lang::En, Tag::Kind) => "😊 Kind",
        (Lang::Es, Tag::Overcharged) => "💸 Cobró de más",
        (Lang::En, Tag::Overcharged) => "💸 Overcharged",
        (Lang::Es, Tag::NoShow) => "🙈 No apareció",
        (Lang::En, Tag::NoShow) => "🙈 Didn't show up",
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
