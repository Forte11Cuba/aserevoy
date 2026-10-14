//! Drivers' profiles. A driver fills in the profile from the bot, an admin
//! approves it once, and later edits do not need approval again.

use super::ids::UserId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverStatus {
    Pending,
    Active,
    Blocked,
}

impl DriverStatus {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(DriverStatus::Pending),
            "active" => Some(DriverStatus::Active),
            "blocked" => Some(DriverStatus::Blocked),
            _ => None,
        }
    }
}

/// Which payments the driver can take. At least one is always set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PaymentMethods {
    pub cash: bool,
    pub transfer: bool,
}

impl PaymentMethods {
    pub const CASH: Self = PaymentMethods { cash: true, transfer: false };
    pub const TRANSFER: Self = PaymentMethods { cash: false, transfer: true };
    pub const BOTH: Self = PaymentMethods { cash: true, transfer: true };

    pub fn as_str(self) -> &'static str {
        match (self.cash, self.transfer) {
            (true, true) => "cash,transfer",
            (false, true) => "transfer",
            _ => "cash",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "cash" => Some(Self::CASH),
            "transfer" => Some(Self::TRANSFER),
            "cash,transfer" => Some(Self::BOTH),
            _ => None,
        }
    }
}

/// What the driver fills in. The phone is in clear here; the database layer
/// encrypts it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Profile {
    pub alias: String,
    pub phone: String,
    pub plate: String,
    pub bike_model: String,
    pub bike_color: String,
    pub photo_file_id: String,
    pub bike_photo_file_id: Option<String>,
    pub payment_methods: PaymentMethods,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Driver {
    pub id: UserId,
    pub profile: Profile,
    pub status: DriverStatus,
}

pub const ALIAS_MAX: usize = 32;
pub const FIELD_MAX: usize = 40;

/// Trims a short free-text answer (alias, plate, bike model or color) and
/// rejects empty or overly long ones.
pub fn clean_field(input: &str, max_chars: usize) -> Option<String> {
    let text = input.split_whitespace().collect::<Vec<_>>().join(" ");
    let len = text.chars().count();
    (1..=max_chars).contains(&len).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payment_methods_round_trip() {
        for m in [PaymentMethods::CASH, PaymentMethods::TRANSFER, PaymentMethods::BOTH] {
            assert_eq!(PaymentMethods::parse(m.as_str()), Some(m));
        }
    }

    #[test]
    fn clean_field_collapses_spaces_and_checks_length() {
        assert_eq!(clean_field("  Suzuki   AX 100 ", FIELD_MAX).as_deref(), Some("Suzuki AX 100"));
        assert!(clean_field("   ", FIELD_MAX).is_none());
        assert!(clean_field(&"a".repeat(33), ALIAS_MAX).is_none());
    }

    #[test]
    fn status_parses_the_column_values() {
        assert_eq!(DriverStatus::parse("pending"), Some(DriverStatus::Pending));
        assert_eq!(DriverStatus::parse("active"), Some(DriverStatus::Active));
        assert_eq!(DriverStatus::parse("blocked"), Some(DriverStatus::Blocked));
        assert_eq!(DriverStatus::parse("other"), None);
    }
}
