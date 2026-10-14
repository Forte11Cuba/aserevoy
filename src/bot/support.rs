//! 💬 Support through a human Telegram account (`SUPPORT_USERNAME`): the bot
//! gives the person a link to it and a short support code. Admins type the
//! code in the bot (`/caso K7P2`) to act on that person without knowing their
//! Telegram id. Alerts go to each staff member's chat with the bot.

use teloxide::prelude::*;
use teloxide::types::InlineKeyboardMarkup;

use super::client::Ctx;
use super::{App, chat, keyboards, now};
use crate::db;
use crate::domain::ids::{UserId, short_code};
use crate::domain::reputation::Role;
use crate::i18n::{self, Text};

/// 💬 Support from the menu: the link to the support account and the code.
pub async fn open(cx: &Ctx<'_>) -> anyhow::Result<()> {
    let Some(username) = cx.app.config.support_username.as_deref() else {
        cx.bot.send_message(chat(cx.user), i18n::text(cx.lang, Text::SupportUnavailable)).await?;
        return Ok(());
    };
    let code = code_for(cx.app, cx.user).await?;
    let markup = keyboards::support_link(cx.lang, username);
    cx.bot.send_message(chat(cx.user), i18n::support(cx.lang, username, &code)).reply_markup(markup).await?;
    Ok(())
}

/// The user's support code, creating one if they have none or it expired.
async fn code_for(app: &App, user: UserId) -> anyhow::Result<String> {
    let now = now();
    if let Some(code) = db::support::code_of(&app.pool, user, now).await? {
        return Ok(code);
    }
    loop {
        let code = short_code(&mut rand::rng(), 4);
        if db::support::set_code(&app.pool, user, &code, now).await? {
            return Ok(code);
        }
    }
}

/// `/caso <code>` (staff): who it is (role, reputation, blocks) and the buttons
/// to act on them.
pub async fn lookup(cx: &Ctx<'_>, code: &str) -> anyhow::Result<()> {
    let app = cx.app;
    if !app.is_staff(cx.user).await? {
        cx.bot.send_message(chat(cx.user), i18n::text(cx.lang, Text::NotAllowed)).await?;
        return Ok(());
    }
    let code = code.trim().to_ascii_uppercase();
    let Some(user) = db::support::user_of(&app.pool, &code, now()).await? else {
        cx.bot.send_message(chat(cx.user), i18n::text(cx.lang, Text::UnknownCode)).await?;
        return Ok(());
    };
    let driver = app.driver_status(user).await?;
    let now = now();
    let client_rep = db::reputation::get(&app.pool, user, Role::Client).await?.map(|r| r.shown(now));
    let driver_rep = db::reputation::get(&app.pool, user, Role::Driver).await?.map(|r| r.shown(now));
    let blocked = db::moderation::blocked_until(&app.pool, user, Role::Client, now).await?;
    let text = i18n::case_summary(
        cx.lang,
        &code,
        driver,
        client_rep.as_ref(),
        driver_rep.as_ref(),
        blocked.map(|until| until.map(super::local_time)),
    );
    cx.bot.send_message(chat(cx.user), text).reply_markup(keyboards::moderate(cx.lang, user, driver.is_some())).await?;
    Ok(())
}

/// Something the admins must know about, with buttons to act on it: posted
/// once in the alerts channel (in Spanish), or, without one or if posting
/// fails, sent to every superadmin and admin in their chat with the bot.
pub async fn alert(
    bot: &Bot,
    app: &App,
    text: impl Fn(i18n::Lang) -> String,
    photo: Option<String>,
    markup: impl Fn(i18n::Lang) -> InlineKeyboardMarkup,
) -> anyhow::Result<()> {
    let send = |to: ChatId, lang: i18n::Lang| {
        let (text, markup) = (text(lang), markup(lang));
        let photo = photo.clone();
        async move {
            match photo {
                Some(file) => {
                    let photo = teloxide::types::InputFile::file_id(teloxide::types::FileId(file));
                    bot.send_photo(to, photo).caption(text).reply_markup(markup).await.map(|_| ())
                }
                None => bot.send_message(to, text).reply_markup(markup).await.map(|_| ()),
            }
        }
    };
    if let Some(channel) = app.config.alerts_channel_id {
        match send(ChatId(channel), i18n::Lang::Es).await {
            Ok(()) => return Ok(()),
            Err(e) => tracing::warn!(error = %e, "cannot post in the alerts channel"),
        }
    }
    for staff in app.staff().await? {
        let lang = app.lang_of(staff).await;
        if let Err(e) = send(chat(staff), lang).await {
            // Usually a staff member who never opened the bot.
            tracing::warn!(error = %e, "cannot send alert to staff");
        }
    }
    Ok(())
}

/// An alert in the channel was acted on: its buttons go and it says who did
/// what ("— @ana: Aprobado"), so the other admins see it is handled.
pub async fn mark_alert_handled(bot: &Bot, msg: &Message, who: &str, outcome: &str) {
    let note = format!("\n\n— @{who}: {outcome}");
    let edited = if msg.photo().is_some() {
        let caption = format!("{}{note}", msg.caption().unwrap_or_default());
        bot.edit_message_caption(msg.chat.id, msg.id).caption(caption).await.map(|_| ())
    } else {
        let text = format!("{}{note}", msg.text().unwrap_or_default());
        bot.edit_message_text(msg.chat.id, msg.id, text).await.map(|_| ())
    };
    if let Err(e) = edited {
        tracing::warn!(error = %e, "cannot mark the alert as handled");
    }
}

/// A one-use link to the alerts channel, for a new admin.
pub async fn alerts_invite(bot: &Bot, app: &App) -> Option<String> {
    let channel = ChatId(app.config.alerts_channel_id?);
    match bot.create_chat_invite_link(channel).member_limit(1).await {
        Ok(link) => Some(link.invite_link),
        Err(e) => {
            tracing::warn!(error = %e, "cannot create an alerts channel link");
            None
        }
    }
}

/// Takes a removed admin out of the alerts channel (banned and unbanned, so
/// they could be invited again later).
pub async fn leave_alerts(bot: &Bot, app: &App, user: UserId) {
    let Some(channel) = app.config.alerts_channel_id.map(ChatId) else {
        return;
    };
    let member = teloxide::types::UserId(user.0 as u64);
    let removed = bot.ban_chat_member(channel, member).await;
    if let Err(e) = removed {
        tracing::warn!(error = %e, "cannot remove an admin from the alerts channel");
        return;
    }
    let _ = bot.unban_chat_member(channel, member).only_if_banned(true).await;
}
