//! 🧾 My trips: each side's completed trips of the last 45 days, the most
//! recent first, by pages; and deleting them.

use teloxide::prelude::*;
use teloxide::types::MessageId;

use super::client::Ctx;
use super::{chat, keyboards, local_time, panel};
use crate::db;
use crate::domain::reputation::Role;
use crate::i18n::{self, Text};

/// Trips per page.
pub const PER_PAGE: i64 = 10;

/// `hist:<c|d>:<page|del>`, or from the menus with `pressed` = None.
pub async fn on_callback(cx: &Ctx<'_>, arg: &str, pressed: Option<MessageId>) -> anyhow::Result<()> {
    let (role, what) = arg.split_once(':').unwrap_or((arg, "0"));
    let role = if role == "d" { Role::Driver } else { Role::Client };
    if what == "del" {
        db::history::delete_for(&cx.app.pool, cx.user, role).await?;
        tracing::info!("trip history deleted");
        cx.bot.send_message(chat(cx.user), i18n::text(cx.lang, Text::HistoryDeleted)).await?;
        return Ok(());
    }
    show(cx, role, what.parse().unwrap_or(0), pressed).await
}

/// One page of the user's trips in `role`; paging edits the same message.
pub async fn show(cx: &Ctx<'_>, role: Role, page: i64, pressed: Option<MessageId>) -> anyhow::Result<()> {
    if role == Role::Driver && cx.app.driver_status(cx.user).await?.is_none() {
        return Ok(());
    }
    let (entries, total) =
        db::history::page(&cx.app.pool, &cx.app.cipher, cx.user, role, page * PER_PAGE, PER_PAGE).await?;
    let pages = ((total + PER_PAGE - 1) / PER_PAGE).max(1);
    let lines: Vec<i18n::HistoryLine> = entries
        .iter()
        .map(|e| i18n::HistoryLine {
            number: e.number,
            request: &e.request_id,
            when: local_time(e.completed_at),
            origin: &e.origin,
            destination: &e.destination,
            price: e.price,
            pay: e.payment,
            counterpart: &e.counterpart,
            stars: e.stars,
        })
        .collect();
    let text = i18n::history(cx.lang, role, &lines, total);
    let markup = keyboards::history(cx.lang, role, page as usize, pages as usize, total > 0);
    panel::edit_or_send(cx, pressed, text, markup).await
}
