//! Blind ratings after a finished trip: each side picks 1 to 5 stars and
//! optional tags; nothing is added to the totals until both finished or 24 h
//! passed, so neither can retaliate. The same pair counts once a week.

use teloxide::prelude::*;
use teloxide::types::MessageId;

use super::client::Ctx;
use super::{App, chat, keyboards, now, support};
use crate::db;
use crate::db::reputation::Pending;
use crate::domain::ids::{UserId, short_code};
use crate::domain::reputation::{RATING_WINDOW_SECS, Role, Tag, valid_stars};
use crate::i18n::{self, Text};

/// A trip finished: counts it for both and asks both for a rating. Returns
/// the rating's code (the history records each side's stars by it).
pub async fn start(bot: &Bot, app: &App, client: UserId, driver: UserId) -> anyhow::Result<String> {
    let now = now();
    db::reputation::add_trip(&app.pool, client, Role::Client, now).await?;
    db::reputation::add_trip(&app.pool, driver, Role::Driver, now).await?;
    let pair = app.pair_key.fingerprint(client.0, driver.0);
    let counts = db::reputation::pair_counts(&app.pool, &pair, now).await?;
    let code = short_code(&mut rand::rng(), 8);
    db::reputation::create_pending(&app.pool, &code, client, driver, counts, now + RATING_WINDOW_SECS).await?;
    for (rater, rated) in [(client, Role::Driver), (driver, Role::Client)] {
        let lang = app.lang_of(rater).await;
        let sent = bot.send_message(chat(rater), i18n::rate_prompt(lang, rated)).reply_markup(keyboards::stars(&code)).await;
        if let Err(e) = sent {
            tracing::warn!(error = %e, "cannot ask for a rating");
        }
    }
    Ok(code)
}

/// The pending rating and the presser's role in it, if they are part of it.
async fn mine(cx: &Ctx<'_>, code: &str) -> anyhow::Result<Option<(Pending, Role)>> {
    let pending = db::reputation::get_pending(&cx.app.pool, code).await?;
    Ok(pending.and_then(|p| p.rater_role(cx.user).map(|role| (p, role))))
}

fn rated(rater: Role) -> Role {
    match rater {
        Role::Client => Role::Driver,
        Role::Driver => Role::Client,
    }
}

/// Redraws the rating message: chosen stars, tags with ✓, and Done.
async fn redraw(cx: &Ctx<'_>, msg: MessageId, pending: &Pending, rater: Role) -> anyhow::Result<()> {
    let side = pending.side(rater);
    let stars = "⭐".repeat(side.stars.unwrap_or(0) as usize);
    let text = format!(
        "{}\n\n{stars}\n{}",
        i18n::rate_prompt(cx.lang, rated(rater)),
        i18n::text(cx.lang, Text::RatingTags)
    );
    let markup = keyboards::rating_tags(cx.lang, &pending.code, Tag::for_rating(rated(rater)), side.tags);
    let _ = cx.bot.edit_message_text(chat(cx.user), msg, text).reply_markup(markup).await;
    Ok(())
}

/// `rs:<code>:<stars>`.
pub async fn stars(cx: &Ctx<'_>, code: &str, stars: i64, pressed: Option<MessageId>) -> anyhow::Result<()> {
    let Some((_, rater)) = mine(cx, code).await? else { return Ok(()) };
    if !valid_stars(stars) || !db::reputation::set_stars(&cx.app.pool, code, rater, stars).await? {
        return Ok(());
    }
    // The stars this user gave, for their own history only.
    db::history::set_stars(&cx.app.pool, cx.user, code, stars).await?;
    if let (Some(msg), Some((pending, _))) = (pressed, mine(cx, code).await?) {
        redraw(cx, msg, &pending, rater).await?;
    }
    Ok(())
}

/// `rtg:<code>:<tag>`.
pub async fn toggle(cx: &Ctx<'_>, code: &str, tag: &str, pressed: Option<MessageId>) -> anyhow::Result<()> {
    let (Some((_, rater)), Some(tag)) = (mine(cx, code).await?, Tag::parse(tag)) else { return Ok(()) };
    if !Tag::for_rating(rated(rater)).contains(&tag) || !db::reputation::toggle_tag(&cx.app.pool, code, rater, tag).await? {
        return Ok(());
    }
    if let (Some(msg), Some((pending, _))) = (pressed, mine(cx, code).await?) {
        redraw(cx, msg, &pending, rater).await?;
    }
    Ok(())
}

/// `rdn:<code>`: this side is done; the rating stays blind until applied.
pub async fn done(cx: &Ctx<'_>, code: &str, pressed: Option<MessageId>) -> anyhow::Result<()> {
    let Some((_, rater)) = mine(cx, code).await? else { return Ok(()) };
    if db::reputation::finish_side(&cx.app.pool, code, rater).await?
        && let Some(msg) = pressed
    {
        let _ = cx.bot.edit_message_text(chat(cx.user), msg, i18n::text(cx.lang, Text::RatingThanks)).await;
    }
    Ok(())
}

/// Sweeper: adds the ratings whose both sides finished or whose 24 h passed,
/// alerting the admins when someone's average falls below 3.5.
pub async fn apply_due(bot: &Bot, app: &App) -> anyhow::Result<()> {
    let now = now();
    for code in db::reputation::due_pending(&app.pool, now).await? {
        let Some(pending) = db::reputation::take_pending(&app.pool, &code).await? else { continue };
        if !pending.counts {
            continue; // the same pair rated each other this week
        }
        // The client's rating is about the driver, and the other way round.
        let sides = [(pending.by_client, pending.driver, Role::Driver), (pending.by_driver, pending.client, Role::Client)];
        for (side, rated_user, role) in sides {
            let Some(rating) = side.rating() else { continue };
            let before = db::reputation::get(&app.pool, rated_user, role).await?.unwrap_or_default();
            db::reputation::add_rating(&app.pool, rated_user, role, rating, now).await?;
            if before.becomes_low_with(rating) {
                let shown = db::reputation::get(&app.pool, rated_user, role).await?.unwrap_or_default().shown(now);
                let is_driver = app.driver_status(rated_user).await?.is_some();
                let text = |lang| i18n::alert_low_rating(lang, role, &shown);
                let markup = |lang| keyboards::moderate(lang, rated_user, is_driver);
                support::alert(bot, app, text, None, markup).await?;
            }
        }
    }
    Ok(())
}
