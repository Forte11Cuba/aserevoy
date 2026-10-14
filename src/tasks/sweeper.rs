//! Every 10 s: asks "still waiting?" and expires requests, releases drivers
//! the client did not confirm, auto-completes forgotten trips, drops expired
//! counteroffers, invitations and idle conversations.
//! Deadlines live in the database, so a restart loses nothing.

use std::time::Duration;

use teloxide::Bot;

use crate::bot::{App, AppRef, effects, now};
use crate::db;
use crate::domain::effect::Effect;
use crate::domain::request::{AUTO_COMPLETE_SECS, Request, Tick, UnixTime};

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
    for request in db::requests::due(&app.pool, now, AUTO_COMPLETE_SECS).await? {
        let id = request.id.clone();
        // One failing request must not hold back the others.
        if let Err(e) = handle_due(bot, app, request, now).await {
            tracing::error!(request = %id, error = %e, "sweeping request failed");
        }
    }
    for id in db::counteroffers::delete_expired(&app.pool, now).await? {
        if let Some(mut request) = db::requests::get(&app.pool, &id).await? {
            let refresh = vec![Effect::RefreshOffers { request: id.clone() }];
            if let Err(e) = effects::apply(bot, app, &mut request, refresh).await {
                tracing::error!(request = %id, error = %e, "refreshing offers failed");
            }
        }
    }
    db::invitations::delete_expired(&app.pool, now).await?;
    db::dialogs::delete_idle(&app.pool, now).await?;
    Ok(())
}

async fn handle_due(bot: &Bot, app: &App, mut request: Request, now: UnixTime) -> anyhow::Result<()> {
    let before = request.state;
    match request.tick(now) {
        // A user may have acted in the meantime: only the winner applies.
        Some(Tick::Close(effects)) => {
            if db::requests::close(&app.pool, &request.id, before).await? {
                tracing::info!(request = %request.id, state = request.state.as_str(), "closed by sweeper");
                effects::apply(bot, app, &mut request, effects).await?;
            }
        }
        Some(Tick::AskStillWaiting(effects)) => {
            let answer_by = request.answer_by.expect("asking sets the deadline");
            if db::requests::ask_still_waiting(&app.pool, &request.id, answer_by).await? {
                tracing::info!(request = %request.id, "asked still waiting");
                effects::apply(bot, app, &mut request, effects).await?;
            }
        }
        None => {}
    }
    Ok(())
}
