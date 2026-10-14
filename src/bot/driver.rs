//! Driver onboarding: "Apply to drive" or a one-use invitation link, the
//! profile form, approval by staff, and editing the profile later.

use teloxide::prelude::*;
use teloxide::types::{FileId, InputFile, KeyboardButton, MessageId};

use super::client::Ctx;
use super::dialog::{Dialog, DriverForm, FormStep};
use super::{App, chat, keyboards, now, support};
use crate::db;
use crate::db::invitations::InviteRole;
use crate::domain::reputation::Role;
use crate::domain::driver::{self, ALIAS_MAX, DriverStatus, FIELD_MAX, PaymentMethods, Profile};
use crate::domain::ids::{InviteCode, UserId};
use crate::domain::phone;
use crate::i18n::{self, Button, Text, parse_button};

/// "🧑‍✈️ Apply to drive".
pub async fn apply(cx: &Ctx<'_>) -> anyhow::Result<()> {
    match cx.app.driver_status(cx.user).await? {
        None => start_form(cx, DriverForm::default()).await,
        Some(status) => refuse(cx, status).await,
    }
}

/// `/start inv_<code>`.
pub async fn start_from_invite(cx: &Ctx<'_>, code: &str) -> anyhow::Result<()> {
    if let Some(status) = cx.app.driver_status(cx.user).await? {
        return refuse(cx, status).await;
    }
    let inviter = match InviteCode::parse(code) {
        Some(code) => db::invitations::check(&cx.app.pool, &code, InviteRole::Driver, now()).await?,
        None => None,
    };
    let Some(inviter) = inviter else {
        let menu = cx.app.menu(cx.user, cx.lang).await?;
        cx.bot.send_message(chat(cx.user), i18n::text(cx.lang, Text::InviteInvalid)).reply_markup(menu).await?;
        return Ok(());
    };
    let form = DriverForm { invite: Some((code.to_owned(), inviter.0)), ..DriverForm::default() };
    start_form(cx, form).await
}

async fn refuse(cx: &Ctx<'_>, status: DriverStatus) -> anyhow::Result<()> {
    let t = if status == DriverStatus::Blocked { Text::DriverBlocked } else { Text::AlreadyDriver };
    cx.bot.send_message(chat(cx.user), i18n::text(cx.lang, t)).await?;
    Ok(())
}

/// "🪪 My profile": the card as clients see it, with an Edit button.
pub async fn show_profile(cx: &Ctx<'_>) -> anyhow::Result<()> {
    let Some(d) = db::drivers::get(&cx.app.pool, &cx.app.cipher, cx.user).await? else {
        return apply(cx).await;
    };
    let photo = InputFile::file_id(FileId(d.profile.photo_file_id.clone()));
    cx.bot
        .send_photo(chat(cx.user), photo)
        .caption(i18n::profile_card(cx.lang, &d.profile, Some(d.status)))
        .reply_markup(keyboards::edit_profile(cx.lang))
        .await?;
    Ok(())
}

/// "✏️ Edit profile": the same form, starting from the current values.
pub async fn edit(cx: &Ctx<'_>) -> anyhow::Result<()> {
    let Some(d) = db::drivers::get(&cx.app.pool, &cx.app.cipher, cx.user).await? else {
        return Ok(());
    };
    if d.status == DriverStatus::Blocked {
        return refuse(cx, d.status).await;
    }
    let p = d.profile;
    let form = DriverForm {
        step: None,
        invite: None,
        alias: Some(p.alias),
        phone: Some(p.phone),
        plate: Some(p.plate),
        bike_model: Some(p.bike_model),
        bike_color: Some(p.bike_color),
        photo_file_id: Some(p.photo_file_id),
        bike_photo_file_id: p.bike_photo_file_id,
        payment_methods: Some(p.payment_methods),
    };
    start_form(cx, form).await
}

async fn start_form(cx: &Ctx<'_>, form: DriverForm) -> anyhow::Result<()> {
    ask(cx, form, FormStep::Alias).await
}

/// Saves the form at `step` and asks its question.
async fn ask(cx: &Ctx<'_>, mut form: DriverForm, step: FormStep) -> anyhow::Result<()> {
    let lang = cx.lang;
    // Skip keeps the current value when editing; the bike photo is optional.
    let can_skip = match step {
        FormStep::Alias => form.alias.is_some(),
        FormStep::Phone => form.phone.is_some(),
        FormStep::Plate => form.plate.is_some(),
        FormStep::BikeModel => form.bike_model.is_some(),
        FormStep::BikeColor => form.bike_color.is_some(),
        FormStep::Photo => form.photo_file_id.is_some(),
        FormStep::BikePhoto => true,
        FormStep::Methods => false,
    };
    let mut extra: Vec<KeyboardButton> = Vec::new();
    if step == FormStep::Phone {
        extra.push(keyboards::share_phone(lang));
    }
    if can_skip {
        extra.push(keyboards::skip(lang));
    }
    let question = match step {
        FormStep::Alias => Text::AskAlias,
        FormStep::Phone => Text::AskDriverPhone,
        FormStep::Plate => Text::AskPlate,
        FormStep::BikeModel => Text::AskBikeModel,
        FormStep::BikeColor => Text::AskBikeColor,
        FormStep::Photo => Text::AskPhoto,
        FormStep::BikePhoto => Text::AskBikePhoto,
        FormStep::Methods => Text::AskPaymentMethods,
    };
    form.step = Some(step);
    db::dialogs::set(&cx.app.pool, cx.user, &Dialog::DriverForm(form), now()).await?;
    let send = cx.bot.send_message(chat(cx.user), i18n::text(lang, question));
    if step == FormStep::Methods {
        send.reply_markup(keyboards::payment_methods(lang)).await?;
    } else {
        send.reply_markup(keyboards::step(lang, &extra)).await?;
    }
    Ok(())
}

fn next(step: FormStep) -> FormStep {
    match step {
        FormStep::Alias => FormStep::Phone,
        FormStep::Phone => FormStep::Plate,
        FormStep::Plate => FormStep::BikeModel,
        FormStep::BikeModel => FormStep::BikeColor,
        FormStep::BikeColor => FormStep::Photo,
        FormStep::Photo => FormStep::BikePhoto,
        FormStep::BikePhoto | FormStep::Methods => FormStep::Methods,
    }
}

/// A message while the form is open.
pub async fn on_message(cx: &Ctx<'_>, mut form: DriverForm, msg: &Message) -> anyhow::Result<()> {
    let Some(step) = form.step else {
        return start_form(cx, form).await;
    };
    let text = msg.text().map(str::trim);
    let skipped = text.and_then(parse_button) == Some(Button::Skip);
    let photo = msg.photo().and_then(|sizes| sizes.last()).map(|p| p.file.id.0.clone());

    let field = |max| text.and_then(|t| driver::clean_field(t, max));
    let accepted = match step {
        FormStep::Alias if skipped => form.alias.is_some(),
        FormStep::Alias => set(&mut form.alias, field(ALIAS_MAX)),
        FormStep::Phone if skipped => form.phone.is_some(),
        FormStep::Phone => {
            let typed = msg.contact().map(|c| c.phone_number.as_str()).or(text);
            set(&mut form.phone, typed.and_then(phone::normalize))
        }
        FormStep::Plate if skipped => form.plate.is_some(),
        FormStep::Plate => set(&mut form.plate, field(FIELD_MAX)),
        FormStep::BikeModel if skipped => form.bike_model.is_some(),
        FormStep::BikeModel => set(&mut form.bike_model, field(FIELD_MAX)),
        FormStep::BikeColor if skipped => form.bike_color.is_some(),
        FormStep::BikeColor => set(&mut form.bike_color, field(FIELD_MAX)),
        FormStep::Photo if skipped => form.photo_file_id.is_some(),
        FormStep::Photo => set(&mut form.photo_file_id, photo),
        FormStep::BikePhoto if skipped => true,
        FormStep::BikePhoto => set(&mut form.bike_photo_file_id, photo),
        // Waits for an inline button.
        FormStep::Methods => false,
    };
    if accepted {
        return ask(cx, form, next(step)).await;
    }
    let complaint = match step {
        FormStep::Phone => Text::InvalidPhone,
        FormStep::Photo | FormStep::BikePhoto => Text::NeedPhoto,
        FormStep::Methods => Text::AskPaymentMethods,
        _ => Text::InvalidField,
    };
    let send = cx.bot.send_message(chat(cx.user), i18n::text(cx.lang, complaint));
    if step == FormStep::Methods {
        send.reply_markup(keyboards::payment_methods(cx.lang)).await?;
    } else {
        send.await?;
    }
    Ok(())
}

fn set(slot: &mut Option<String>, value: Option<String>) -> bool {
    match value {
        Some(v) => {
            *slot = Some(v);
            true
        }
        None => false,
    }
}

/// `pm:<cash|transfer|both>`: the last step; saves the profile.
pub async fn on_payment_methods(
    cx: &Ctx<'_>,
    mut form: DriverForm,
    arg: &str,
    pressed: Option<MessageId>,
) -> anyhow::Result<()> {
    if form.step != Some(FormStep::Methods) {
        return Ok(());
    }
    form.payment_methods = match arg {
        "cash" => Some(PaymentMethods::CASH),
        "transfer" => Some(PaymentMethods::TRANSFER),
        "both" => Some(PaymentMethods::BOTH),
        _ => return Ok(()),
    };
    if let Some(msg) = pressed {
        let _ = cx.bot.edit_message_reply_markup(chat(cx.user), msg).await;
    }
    finish(cx, form).await
}

async fn finish(cx: &Ctx<'_>, form: DriverForm) -> anyhow::Result<()> {
    let app = cx.app;
    let inviter = form.inviter();
    let invite = form.invite.as_ref().and_then(|(code, _)| InviteCode::parse(code));
    let (Some(alias), Some(phone), Some(plate), Some(bike_model), Some(bike_color), Some(photo), Some(methods)) = (
        form.alias,
        form.phone,
        form.plate,
        form.bike_model,
        form.bike_color,
        form.photo_file_id,
        form.payment_methods,
    ) else {
        // Cannot happen through the steps; start over rather than save a hole.
        return start_form(cx, DriverForm::default()).await;
    };
    let profile = Profile {
        alias,
        phone,
        plate,
        bike_model,
        bike_color,
        photo_file_id: photo,
        bike_photo_file_id: form.bike_photo_file_id,
        payment_methods: methods,
    };

    let is_new = app.driver_status(cx.user).await?.is_none();
    // The invitation is consumed only now, so an abandoned form can be resumed
    // with the same link. If it was used or expired meanwhile, the profile is
    // still sent as a regular application.
    let invited = match &invite {
        Some(code) if is_new => db::invitations::redeem(&app.pool, code, InviteRole::Driver, cx.user, now()).await?,
        _ => false,
    };
    let added_by = if invited { inviter } else { None };
    db::drivers::save_profile(&app.pool, &app.cipher, cx.user, &profile, added_by, now()).await?;
    db::dialogs::clear(&app.pool, cx.user).await?;

    let menu = app.menu(cx.user, cx.lang).await?;
    if is_new {
        tracing::info!("driver application received");
        cx.bot.send_message(chat(cx.user), i18n::text(cx.lang, Text::ApplicationSent)).reply_markup(menu).await?;
        notify_staff(cx.bot, app, cx.user, &profile, invited).await?;
    } else {
        cx.bot.send_message(chat(cx.user), i18n::text(cx.lang, Text::ProfileUpdated)).reply_markup(menu).await?;
    }
    Ok(())
}

async fn notify_staff(bot: &Bot, app: &App, driver: UserId, profile: &Profile, invited: bool) -> anyhow::Result<()> {
    let text = |lang| i18n::application(lang, profile, invited);
    let markup = |lang| keyboards::review_application(lang, driver);
    support::alert(bot, app, text, Some(profile.photo_file_id.clone()), markup).await
}

/// `dok:<id>` / `dno:<id>`: staff approve or reject an application.
pub async fn review(
    cx: &Ctx<'_>,
    approve: bool,
    arg: &str,
    pressed: Option<MessageId>,
) -> anyhow::Result<Option<String>> {
    let app = cx.app;
    if !app.is_staff(cx.user).await? {
        return Ok(Some(i18n::text(cx.lang, Text::NotAllowed).into()));
    }
    let Ok(driver) = arg.parse().map(UserId) else {
        return Ok(None);
    };
    let decided = if approve {
        db::drivers::approve(&app.pool, driver, cx.user).await?
    } else {
        db::drivers::reject(&app.pool, driver).await?
    };
    if let Some(msg) = pressed {
        let _ = cx.bot.edit_message_reply_markup(chat(cx.user), msg).await;
    }
    if !decided {
        return Ok(Some(i18n::text(cx.lang, Text::AlreadyDecided).into()));
    }

    let driver_lang = app.lang_of(driver).await;
    let to_driver = if approve {
        tracing::info!("driver approved");
        db::reputation::touch(&app.pool, driver, Role::Driver, now()).await?;
        i18n::approved(driver_lang, &app.channel_link)
    } else {
        tracing::info!("driver rejected");
        i18n::text(driver_lang, Text::Rejected).into()
    };
    let menu = app.menu(driver, driver_lang).await?;
    if let Err(e) = cx.bot.send_message(chat(driver), to_driver).reply_markup(menu).await {
        tracing::warn!(error = %e, "cannot tell the driver about the decision");
    }
    let done = if approve { Text::StaffApproved } else { Text::StaffRejected };
    Ok(Some(i18n::text(cx.lang, done).into()))
}

/// "➕ Invite a driver": a one-use link valid for 48 h.
pub async fn invite(cx: &Ctx<'_>) -> anyhow::Result<()> {
    if !cx.app.is_staff(cx.user).await? {
        cx.bot.send_message(chat(cx.user), i18n::text(cx.lang, Text::NotAllowed)).await?;
        return Ok(());
    }
    let code = InviteCode::generate(&mut rand::rng());
    db::invitations::create(&cx.app.pool, &code, InviteRole::Driver, cx.user, now()).await?;
    let link = format!("https://t.me/{}?start=inv_{}", cx.app.bot_username, code.as_str());
    cx.bot.send_message(chat(cx.user), i18n::invite_created(cx.lang, &link)).await?;
    Ok(())
}
