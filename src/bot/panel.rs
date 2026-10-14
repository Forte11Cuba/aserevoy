//! ⚙️ Administration: a bottom keyboard with the sections (drivers and
//! invitations for all staff; admins, fares and metrics for superadmins).
//! Each section answers with a message whose buttons act on what it shows
//! (a page of drivers, a driver, the fares); paging edits that message, and a
//! driver's card, being a photo, comes as a new one. Roles are checked on
//! every press.

use teloxide::prelude::*;
use teloxide::types::{FileId, InlineKeyboardMarkup, InputFile, MessageId};

use super::client::Ctx;
use super::{admin, chat, keyboards, now};
use crate::db;
use crate::domain::driver::DriverStatus;
use crate::domain::fare::FareKey;
use crate::domain::ids::UserId;
use crate::domain::reputation::Role;
use crate::i18n::{self, Button, Text};

/// Drivers per page of the list.
pub const DRIVERS_PER_PAGE: i64 = 10;

/// Which drivers the list shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    All,
    Active,
    Pending,
    Blocked,
}

impl Filter {
    pub const ALL: [Filter; 4] = [Filter::All, Filter::Active, Filter::Pending, Filter::Blocked];

    pub fn code(self) -> &'static str {
        match self {
            Filter::All => "all",
            Filter::Active => "act",
            Filter::Pending => "pen",
            Filter::Blocked => "blk",
        }
    }

    fn parse(code: &str) -> Filter {
        Filter::ALL.into_iter().find(|f| f.code() == code).unwrap_or(Filter::All)
    }

    fn status(self) -> Option<DriverStatus> {
        match self {
            Filter::All => None,
            Filter::Active => Some(DriverStatus::Active),
            Filter::Pending => Some(DriverStatus::Pending),
            Filter::Blocked => Some(DriverStatus::Blocked),
        }
    }
}

/// "⚙️ Administration" from the menu: the administration keyboard below.
pub async fn show(cx: &Ctx<'_>) -> anyhow::Result<()> {
    if !cx.app.is_staff(cx.user).await? {
        cx.bot.send_message(chat(cx.user), i18n::text(cx.lang, Text::NotAllowed)).await?;
        return Ok(());
    }
    let superadmin = cx.app.config.is_superadmin(cx.user);
    cx.bot
        .send_message(chat(cx.user), i18n::text(cx.lang, Text::PanelHome))
        .reply_markup(keyboards::admin_menu(cx.lang, superadmin))
        .await?;
    Ok(())
}

/// A section from the administration keyboard.
pub async fn section(cx: &Ctx<'_>, which: Button) -> anyhow::Result<()> {
    let allowed = match which {
        Button::PanelDrivers => cx.app.is_staff(cx.user).await?,
        _ => cx.app.config.is_superadmin(cx.user),
    };
    if !allowed {
        cx.bot.send_message(chat(cx.user), i18n::text(cx.lang, Text::NotAllowed)).await?;
        return Ok(());
    }
    match which {
        Button::PanelDrivers => drivers(cx, None, Filter::All, 0).await,
        Button::PanelAdmins => admin::show_admins(cx).await,
        Button::PanelFares => fares(cx, None).await,
        Button::PanelMetrics => {
            let days = db::metrics::last(&cx.app.pool, 7).await?;
            cx.bot.send_message(chat(cx.user), i18n::metrics(cx.lang, &days)).await?;
            Ok(())
        }
        _ => Ok(()),
    }
}

/// `ap:<what>[:…]`. Returns the alert to show, if any.
pub async fn on_callback(cx: &Ctx<'_>, arg: &str, pressed: Option<MessageId>) -> anyhow::Result<Option<String>> {
    if !cx.app.is_staff(cx.user).await? {
        return Ok(Some(i18n::text(cx.lang, Text::NotAllowed).into()));
    }
    let superadmin = cx.app.config.is_superadmin(cx.user);
    let parts: Vec<&str> = arg.split(':').collect();
    match parts[..] {
        ["drv", filter, page] => {
            let page = page.parse().unwrap_or(0);
            drivers(cx, pressed, Filter::parse(filter), page).await?;
        }
        ["d", id] => {
            if let Ok(id) = id.parse() {
                driver_card(cx, UserId(id)).await?;
            }
        }
        // Superadmins only from here.
        _ if !superadmin => return Ok(Some(i18n::text(cx.lang, Text::NotAllowed).into())),
        ["fare"] => fares(cx, pressed).await?,
        ["fa", key, delta] => {
            if let (Some(key), Ok(delta)) = (FareKey::parse(key), delta.parse::<i64>()) {
                let current = db::fares::load(&cx.app.pool).await?;
                let value = match key {
                    FareKey::Base => current.base + delta,
                    FareKey::PerKm => current.per_km + delta,
                    FareKey::MinPercent => i64::from(current.min_percent) + delta,
                };
                if !key.valid(value) {
                    return Ok(Some(i18n::text(cx.lang, Text::FareOutOfRange).into()));
                }
                db::fares::set(&cx.app.pool, key, value).await?;
                tracing::info!(?key, value, "fare changed");
                fares(cx, pressed).await?;
            }
        }
        _ => {}
    }
    Ok(None)
}

/// Edits the section's message in place (paging, a fare change), or sends a
/// new one (from the keyboard, or when the pressed message was a photo).
async fn edit_or_send(
    cx: &Ctx<'_>,
    pressed: Option<MessageId>,
    text: String,
    markup: InlineKeyboardMarkup,
) -> anyhow::Result<()> {
    if let Some(msg) = pressed
        && cx.bot.edit_message_text(chat(cx.user), msg, text.clone()).reply_markup(markup.clone()).await.is_ok()
    {
        return Ok(());
    }
    cx.bot.send_message(chat(cx.user), text).reply_markup(markup).await?;
    Ok(())
}

async fn drivers(cx: &Ctx<'_>, pressed: Option<MessageId>, filter: Filter, page: i64) -> anyhow::Result<()> {
    let (listed, total) =
        db::drivers::list(&cx.app.pool, filter.status(), page * DRIVERS_PER_PAGE, DRIVERS_PER_PAGE).await?;
    let pages = ((total + DRIVERS_PER_PAGE - 1) / DRIVERS_PER_PAGE).max(1);
    let text = i18n::drivers_list(cx.lang, filter, total);
    let markup = keyboards::drivers_list(cx.lang, &listed, filter, page, pages);
    edit_or_send(cx, pressed, text, markup).await
}

/// A driver's card for staff: photo, profile (with phone), status and
/// reputation, with the actions that fit their status.
async fn driver_card(cx: &Ctx<'_>, id: UserId) -> anyhow::Result<()> {
    let Some(d) = db::drivers::get(&cx.app.pool, &cx.app.cipher, id).await? else {
        cx.bot.send_message(chat(cx.user), i18n::text(cx.lang, Text::AlreadyDecided)).await?;
        return Ok(());
    };
    let shown = db::reputation::get(&cx.app.pool, id, Role::Driver).await?.map(|r| r.shown(now()));
    let username = cx.bot.get_chat(chat(id)).await.ok().and_then(|c| c.username().map(str::to_owned));
    let caption = i18n::driver_admin_card(cx.lang, &d.profile, d.status, shown.as_ref(), username.as_deref());
    let photo = InputFile::file_id(FileId(d.profile.photo_file_id.clone()));
    cx.bot
        .send_photo(chat(cx.user), photo)
        .caption(caption)
        .reply_markup(keyboards::driver_admin(cx.lang, id, d.status))
        .await?;
    Ok(())
}

async fn fares(cx: &Ctx<'_>, pressed: Option<MessageId>) -> anyhow::Result<()> {
    let fares = db::fares::load(&cx.app.pool).await?;
    edit_or_send(cx, pressed, i18n::fares_panel(cx.lang, &fares), keyboards::fares_panel(cx.lang)).await
}
