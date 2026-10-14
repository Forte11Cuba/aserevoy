//! Every 10 s: expires open requests past their round, auto-completes
//! forgotten trips, and drops expired invitations and idle conversations.
//! Deadlines live in the database, so a restart loses nothing.

use std::time::Duration;

use teloxide::Bot;

use crate::bot::{App, AppRef, effects, now};
use crate::db;
use crate::domain::request::AUTO_COMPLETE_SECS;

const EVERY: Duration = Duration::from_secs(10);

pub async fn run(bot: Bot, app: AppRef) {
    let mut interval = tokio::time::interval(EVERY);
    loop {
        interval.tick().await;
        if let Err(e) = sweep(&bot, &app).await {
            tracing::error!(error = %e, "sweep failed");
        }
    }
}

async fn sweep(bot: &Bot, app: &App) -> anyhow::Result<()> {
    let now = now();
    for mut request in db::requests::due(&app.pool, now, AUTO_COMPLETE_SECS).await? {
        let before = request.state;
        let Some(effects) = request.tick(now) else { continue };
        // A user may have closed it in the meantime.
        if db::requests::close(&app.pool, &request.id, before).await? {
            tracing::info!(request = %request.id, state = request.state.as_str(), "closed by sweeper");
            effects::apply(bot, app, &mut request, effects).await?;
        }
    }
    db::invitations::delete_expired(&app.pool, now).await?;
    db::dialogs::delete_idle(&app.pool, now).await?;
    Ok(())
}
