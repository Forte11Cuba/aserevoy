//! Price suggestion: a flat base plus a fixed rate per km. The initial values
//! come from `.env`; superadmins change them from the bot (`/tarifa`).

/// Amount in CUP.
pub type Cup = i64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fares {
    /// Flat amount per trip (going to the pickup, waiting).
    pub base: Cup,
    pub per_km: Cup,
    /// Minimum allowed offer, as a percentage of the suggested price.
    pub min_percent: u8,
}

/// `base + per_km × km`, rounded to the nearest 10 CUP.
pub fn suggested_price(km: f64, fares: &Fares) -> Cup {
    round_to_10(fares.base as f64 + fares.per_km as f64 * km.max(0.0))
}

pub fn minimum_price(suggested: Cup, fares: &Fares) -> Cup {
    round_to_10(suggested as f64 * f64::from(fares.min_percent) / 100.0)
}

/// Without an exact location (a written description), the route goes to the
/// municipality's centre; the driver's uncertainty is paid with 50 % more km.
pub const APPROX_KM_FACTOR: f64 = 1.5;

pub fn approximate_km(km: f64) -> f64 {
    km * APPROX_KM_FACTOR
}

/// Which fare a superadmin changes with `/tarifa <name> <value>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FareKey {
    Base,
    PerKm,
    MinPercent,
}

impl FareKey {
    /// The names used in the command (`base`, `km`, `minimo`).
    pub fn parse(name: &str) -> Option<FareKey> {
        match name.to_ascii_lowercase().as_str() {
            "base" => Some(FareKey::Base),
            "km" => Some(FareKey::PerKm),
            "minimo" | "mínimo" | "min" => Some(FareKey::MinPercent),
            _ => None,
        }
    }

    /// Whether `value` is acceptable: a positive amount, or a percentage.
    pub fn valid(self, value: i64) -> bool {
        match self {
            FareKey::Base | FareKey::PerKm => (1..=1_000_000).contains(&value),
            FareKey::MinPercent => (0..=100).contains(&value),
        }
    }
}

fn round_to_10(amount: f64) -> Cup {
    ((amount / 10.0).round() * 10.0) as Cup
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fares() -> Fares {
        Fares { base: 100, per_km: 150, min_percent: 80 }
    }

    #[test]
    fn suggested_price_rounds_to_10() {
        assert_eq!(suggested_price(3.2, &fares()), 580); // 100 + 480
        assert_eq!(suggested_price(3.23, &fares()), 580); // 584.5 -> 580
        assert_eq!(suggested_price(3.27, &fares()), 590); // 590.5 -> 590
    }

    #[test]
    fn zero_or_negative_distance_costs_the_base() {
        assert_eq!(suggested_price(0.0, &fares()), 100);
        assert_eq!(suggested_price(-1.0, &fares()), 100);
    }

    #[test]
    fn fare_keys_parse_and_validate() {
        assert_eq!(FareKey::parse("KM"), Some(FareKey::PerKm));
        assert_eq!(FareKey::parse("mínimo"), Some(FareKey::MinPercent));
        assert_eq!(FareKey::parse("litro"), None);
        assert!(FareKey::PerKm.valid(150) && !FareKey::PerKm.valid(0));
        assert!(FareKey::MinPercent.valid(80) && !FareKey::MinPercent.valid(101));
    }

    #[test]
    fn minimum_is_a_percentage_of_suggested() {
        assert_eq!(minimum_price(580, &fares()), 460); // 464 -> 460
    }
}
