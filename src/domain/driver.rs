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

/// What the driver fills in. The phone and full name are in clear here; the
/// database layer encrypts them.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Profile {
    pub alias: String,
    /// As on their ID, for staff and, once a trip is assigned, the client.
    /// `None` for drivers from before it was asked.
    #[serde(default)]
    pub full_name: Option<String>,
    pub phone: String,
    pub plate: String,
    pub bike_model: String,
    pub bike_color: String,
    pub photo_file_id: String,
    pub bike_photo_file_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Driver {
    pub id: UserId,
    pub profile: Profile,
    pub status: DriverStatus,
    /// A new full name waiting for an admin's approval.
    pub pending_full_name: Option<String>,
}

pub const ALIAS_MAX: usize = 32;
pub const FULL_NAME_MAX: usize = 80;

/// A full name has at least two words (name and surname).
pub fn clean_full_name(input: &str) -> Option<String> {
    clean_field(input, FULL_NAME_MAX).filter(|name| name.split(' ').filter(|w| w.chars().any(char::is_alphabetic)).count() >= 2)
}
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
