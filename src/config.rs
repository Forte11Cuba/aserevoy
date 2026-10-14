use std::env;

use anyhow::{Context, bail};

use crate::domain::fare::Fares;
use crate::domain::ids::UserId;

/// Settings read from the environment (`.env` in development). See `.env.example`.
#[derive(Debug, Clone)]
pub struct Config {
    /// Without it the program only prepares the database and exits.
    pub telegram_token: Option<String>,
    pub database_url: String,
    pub superadmins: Vec<UserId>,
    pub offers_channel_id: Option<i64>,
    /// Private channel where staff alerts go; without it, each admin gets
    /// them in their chat with the bot.
    pub alerts_channel_id: Option<i64>,
    /// Hex key that encrypts drivers' phones at rest.
    pub phone_key: Option<String>,
    /// Hex key for the fingerprints of client-driver pairs (ratings).
    pub pair_key: Option<String>,
    /// The human Telegram account that answers support (without the @).
    pub support_username: Option<String>,
    pub osrm_url: String,
    /// Jev (TypeSafe) picks among candidate places for written addresses.
    pub typesafe_api_key: Option<String>,
    pub jev_enabled: bool,
    /// Streets, corners and places from OSM (`scripts/prepare_osm.py`).
    pub streets_path: String,
    pub default_fares: Fares,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Config> {
        let superadmins = optional("SUPERADMIN_IDS")
            .unwrap_or_default()
            .split(',')
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.trim().parse().map(UserId).context("SUPERADMIN_IDS must be numeric Telegram ids"))
            .collect::<anyhow::Result<Vec<_>>>()?;
        if superadmins.is_empty() {
            bail!("SUPERADMIN_IDS needs at least one Telegram id");
        }

        let min_percent: u8 = parse_or("FARE_MIN_PERCENT", 80)?;
        if min_percent > 100 {
            bail!("FARE_MIN_PERCENT must be between 0 and 100");
        }

        Ok(Config {
            telegram_token: optional("TELEGRAM_TOKEN"),
            database_url: optional("DATABASE_URL").unwrap_or_else(|| "sqlite://aserevoy.db".into()),
            superadmins,
            offers_channel_id: optional("OFFERS_CHANNEL_ID")
                .map(|v| v.parse().context("OFFERS_CHANNEL_ID must be numeric"))
                .transpose()?,
            alerts_channel_id: optional("ALERTS_CHANNEL_ID")
                .map(|v| v.parse().context("ALERTS_CHANNEL_ID must be numeric"))
                .transpose()?,
            phone_key: optional("PHONE_KEY"),
            pair_key: optional("PAIR_KEY"),
            support_username: optional("SUPPORT_USERNAME").map(|u| u.trim().trim_start_matches('@').to_owned()),
            osrm_url: optional("OSRM_URL").unwrap_or_else(|| "http://localhost:5000".into()),
            typesafe_api_key: optional("TYPESAFE_API_KEY"),
            jev_enabled: parse_or("JEV_ENABLED", false)?,
            streets_path: optional("STREETS_DB").unwrap_or_else(|| "data/streets.sqlite".into()),
            default_fares: Fares {
                base: parse_or("FARE_BASE", 100)?,
                per_km: parse_or("FARE_PER_KM", 150)?,
                min_percent,
            },
        })
    }

    pub fn is_superadmin(&self, user: UserId) -> bool {
        self.superadmins.contains(&user)
    }
}

fn optional(key: &str) -> Option<String> {
    env::var(key).ok().filter(|v| !v.trim().is_empty())
}

fn parse_or<T: std::str::FromStr>(key: &str, default: T) -> anyhow::Result<T> {
    match optional(key) {
        Some(v) => v.trim().parse().ok().with_context(|| format!("{key} has an invalid value")),
        None => Ok(default),
    }
}
