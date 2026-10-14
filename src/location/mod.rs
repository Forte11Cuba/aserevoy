//! Where the trip starts and ends, and how far it is. Phase 1 only takes pins;
//! typed addresses (rules, `streets.sqlite`, Jev) come in phase 4.

pub mod jev;
pub mod maplink;
pub mod routing;
pub mod streets;
pub mod zones;
