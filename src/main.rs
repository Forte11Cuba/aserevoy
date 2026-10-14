mod bot;
mod config;
mod crypto;
mod db;
mod domain;
mod i18n;
mod location;
mod tasks;

use std::sync::Arc;

use anyhow::Context;
use teloxide::prelude::*;
use tracing_subscriber::EnvFilter;

use crate::bot::App;
use crate::crypto::{PairKey, PhoneCipher};
use crate::location::routing::Osrm;
use crate::location::jev::Jev;
use crate::location::streets::Streets;
use crate::location::zones::Zones;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    // Logs carry events and short ids only: never locations, addresses or phones.
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "aserevoy=info".into()))
        .init();

    let config = config::Config::from_env()?;
    let pool = db::connect(&config.database_url).await?;
    db::fares::seed(&pool, &config.default_fares).await?;
    let fares = db::fares::load(&pool).await?;
    tracing::info!(?fares, superadmins = config.superadmins.len(), "database ready");

    let Some(token) = config.telegram_token.clone() else {
        tracing::warn!("TELEGRAM_TOKEN is not set; the bot is not started");
        return Ok(());
    };
    let channel = ChatId(config.offers_channel_id.context("OFFERS_CHANNEL_ID is required to run the bot")?);
    let cipher = PhoneCipher::from_hex(config.phone_key.as_deref().context("PHONE_KEY is required to run the bot")?)?;
    let pair_key = PairKey::from_hex(config.pair_key.as_deref().context("PAIR_KEY is required to run the bot")?)?;
    if config.support_username.is_none() {
        tracing::warn!("SUPPORT_USERNAME is not set; the Support button will say it is unavailable");
    }
    let osrm = Osrm::new(&config.osrm_url)?;
    let zones = Zones::bundled()?;
    let streets = Streets::load(&config.streets_path).await.context("cannot read the streets database")?;
    let jev = match (config.jev_enabled, config.typesafe_api_key.as_deref()) {
        (true, Some(key)) => Some(Jev::new(key)?),
        (true, None) => {
            tracing::warn!("JEV_ENABLED is set but TYPESAFE_API_KEY is not; written addresses work without Jev");
            None
        }
        (false, _) => None,
    };

    let bot = Bot::new(token);
    let me = bot.get_me().await.context("cannot reach Telegram with TELEGRAM_TOKEN")?;
    let channel_link = channel_link(&bot, &pool, channel).await?;

    let app = Arc::new(App {
        pool,
        config,
        cipher,
        pair_key,
        osrm,
        zones,
        streets,
        jev,
        channel,
        channel_link,
        bot_username: me.username().to_owned(),
        seen_langs: Default::default(),
        last_action: Default::default(),
    });
    tokio::spawn(tasks::sweeper::run(bot.clone(), app.clone()));
    tracing::info!("bot started");
    bot::dispatcher::run(bot, app).await;
    Ok(())
}

/// The join-request link to the offers channel, created once per channel and
/// reused, so restarts do not pile up links.
async fn channel_link(bot: &Bot, pool: &sqlx::SqlitePool, channel: ChatId) -> anyhow::Result<String> {
    let key = format!("channel_link:{}", channel.0);
    if let Some(link) = db::settings::get(pool, &key).await? {
        return Ok(link);
    }
    let link = bot
        .create_chat_invite_link(channel)
        .name("Asere Voy")
        .creates_join_request(true)
        .await
        .context("the bot must be an admin of the offers channel, allowed to invite users")?
        .invite_link;
    db::settings::set(pool, &key, &link).await?;
    Ok(link)
}
