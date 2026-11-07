//! Business rules. Nothing here depends on Telegram or the database.

pub mod driver;
pub mod effect;
pub mod fare;
pub mod ids;
pub mod phone;
pub mod reputation;
pub mod request;
// Temporary until the vehicles work wires it in (issue #29).
#[allow(dead_code)]
pub mod vehicle;
