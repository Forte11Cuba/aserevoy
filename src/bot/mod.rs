//! Telegram layer: reads updates, calls the domain and the database, and runs
//! the effects the domain returns. Roles are checked here on every action.

pub mod admin;
pub mod auction;
pub mod client;
pub mod dialog;
pub mod dispatcher;
pub mod driver;
pub mod effects;
pub mod history;
pub mod keyboards;
pub mod panel;
pub mod rating;
pub mod settings;
pub mod support;
pub mod trip;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use sqlx::SqlitePool;
use teloxide::types::{ChatId, KeyboardMarkup, User};

use crate::config::Config;
use crate::crypto::{PairKey, PhoneCipher};
use crate::db;
use crate::domain::driver::DriverStatus;
use crate::domain::ids::UserId;
use crate::domain::reputation::Role;
use crate::domain::request::{State, UnixTime};
use crate::i18n::{self, Lang, TERMS_VERSION};
use crate::location::routing::Osrm;
use crate::location::jev::Jev;
use crate::location::maplink::MapLinks;
use crate::location::streets::Streets;
use crate::location::zones::Zones;

pub type HandlerResult = anyhow::Result<()>;

pub struct App {
    pub pool: SqlitePool,
    pub config: Config,
    pub cipher: PhoneCipher,
    pub pair_key: PairKey,
    pub osrm: Osrm,
    pub zones: Zones,
    /// Streets, corners and places of OSM, for written addresses.
    pub streets: Streets,
    /// Picks among candidate places; `None` when disabled (`JEV_ENABLED`).
    pub jev: Option<Jev>,
    /// Coordinates and map links sent instead of a pin.
    pub map_links: MapLinks,
    /// The offers channel.
    pub channel: ChatId,
    /// Join-request link to the channel, sent to approved drivers.
    pub channel_link: String,
    pub bot_username: String,
    /// Language Telegram reported for each user seen since start, to notify
    /// them in it when someone else triggers the message. Not persisted;
    /// starts empty.
    pub seen_langs: Mutex<HashMap<i64, Lang>>,
    /// Last action of each user, for the anti-flood (1 per second).
    pub last_action: Mutex<HashMap<i64, Instant>>,
}

/// At most one action per second per user; extra taps are ignored.
const FLOOD_INTERVAL: Duration = Duration::from_secs(1);

pub type AppRef = Arc<App>;

impl App {
    /// The language for the user who sent an update: their saved choice, or
    /// else what their Telegram app uses.
    pub async fn lang_for(&self, user: &User) -> anyhow::Result<Lang> {
        let telegram = Lang::from_telegram(user.language_code.as_deref());
        self.seen_langs.lock().unwrap().insert(user_id(user).0, telegram);
        Ok(db::users::lang(&self.pool, user_id(user)).await?.unwrap_or(telegram))
    }

    /// The language for a user we are notifying.
    pub async fn lang_of(&self, user: UserId) -> Lang {
        if let Ok(Some(lang)) = db::users::lang(&self.pool, user).await {
            return lang;
        }
        self.seen_langs.lock().unwrap().get(&user.0).copied().unwrap_or_default()
    }

    /// Superadmins (from `.env`) and admins (appointed in the bot).
    pub async fn is_staff(&self, user: UserId) -> anyhow::Result<bool> {
        Ok(self.config.is_superadmin(user) || db::users::is_admin(&self.pool, user).await?)
    }

    pub async fn staff(&self) -> anyhow::Result<Vec<UserId>> {
        let mut ids = self.config.superadmins.clone();
        for admin in db::users::admins(&self.pool).await? {
            if !ids.contains(&admin) {
                ids.push(admin);
            }
        }
        Ok(ids)
    }

    /// The main menu, with the buttons this user's roles allow.
    /// The menu below: the trip's while one is assigned (the driver's or the
    /// client's, by their part in it), the main menu otherwise. Every message that
    /// starts or ends a trip carries it, so it follows the trip.
    pub async fn menu(&self, user: UserId, lang: Lang) -> anyhow::Result<KeyboardMarkup> {
        if let Some(trip) = db::requests::active_for(&self.pool, user).await?
            && trip.state == State::Assigned
        {
            return Ok(if trip.driver == Some(user) {
                keyboards::driver_trip_menu(lang)
            } else {
                keyboards::client_trip_menu(lang)
            });
        }
        let is_driver = self.driver_status(user).await?.is_some();
        Ok(keyboards::main_menu(lang, is_driver, self.is_staff(user).await?))
    }

    /// True if the user acted less than a second ago (and then this action is
    /// dropped); records the action otherwise.
    pub fn too_fast(&self, user: UserId) -> bool {
        let now = Instant::now();
        let mut last = self.last_action.lock().unwrap();
        if last.get(&user.0).is_some_and(|t| now.duration_since(*t) < FLOOD_INTERVAL) {
            return true;
        }
        last.insert(user.0, now);
        // Keep the map small: forget users idle for a minute.
        if last.len() > 10_000 {
            last.retain(|_, t| now.duration_since(*t) < Duration::from_secs(60));
        }
        false
    }

    pub async fn accepted_terms(&self, user: UserId) -> anyhow::Result<bool> {
        Ok(db::support::terms_version(&self.pool, user).await?.is_some_and(|v| v >= TERMS_VERSION))
    }

    /// The message to show if the user is blocked in `role`.
    pub async fn blocked_message(&self, user: UserId, role: Role, lang: Lang) -> anyhow::Result<Option<String>> {
        let until = db::moderation::blocked_until(&self.pool, user, role, now()).await?;
        Ok(until.map(|until| i18n::blocked(lang, until.map(local_time).as_deref())))
    }

    pub async fn driver_status(&self, user: UserId) -> anyhow::Result<Option<DriverStatus>> {
        Ok(db::drivers::status(&self.pool, user).await?)
    }

    /// What the driver of this user's trip gets about them: their optional
    /// client details. A driver requesting a ride is a client like any other:
    /// their driver profile is kept apart.
    pub async fn client_contact(&self, user: UserId) -> anyhow::Result<db::clients::ClientProfile> {
        Ok(db::clients::get(&self.pool, &self.cipher, user).await?.unwrap_or_default())
    }
}

pub fn user_id(user: &User) -> UserId {
    UserId(user.id.0 as i64)
}

pub fn chat(user: UserId) -> ChatId {
    ChatId(user.0)
}

/// A time as people in Cuba read it: "14/10 18:05".
pub fn local_time(t: UnixTime) -> String {
    use chrono::TimeZone;
    match chrono_tz::America::Havana.timestamp_opt(t, 0) {
        chrono::LocalResult::Single(dt) => dt.format("%d/%m %H:%M").to_string(),
        _ => String::new(),
    }
}

/// The Cuban date of a time, "2026-10-14", for the daily metrics.
pub fn local_date(t: UnixTime) -> String {
    use chrono::TimeZone;
    match chrono_tz::America::Havana.timestamp_opt(t, 0) {
        chrono::LocalResult::Single(dt) => dt.format("%Y-%m-%d").to_string(),
        _ => String::new(),
    }
}

pub fn now() -> UnixTime {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as UnixTime).unwrap_or_default()
}
