//! 🔧 Settings: the client's optional details (name, phone, photo) for the
//! driver of their trip, and the language. Everyone has them, drivers too,
//! apart from the driver profile; changes need no approval.

use teloxide::prelude::*;
use teloxide::types::{FileId, InputFile, KeyboardButton};

use super::client::Ctx;
use super::dialog::{ClientField, Dialog};
use super::{chat, keyboards, now};
use crate::db;
use crate::db::clients::ClientProfile;
use crate::domain::driver::{self, ALIAS_MAX};
use crate::domain::phone;
use crate::domain::reputation::Role;
use crate::i18n::{self, Button, Text, parse_button};

/// "🔧 Settings".
pub async fn show(cx: &Ctx<'_>) -> anyhow::Result<()> {
    cx.bot
        .send_message(chat(cx.user), i18n::text(cx.lang, Text::Settings))
        .reply_markup(keyboards::settings(cx.lang))
        .await?;
    Ok(())
}

/// `set:<what>`.
pub async fn on_callback(cx: &Ctx<'_>, arg: &str) -> anyhow::Result<()> {
    match arg {
        "me" => my_details(cx).await,
        "rep" => {
            let now = now();
            let pool = &cx.app.pool;
            let rep = db::reputation::get(pool, cx.user, Role::Client).await?;
            let tags = db::reputation::tags(pool, cx.user, Role::Client).await?;
            let cancellations = db::moderation::recent_cancellations(pool, cx.user, now).await?;
            let text = i18n::client_stats(cx.lang, rep.as_ref(), &tags, cancellations, now);
            cx.bot.send_message(chat(cx.user), text).await?;
            Ok(())
        }
        "name" => ask(cx, ClientField::Name).await,
        "phone" => ask(cx, ClientField::Phone).await,
        "photo" => ask(cx, ClientField::Photo).await,
        "del" => {
            db::clients::delete(&cx.app.pool, cx.user).await?;
            tracing::info!("client details deleted");
            cx.bot.send_message(chat(cx.user), i18n::text(cx.lang, Text::DetailsDeleted)).await?;
            my_details(cx).await
        }
        "lang" => {
            let lang = cx.lang.other();
            db::users::set_lang(&cx.app.pool, cx.user, lang).await?;
            let menu = cx.app.menu(cx.user, lang).await?;
            cx.bot.send_message(chat(cx.user), i18n::text(lang, Text::LangChanged)).reply_markup(menu).await?;
            Ok(())
        }
        _ => Ok(()),
    }
}

/// "👤 My details": what the driver of a trip will get, with a button per
/// detail.
async fn my_details(cx: &Ctx<'_>) -> anyhow::Result<()> {
    let p = db::clients::get(&cx.app.pool, &cx.app.cipher, cx.user).await?.unwrap_or_default();
    let card = i18n::client_details(cx.lang, p.name.as_deref(), p.phone.as_deref(), p.photo_file_id.is_some());
    let markup = keyboards::client_details(cx.lang, !p.is_empty());
    match &p.photo_file_id {
        Some(photo) => {
            let photo = InputFile::file_id(FileId(photo.clone()));
            cx.bot.send_photo(chat(cx.user), photo).caption(card).reply_markup(markup).await?;
        }
        None => {
            cx.bot.send_message(chat(cx.user), card).reply_markup(markup).await?;
        }
    }
    Ok(())
}

async fn ask(cx: &Ctx<'_>, field: ClientField) -> anyhow::Result<()> {
    db::dialogs::set(&cx.app.pool, cx.user, &Dialog::EditClient { field }, now()).await?;
    let (question, extra): (Text, Vec<KeyboardButton>) = match field {
        ClientField::Name => (Text::AskClientName, Vec::new()),
        ClientField::Phone => (Text::AskClientPhone, vec![keyboards::share_phone(cx.lang)]),
        ClientField::Photo => (Text::AskClientPhoto, Vec::new()),
    };
    cx.bot
        .send_message(chat(cx.user), i18n::text(cx.lang, question))
        .reply_markup(keyboards::step(cx.lang, &extra))
        .await?;
    Ok(())
}

/// The new value of a detail; Back leaves it as it was.
pub async fn on_message(cx: &Ctx<'_>, field: ClientField, msg: &Message) -> anyhow::Result<()> {
    let text = msg.text().map(str::trim);
    if text.and_then(parse_button) == Some(Button::StepBack) {
        db::dialogs::clear(&cx.app.pool, cx.user).await?;
        return back(cx, Text::Cancelled).await;
    }
    let mut p = db::clients::get(&cx.app.pool, &cx.app.cipher, cx.user).await?.unwrap_or_default();
    let ok = match field {
        ClientField::Name => text.and_then(|t| driver::clean_field(t, ALIAS_MAX)).map(|v| p.name = Some(v)),
        ClientField::Phone => contact_phone(msg).map(|v| p.phone = Some(v)),
        ClientField::Photo => medium_photo(msg).map(|v| p.photo_file_id = Some(v)),
    };
    if ok.is_none() {
        let complaint = match field {
            ClientField::Name => Text::InvalidField,
            ClientField::Phone => Text::InvalidPhone,
            ClientField::Photo => Text::NeedPhoto,
        };
        cx.bot.send_message(chat(cx.user), i18n::text(cx.lang, complaint)).await?;
        return Ok(());
    }
    db::clients::save(&cx.app.pool, &cx.app.cipher, cx.user, &p, now()).await?;
    db::dialogs::clear(&cx.app.pool, cx.user).await?;
    back(cx, Text::DetailsSaved).await
}

/// The main menu back, and the details again.
async fn back(cx: &Ctx<'_>, note: Text) -> anyhow::Result<()> {
    let menu = cx.app.menu(cx.user, cx.lang).await?;
    cx.bot.send_message(chat(cx.user), i18n::text(cx.lang, note)).reply_markup(menu).await?;
    my_details(cx).await
}

/// A phone shared with the button or typed, normalized.
pub fn contact_phone(msg: &Message) -> Option<String> {
    msg.contact().map(|c| c.phone_number.as_str()).or(msg.text().map(str::trim)).and_then(phone::normalize)
}

/// A photo of a person is only seen small: the medium size is enough.
fn medium_photo(msg: &Message) -> Option<String> {
    let sizes = msg.photo()?;
    let dims: Vec<(u32, u32)> = sizes.iter().map(|p| (p.width, p.height)).collect();
    super::driver::medium_size(&dims).map(|i| sizes[i].file.id.0.clone())
}

/// Saves a phone given while requesting a ride, keeping the other details.
pub async fn remember_phone(cx: &Ctx<'_>, phone: String) -> anyhow::Result<()> {
    let mut p: ClientProfile = db::clients::get(&cx.app.pool, &cx.app.cipher, cx.user).await?.unwrap_or_default();
    p.phone = Some(phone);
    db::clients::save(&cx.app.pool, &cx.app.cipher, cx.user, &p, now()).await?;
    Ok(())
}
