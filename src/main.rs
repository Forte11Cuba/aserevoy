mod config;
mod crypto;
mod db;
mod domain;
mod i18n;

use tracing_subscriber::EnvFilter;

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

    if config.telegram_token.is_none() {
        tracing::warn!("TELEGRAM_TOKEN is not set; the bot is not started yet");
    }
    Ok(())
}
