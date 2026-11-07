//! Price suggestion: a flat base plus a fixed rate per km, for each kind of
//! vehicle (decision 118). The initial values come from `.env`; superadmins
//! change them from the bot (`/tarifa`).

use super::vehicle::VehicleKind;

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

/// The fares of each kind of vehicle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FareTable {
    pub moto: Fares,
    pub car: Fares,
    pub tricycle: Fares,
}

impl FareTable {
    /// Car and tricycle start as copies of the moto, until a superadmin
    /// adjusts them (the database does it at start; tests use this).
    #[cfg(test)]
    pub fn like_moto(moto: Fares) -> FareTable {
        FareTable { car: moto.clone(), tricycle: moto.clone(), moto }
    }

    pub fn of(&self, kind: VehicleKind) -> &Fares {
        match kind {
            VehicleKind::Moto => &self.moto,
            VehicleKind::Car => &self.car,
            VehicleKind::Tricycle => &self.tricycle,
        }
    }
}

/// `base + per_km × km`, rounded to the nearest 10 CUP.
pub fn suggested_price(km: f64, fares: &Fares) -> Cup {
    round_to_10(fares.base as f64 + fares.per_km as f64 * km.max(0.0))
}

pub fn minimum_price(suggested: Cup, fares: &Fares) -> Cup {
    round_to_10(suggested as f64 * f64::from(fares.min_percent) / 100.0)
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

/// `/tarifa [carro|triciclo] <base|km|minimo> <value>`: without a kind, the
/// moto, as before there were other kinds. `None` if anything is missing,
/// unknown or out of range.
pub fn parse_change(args: &str) -> Option<(VehicleKind, FareKey, i64)> {
    let mut words = args.split_whitespace().peekable();
    let kind = match words.peek().and_then(|w| VehicleKind::parse_word(w)) {
        Some(kind) => {
            words.next();
            kind
        }
        None => VehicleKind::Moto,
    };
    let key = FareKey::parse(words.next()?)?;
    let value = words.next()?.parse().ok()?;
    (words.next().is_none() && key.valid(value)).then_some((kind, key, value))
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
    fn each_kind_has_its_fares() {
        let mut table = FareTable::like_moto(fares());
        assert_eq!(table.of(VehicleKind::Car), &fares());
        table.car.per_km = 250;
        assert_eq!(suggested_price(2.0, table.of(VehicleKind::Car)), 600);
        assert_eq!(suggested_price(2.0, table.of(VehicleKind::Moto)), 400);
    }

    #[test]
    fn fare_changes_name_the_kind_or_mean_the_moto() {
        assert_eq!(parse_change("km 60"), Some((VehicleKind::Moto, FareKey::PerKm, 60)));
        assert_eq!(parse_change("carro km 80"), Some((VehicleKind::Car, FareKey::PerKm, 80)));
        assert_eq!(parse_change("Triciclo minimo 70"), Some((VehicleKind::Tricycle, FareKey::MinPercent, 70)));
        assert_eq!(parse_change("moto base 100"), Some((VehicleKind::Moto, FareKey::Base, 100)));
        assert_eq!(parse_change("carro km"), None);
        assert_eq!(parse_change("carro litro 5"), None);
        assert_eq!(parse_change("km 0"), None);
        assert_eq!(parse_change("km 60 70"), None);
        assert_eq!(parse_change(""), None);
    }

    #[test]
    fn minimum_is_a_percentage_of_suggested() {
        assert_eq!(minimum_price(580, &fares()), 460); // 464 -> 460
    }
}
