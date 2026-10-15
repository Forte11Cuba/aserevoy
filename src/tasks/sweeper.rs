//! Every 10 s: asks "still waiting?" and expires requests, releases drivers
//! the client did not confirm, auto-completes forgotten trips, drops expired
//! counteroffers (and their messages), applies blind ratings that are due,
//! and deletes expired invitations, conversations, cancellations, blocks,
//! pair fingerprints and support codes.
//! Deadlines live in the database, so a restart loses nothing.

use std::time::Duration;

use teloxide::prelude::*;
use teloxide::types::MessageId;

use crate::bot::{App, AppRef, chat, effects, now, rating};
use crate::db;
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
    for expired in db::counteroffers::take_expired(&app.pool, now).await? {
        if let Some(msg) = expired.client_msg_id
            && let Err(e) = bot.delete_message(chat(expired.client), MessageId(msg)).await
        {
            tracing::warn!(request = %expired.request, error = %e, "hiding expired offer failed");
        }
    }
    if let Err(e) = rating::apply_due(bot, app).await {
        tracing::error!(error = %e, "applying ratings failed");
    }
    db::invitations::delete_expired(&app.pool, now).await?;
    db::dialogs::delete_idle(&app.pool, now).await?;
    db::moderation::delete_old_cancellations(&app.pool, now).await?;
    db::history::delete_old(&app.pool, now).await?;
    db::moderation::delete_expired_blocks(&app.pool, now).await?;
    db::reputation::delete_expired_pairs(&app.pool, now).await?;
    db::support::delete_expired_codes(&app.pool, now).await?;
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
