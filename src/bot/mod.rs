//! Telegram layer: reads updates, calls the domain and the database, and runs
//! the effects the domain returns. Roles are checked here on every action.

pub mod client;
pub mod dialog;
pub mod dispatcher;
pub mod driver;
pub mod effects;
pub mod keyboards;
pub mod trip;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::SqlitePool;
use teloxide::types::{ChatId, KeyboardMarkup, User};

use crate::config::Config;
use crate::crypto::PhoneCipher;
use crate::db;
use crate::domain::driver::DriverStatus;
use crate::domain::ids::UserId;
use crate::domain::request::UnixTime;
use crate::i18n::Lang;
use crate::location::routing::Osrm;

pub type HandlerResult = anyhow::Result<()>;

pub struct App {
    pub pool: SqlitePool,
    pub config: Config,
    pub cipher: PhoneCipher,
    pub osrm: Osrm,
    /// The offers channel.
    pub channel: ChatId,
    /// Join-request link to the channel, sent to approved drivers.
    pub channel_link: String,
    pub bot_username: String,
    /// Language Telegram reported for each user seen since start, to notify
    /// them in it when someone else triggers the message. Not persisted.
    seen_langs: Mutex<HashMap<i64, Lang>>,
}

pub type AppRef = Arc<App>;

impl App {
    pub fn new(
        pool: SqlitePool,
        config: Config,
        cipher: PhoneCipher,
        osrm: Osrm,
        channel: ChatId,
        channel_link: String,
        bot_username: String,
    ) -> App {
        App { pool, config, cipher, osrm, channel, channel_link, bot_username, seen_langs: Mutex::default() }
    }

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
    pub async fn menu(&self, user: UserId, lang: Lang) -> anyhow::Result<KeyboardMarkup> {
        let is_driver = self.driver_status(user).await?.is_some();
        Ok(keyboards::main_menu(lang, is_driver, self.is_staff(user).await?))
    }

    pub async fn driver_status(&self, user: UserId) -> anyhow::Result<Option<DriverStatus>> {
        Ok(db::drivers::status(&self.pool, user).await?)
    }
}

pub fn user_id(user: &User) -> UserId {
    UserId(user.id.0 as i64)
}

pub fn chat(user: UserId) -> ChatId {
    ChatId(user.0)
}

pub fn now() -> UnixTime {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as UnixTime).unwrap_or_default()
}
