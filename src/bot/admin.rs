//! Staff actions by buttons (block, unblock, remove a driver) and the
//! superadmins' management of admins. Roles are checked on every press.

use teloxide::prelude::*;

use super::client::Ctx;
use super::{App, chat, keyboards, now, support};
use crate::db;
use crate::db::invitations::InviteRole;
use crate::db::moderation::BlockReason;
use crate::domain::driver::DriverStatus;
use crate::domain::fare::FareKey;
use crate::domain::ids::{InviteCode, UserId};
use crate::domain::reputation::{CANCELLATION_BLOCK_SECS, Role};
use crate::i18n::{self, Text};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Block24h,
    Block,
    Unblock,
    RemoveDriver,
}

impl Action {
    pub fn parse(code: &str) -> Option<Action> {
        match code {
            "b24" => Some(Action::Block24h),
            "bl" => Some(Action::Block),
            "ub" => Some(Action::Unblock),
            "rd" => Some(Action::RemoveDriver),
            _ => None,
        }
    }
}

/// `adm:<action>:<user>` from a support code lookup or an alert.
/// Admins cannot act on superadmins, nor on other admins (only superadmins can).
pub async fn moderate(cx: &Ctx<'_>, action: Action, target: UserId) -> anyhow::Result<String> {
    let app = cx.app;
    if !app.is_staff(cx.user).await? {
        return Ok(i18n::text(cx.lang, Text::NotAllowed).into());
    }
    let target_is_staff = app.is_staff(target).await?;
    if app.config.is_superadmin(target) || (target_is_staff && !app.config.is_superadmin(cx.user)) {
        return Ok(i18n::text(cx.lang, Text::NotAllowed).into());
    }
    let now = now();
    let target_lang = app.lang_of(target).await;
    let tell = |text: String| async move {
        let _ = cx.bot.send_message(chat(target), text).await;
    };
    let done = match action {
        Action::Block24h => {
            let until = now + CANCELLATION_BLOCK_SECS;
            db::moderation::block(&app.pool, target, Role::Client, Some(until), BlockReason::Admin, cx.user, now).await?;
            tell(i18n::blocked(target_lang, Some(&super::local_time(until)))).await;
            Text::StaffBlocked
        }
        Action::Block => {
            db::moderation::block(&app.pool, target, Role::Client, None, BlockReason::Admin, cx.user, now).await?;
            if app.driver_status(target).await?.is_some() {
                db::moderation::block(&app.pool, target, Role::Driver, None, BlockReason::Admin, cx.user, now).await?;
                db::drivers::set_status(&app.pool, target, DriverStatus::Blocked).await?;
                kick_from_channel(cx.bot, app, target, false).await;
            }
            tell(i18n::blocked(target_lang, None)).await;
            Text::StaffBlocked
        }
        Action::Unblock => {
            db::moderation::unblock(&app.pool, target).await?;
            if app.driver_status(target).await? == Some(DriverStatus::Blocked) {
                db::drivers::set_status(&app.pool, target, DriverStatus::Active).await?;
                let id = teloxide::types::UserId(target.0 as u64);
                let _ = cx.bot.unban_chat_member(app.channel, id).await;
            }
            tell(i18n::text(target_lang, Text::YouWereUnblocked).into()).await;
            Text::StaffUnblocked
        }
        Action::RemoveDriver => {
            if !db::drivers::remove(&app.pool, target).await? {
                return Ok(i18n::text(cx.lang, Text::AlreadyDecided).into());
            }
            kick_from_channel(cx.bot, app, target, true).await;
            tell(i18n::text(target_lang, Text::YouWereRemoved).into()).await;
            Text::DriverRemoved
        }
    };
    tracing::info!(?action, "staff action");
    Ok(i18n::text(cx.lang, done).into())
}

/// Takes a driver out of the offers channel. With `allow_back`, they may ask
/// to join again later (and are only let in if active).
async fn kick_from_channel(bot: &Bot, app: &App, driver: UserId, allow_back: bool) {
    let id = teloxide::types::UserId(driver.0 as u64);
    if let Err(e) = bot.ban_chat_member(app.channel, id).await {
        tracing::warn!(error = %e, "cannot remove driver from the channel");
    }
    if allow_back {
        let _ = bot.unban_chat_member(app.channel, id).await;
    }
}

/// `/metricas` (superadmins): the last 7 days, aggregates only.
pub async fn show_metrics(cx: &Ctx<'_>) -> anyhow::Result<()> {
    if !cx.app.config.is_superadmin(cx.user) {
        cx.bot.send_message(chat(cx.user), i18n::text(cx.lang, Text::NotAllowed)).await?;
        return Ok(());
    }
    let days = db::metrics::last(&cx.app.pool, 7).await?;
    cx.bot.send_message(chat(cx.user), i18n::metrics(cx.lang, &days)).await?;
    Ok(())
}

/// `/tarifas` (staff): the current fares.
pub async fn show_fares(cx: &Ctx<'_>) -> anyhow::Result<()> {
    if !cx.app.is_staff(cx.user).await? {
        cx.bot.send_message(chat(cx.user), i18n::text(cx.lang, Text::NotAllowed)).await?;
        return Ok(());
    }
    let fares = db::fares::load(&cx.app.pool).await?;
    cx.bot.send_message(chat(cx.user), i18n::fares(cx.lang, &fares)).await?;
    Ok(())
}

/// `/tarifa <base|km|minimo> <value>` (superadmins): changes a fare for new
/// requests; open ones keep their price.
pub async fn set_fare(cx: &Ctx<'_>, args: &str) -> anyhow::Result<()> {
    if !cx.app.config.is_superadmin(cx.user) {
        cx.bot.send_message(chat(cx.user), i18n::text(cx.lang, Text::NotAllowed)).await?;
        return Ok(());
    }
    let mut words = args.split_whitespace();
    let parsed = match (words.next().and_then(FareKey::parse), words.next().and_then(|v| v.parse().ok())) {
        (Some(key), Some(value)) if key.valid(value) => Some((key, value)),
        _ => None,
    };
    let Some((key, value)) = parsed else {
        cx.bot.send_message(chat(cx.user), i18n::text(cx.lang, Text::FareUsage)).await?;
        return Ok(());
    };
    db::fares::set(&cx.app.pool, key, value).await?;
    tracing::info!(?key, value, "fare changed");
    show_fares(cx).await
}

/// "👮 Admins" (superadmins only): the list with Remove buttons, and Invite.
pub async fn show_admins(cx: &Ctx<'_>) -> anyhow::Result<()> {
    if !cx.app.config.is_superadmin(cx.user) {
        cx.bot.send_message(chat(cx.user), i18n::text(cx.lang, Text::NotAllowed)).await?;
        return Ok(());
    }
    // Names are looked up now; we do not store them.
    let name_of = |id: UserId| async move {
        match cx.bot.get_chat(chat(id)).await {
            Ok(c) => {
                let name = c.first_name().map(str::to_owned).unwrap_or_else(|| id.0.to_string());
                match c.username() {
                    Some(u) => format!("{name} (@{u})"),
                    None => name,
                }
            }
            Err(_) => id.0.to_string(),
        }
    };
    let mut superadmins = Vec::new();
    for id in cx.app.config.superadmins.clone() {
        superadmins.push(name_of(id).await);
    }
    let mut admins = Vec::new();
    for id in db::users::admins(&cx.app.pool).await? {
        admins.push((id, name_of(id).await));
    }
    let names: Vec<String> = admins.iter().map(|(_, n)| n.clone()).collect();
    let text = i18n::admins_list(cx.lang, &superadmins, &names);
    cx.bot.send_message(chat(cx.user), text).reply_markup(keyboards::admins(cx.lang, &admins)).await?;
    Ok(())
}

/// `ina`: a one-use link that appoints whoever opens it as admin.
pub async fn invite_admin(cx: &Ctx<'_>) -> anyhow::Result<Option<String>> {
    if !cx.app.config.is_superadmin(cx.user) {
        return Ok(Some(i18n::text(cx.lang, Text::NotAllowed).into()));
    }
    let code = InviteCode::generate(&mut rand::rng());
    db::invitations::create(&cx.app.pool, &code, InviteRole::Admin, cx.user, now()).await?;
    let link = format!("https://t.me/{}?start=adm_{}", cx.app.bot_username, code.as_str());
    cx.bot.send_message(chat(cx.user), i18n::admin_invite_created(cx.lang, &link)).await?;
    Ok(None)
}

/// `/start adm_<code>`: the invited person becomes an admin.
pub async fn redeem_admin_invite(cx: &Ctx<'_>, code: &str) -> anyhow::Result<()> {
    let app = cx.app;
    let redeemed = match InviteCode::parse(code) {
        Some(code) => match db::invitations::check(&app.pool, &code, InviteRole::Admin, now()).await? {
            Some(by) if db::invitations::redeem(&app.pool, &code, InviteRole::Admin, cx.user, now()).await? => Some(by),
            _ => None,
        },
        None => None,
    };
    let menu = |lang| async move { app.menu(cx.user, lang).await };
    let Some(by) = redeemed else {
        let text = i18n::text(cx.lang, Text::InviteInvalid);
        cx.bot.send_message(chat(cx.user), text).reply_markup(menu(cx.lang).await?).await?;
        return Ok(());
    };
    db::users::appoint_admin(&app.pool, cx.user, by, now()).await?;
    tracing::info!("admin appointed");
    let mut welcome = i18n::text(cx.lang, Text::AdminWelcome).to_owned();
    if let Some(link) = support::alerts_invite(cx.bot, app).await {
        welcome.push_str(&i18n::alerts_channel_link(cx.lang, &link));
    }
    cx.bot.send_message(chat(cx.user), welcome).reply_markup(menu(cx.lang).await?).await?;
    Ok(())
}

/// `rma:<user>`: a superadmin removes an admin.
pub async fn remove_admin(cx: &Ctx<'_>, admin: UserId) -> anyhow::Result<Option<String>> {
    let app = cx.app;
    if !app.config.is_superadmin(cx.user) {
        return Ok(Some(i18n::text(cx.lang, Text::NotAllowed).into()));
    }
    if !db::users::remove_admin(&app.pool, admin).await? {
        return Ok(Some(i18n::text(cx.lang, Text::AlreadyDecided).into()));
    }
    tracing::info!("admin removed");
    support::leave_alerts(cx.bot, app, admin).await;
    let lang = app.lang_of(admin).await;
    let menu = app.menu(admin, lang).await?;
    let _ = cx.bot.send_message(chat(admin), i18n::text(lang, Text::AdminRemovedYou)).reply_markup(menu).await;
    show_admins(cx).await?;
    Ok(None)
}
