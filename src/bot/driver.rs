//! Driver onboarding: "Apply to drive" or a one-use invitation link, the
//! profile form, approval by staff, and editing the profile later.

use teloxide::prelude::*;
use teloxide::types::{FileId, InlineKeyboardMarkup, InputFile, KeyboardButton, MessageId};

use super::client::Ctx;
use super::dialog::{Dialog, DriverForm, FormStep};
use super::{App, chat, keyboards, now, support};
use crate::db;
use crate::db::invitations::InviteRole;
use crate::domain::reputation::Role;
use crate::domain::driver::{self, ALIAS_MAX, DriverStatus, FIELD_MAX, Profile};
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

/// "🤝 Work with us": what each role is, with a button to apply.
pub async fn work_with_us(cx: &Ctx<'_>) -> anyhow::Result<()> {
    if cx.app.driver_status(cx.user).await?.is_some() {
        return menu(cx).await;
    }
    cx.bot
        .send_message(chat(cx.user), i18n::text(cx.lang, Text::WorkWithUs))
        .reply_markup(keyboards::work_with_us(cx.lang))
        .await?;
    Ok(())
}

/// "🏢 Apply as a manager": not yet (version 2). The admins hear who is
/// interested, to write to them when it is.
pub async fn manager_interest(cx: &Ctx<'_>, username: Option<&str>) -> anyhow::Result<()> {
    tracing::info!("manager interest");
    let text = |lang| i18n::manager_interest(lang, username);
    support::alert(cx.bot, cx.app, text, None, |_| InlineKeyboardMarkup::default()).await?;
    cx.bot.send_message(chat(cx.user), i18n::text(cx.lang, Text::ManagerSoon)).await?;
    Ok(())
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

/// "🏍 Driver": the driver's own keyboard (stats, profile, back).
pub async fn menu(cx: &Ctx<'_>) -> anyhow::Result<()> {
    if cx.app.driver_status(cx.user).await?.is_none() {
        return apply(cx).await;
    }
    cx.bot
        .send_message(chat(cx.user), i18n::text(cx.lang, Text::DriverMenu))
        .reply_markup(keyboards::driver_menu(cx.lang))
        .await?;
    Ok(())
}

/// "📊 My stats": the aggregate reputation as a driver (no trip history).
pub async fn stats(cx: &Ctx<'_>) -> anyhow::Result<()> {
    if cx.app.driver_status(cx.user).await?.is_none() {
        return apply(cx).await;
    }
    let reputation = db::reputation::get(&cx.app.pool, cx.user, Role::Driver).await?;
    let tags = db::reputation::tags(&cx.app.pool, cx.user, Role::Driver).await?;
    cx.bot.send_message(chat(cx.user), i18n::driver_stats(cx.lang, reputation.as_ref(), &tags, now())).await?;
    Ok(())
}

/// "🪪 My profile": the card, with a button per field to change it.
pub async fn show_profile(cx: &Ctx<'_>) -> anyhow::Result<()> {
    let Some(d) = db::drivers::get(&cx.app.pool, &cx.app.cipher, cx.user).await? else {
        return apply(cx).await;
    };
    let photo = InputFile::file_id(FileId(d.profile.photo_file_id.clone()));
    let mut caption = i18n::profile_card(cx.lang, &d.profile, Some(d.status));
    match (&d.pending_full_name, &d.profile.full_name) {
        (Some(pending), _) => caption.push_str(&i18n::name_pending(cx.lang, pending)),
        (None, None) => caption.push_str(&format!("\n\n{}", i18n::text(cx.lang, Text::FullNameMissing))),
        (None, Some(_)) => {}
    }
    caption.push_str(&format!("\n\n{}", i18n::text(cx.lang, Text::EditWhich)));
    cx.bot.send_photo(chat(cx.user), photo).caption(caption).reply_markup(keyboards::profile_fields(cx.lang)).await?;
    Ok(())
}

/// `df:<field>`: asks for the new value of one field.
pub async fn edit_field(cx: &Ctx<'_>, code: &str) -> anyhow::Result<Option<String>> {
    let Some(field) = FormStep::parse(code) else {
        return Ok(None);
    };
    match cx.app.driver_status(cx.user).await? {
        None => return Ok(Some(i18n::text(cx.lang, Text::NotAllowed).into())),
        Some(DriverStatus::Blocked) => return Ok(Some(i18n::text(cx.lang, Text::DriverBlocked).into())),
        Some(_) => {}
    }
    db::dialogs::set(&cx.app.pool, cx.user, &Dialog::EditField { field }, now()).await?;
    let extra: Vec<KeyboardButton> = if field == FormStep::Phone { vec![keyboards::share_phone(cx.lang)] } else { Vec::new() };
    cx.bot
        .send_message(chat(cx.user), i18n::text(cx.lang, question(field)))
        .reply_markup(keyboards::step(cx.lang, &extra))
        .await?;
    Ok(None)
}

/// The new value of the field being edited. A full name waits for an
/// admin; a new plate or photo is saved and the admins are told.
pub async fn on_edit_message(cx: &Ctx<'_>, field: FormStep, msg: &Message) -> anyhow::Result<()> {
    let app = cx.app;
    if msg.text().map(str::trim).and_then(parse_button) == Some(Button::StepBack) {
        db::dialogs::clear(&app.pool, cx.user).await?;
        return back_to_profile(cx, Text::Cancelled).await;
    }
    let Some(value) = read_value(field, msg) else {
        cx.bot.send_message(chat(cx.user), i18n::text(cx.lang, complaint(field))).await?;
        return Ok(());
    };
    let Some(d) = db::drivers::get(&app.pool, &app.cipher, cx.user).await? else {
        db::dialogs::clear(&app.pool, cx.user).await?;
        return Ok(());
    };
    db::dialogs::clear(&app.pool, cx.user).await?;
    if field == FormStep::FullName {
        db::drivers::set_pending_name(&app.pool, &app.cipher, cx.user, &value).await?;
        tracing::info!("driver name change sent for review");
        let (alias, old) = (d.profile.alias.clone(), d.profile.full_name.clone());
        let text = |lang| i18n::name_change(lang, &alias, old.as_deref(), &value);
        support::alert(cx.bot, app, text, None, |lang| keyboards::review_name(lang, cx.user)).await?;
        return back_to_profile(cx, Text::NameSentForReview).await;
    }
    let before = d.profile.clone();
    let mut profile = d.profile;
    match field {
        FormStep::Alias => profile.alias = value,
        FormStep::Phone => profile.phone = value,
        FormStep::Plate => profile.plate = value,
        FormStep::BikeModel => profile.bike_model = value,
        FormStep::BikeColor => profile.bike_color = value,
        FormStep::Photo => profile.photo_file_id = value,
        FormStep::BikePhoto => profile.bike_photo_file_id = Some(value),
        FormStep::FullName => {}
    }
    db::drivers::save_profile(&app.pool, &app.cipher, cx.user, &profile, None, now()).await?;
    // What staff verified in person: they hear about a new plate or photo.
    match field {
        FormStep::Plate => {
            let text = |lang| i18n::plate_changed(lang, &profile.alias, &before.plate, &profile.plate);
            support::alert(cx.bot, app, text, None, |lang| keyboards::driver_admin(lang, cx.user, d.status)).await?;
        }
        FormStep::Photo => {
            let text = |lang| i18n::photo_changed(lang, &profile.alias);
            let photo = Some(profile.photo_file_id.clone());
            support::alert(cx.bot, app, text, photo, |lang| keyboards::driver_admin(lang, cx.user, d.status)).await?;
        }
        _ => {}
    }
    back_to_profile(cx, Text::ProfileUpdated).await
}

/// A short note, the driver keyboard back, and the profile again.
async fn back_to_profile(cx: &Ctx<'_>, note: Text) -> anyhow::Result<()> {
    cx.bot
        .send_message(chat(cx.user), i18n::text(cx.lang, note))
        .reply_markup(keyboards::driver_menu(cx.lang))
        .await?;
    show_profile(cx).await
}

/// `dna:<id>` / `dnr:<id>`: staff approve or reject a new full name.
pub async fn review_name(
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
    if let Some(msg) = pressed {
        let _ = cx.bot.edit_message_reply_markup(chat(cx.user), msg).await;
    }
    if !db::drivers::decide_pending_name(&app.pool, driver, approve).await? {
        return Ok(Some(i18n::text(cx.lang, Text::AlreadyDecided).into()));
    }
    tracing::info!(approve, "driver name change decided");
    let lang = app.lang_of(driver).await;
    let to_driver = if approve { Text::NameApproved } else { Text::NameRejected };
    if let Err(e) = cx.bot.send_message(chat(driver), i18n::text(lang, to_driver)).await {
        tracing::warn!(error = %e, "cannot tell the driver about the name");
    }
    let done = if approve { Text::StaffApproved } else { Text::StaffRejected };
    Ok(Some(i18n::text(cx.lang, done).into()))
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
        FormStep::FullName => form.full_name.is_some(),
        FormStep::Phone => form.phone.is_some(),
        FormStep::Plate => form.plate.is_some(),
        FormStep::BikeModel => form.bike_model.is_some(),
        FormStep::BikeColor => form.bike_color.is_some(),
        FormStep::Photo => form.photo_file_id.is_some(),
        FormStep::BikePhoto => true,
    };
    let mut extra: Vec<KeyboardButton> = Vec::new();
    if step == FormStep::Phone {
        extra.push(keyboards::share_phone(lang));
    }
    if can_skip {
        extra.push(keyboards::skip(lang));
    }
    form.step = Some(step);
    db::dialogs::set(&cx.app.pool, cx.user, &Dialog::DriverForm(form), now()).await?;
    cx.bot
        .send_message(chat(cx.user), i18n::text(lang, question(step)))
        .reply_markup(keyboards::step(lang, &extra))
        .await?;
    Ok(())
}

fn question(step: FormStep) -> Text {
    match step {
        FormStep::Alias => Text::AskAlias,
        FormStep::FullName => Text::AskFullName,
        FormStep::Phone => Text::AskDriverPhone,
        FormStep::Plate => Text::AskPlate,
        FormStep::BikeModel => Text::AskBikeModel,
        FormStep::BikeColor => Text::AskBikeColor,
        FormStep::Photo => Text::AskPhoto,
        FormStep::BikePhoto => Text::AskBikePhoto,
    }
}

fn complaint(step: FormStep) -> Text {
    match step {
        FormStep::Phone => Text::InvalidPhone,
        FormStep::FullName => Text::InvalidFullName,
        FormStep::Photo | FormStep::BikePhoto => Text::NeedPhoto,
        _ => Text::InvalidField,
    }
}

/// The answer to `step` in the message, checked: a phone, a photo (medium
/// size) or a short text.
fn read_value(step: FormStep, msg: &Message) -> Option<String> {
    let text = msg.text().map(str::trim);
    let field = |max| text.and_then(|t| driver::clean_field(t, max));
    match step {
        FormStep::Alias => field(ALIAS_MAX),
        FormStep::FullName => text.and_then(driver::clean_full_name),
        FormStep::Phone => msg.contact().map(|c| c.phone_number.as_str()).or(text).and_then(phone::normalize),
        FormStep::Plate | FormStep::BikeModel | FormStep::BikeColor => field(FIELD_MAX),
        FormStep::Photo | FormStep::BikePhoto => msg.photo().and_then(|sizes| {
            let dims: Vec<(u32, u32)> = sizes.iter().map(|p| (p.width, p.height)).collect();
            medium_size(&dims).map(|i| sizes[i].file.id.0.clone())
        }),
    }
}

/// The next question; `None` after the last one (the bike photo). Payment
/// methods are not asked: the channel shows how each client pays and the
/// driver takes the offers that suit them.
fn next(step: FormStep) -> Option<FormStep> {
    match step {
        FormStep::Alias => Some(FormStep::FullName),
        FormStep::FullName => Some(FormStep::Phone),
        FormStep::Phone => Some(FormStep::Plate),
        FormStep::Plate => Some(FormStep::BikeModel),
        FormStep::BikeModel => Some(FormStep::BikeColor),
        FormStep::BikeColor => Some(FormStep::Photo),
        FormStep::Photo => Some(FormStep::BikePhoto),
        FormStep::BikePhoto => None,
    }
}

/// Photos are stored as Telegram's `file_id` and sent by it: no copy on our
/// server. Telegram keeps several sizes of each; the largest side up to this
/// is enough on a phone and much lighter to download (data is expensive).
const PHOTO_MAX_SIDE: u32 = 640;

/// Of Telegram's sizes of a photo, the biggest one up to `PHOTO_MAX_SIDE`,
/// or the smallest if all are bigger.
pub(super) fn medium_size(dims: &[(u32, u32)]) -> Option<usize> {
    let side = |i: usize| dims[i].0.max(dims[i].1);
    let fitting = (0..dims.len()).filter(|&i| side(i) <= PHOTO_MAX_SIDE).max_by_key(|&i| side(i));
    fitting.or_else(|| (0..dims.len()).min_by_key(|&i| side(i)))
}

fn previous(step: FormStep) -> Option<FormStep> {
    match step {
        FormStep::Alias => None,
        FormStep::FullName => Some(FormStep::Alias),
        FormStep::Phone => Some(FormStep::FullName),
        FormStep::Plate => Some(FormStep::Phone),
        FormStep::BikeModel => Some(FormStep::Plate),
        FormStep::BikeColor => Some(FormStep::BikeModel),
        FormStep::Photo => Some(FormStep::BikeColor),
        FormStep::BikePhoto => Some(FormStep::Photo),
    }
}

/// A message while the form is open.
pub async fn on_message(cx: &Ctx<'_>, mut form: DriverForm, msg: &Message) -> anyhow::Result<()> {
    let Some(step) = form.step else {
        return start_form(cx, form).await;
    };
    let text = msg.text().map(str::trim);
    if text.and_then(parse_button) == Some(Button::StepBack) {
        // Back to the previous question, keeping the answers; from the
        // first one, out of the form.
        return match previous(step) {
            Some(prev) => ask(cx, form, prev).await,
            None => {
                db::dialogs::clear(&cx.app.pool, cx.user).await?;
                let menu = cx.app.menu(cx.user, cx.lang).await?;
                cx.bot.send_message(chat(cx.user), i18n::text(cx.lang, Text::Cancelled)).reply_markup(menu).await?;
                Ok(())
            }
        };
    }
    let skipped = text.and_then(parse_button) == Some(Button::Skip);
    let value = read_value(step, msg);
    let accepted = match step {
        FormStep::Alias if skipped => form.alias.is_some(),
        FormStep::Alias => set(&mut form.alias, value),
        FormStep::FullName if skipped => form.full_name.is_some(),
        FormStep::FullName => set(&mut form.full_name, value),
        FormStep::Phone if skipped => form.phone.is_some(),
        FormStep::Phone => set(&mut form.phone, value),
        FormStep::Plate if skipped => form.plate.is_some(),
        FormStep::Plate => set(&mut form.plate, value),
        FormStep::BikeModel if skipped => form.bike_model.is_some(),
        FormStep::BikeModel => set(&mut form.bike_model, value),
        FormStep::BikeColor if skipped => form.bike_color.is_some(),
        FormStep::BikeColor => set(&mut form.bike_color, value),
        FormStep::Photo if skipped => form.photo_file_id.is_some(),
        FormStep::Photo => set(&mut form.photo_file_id, value),
        FormStep::BikePhoto if skipped => true,
        FormStep::BikePhoto => set(&mut form.bike_photo_file_id, value),
    };
    if accepted {
        return match next(step) {
            Some(next) => ask(cx, form, next).await,
            None => finish(cx, form).await,
        };
    }
    cx.bot.send_message(chat(cx.user), i18n::text(cx.lang, complaint(step))).await?;
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

async fn finish(cx: &Ctx<'_>, form: DriverForm) -> anyhow::Result<()> {
    let app = cx.app;
    let inviter = form.inviter();
    let invite = form.invite.as_ref().and_then(|(code, _)| InviteCode::parse(code));
    let (Some(alias), Some(full_name), Some(phone), Some(plate), Some(bike_model), Some(bike_color), Some(photo)) = (
        form.alias,
        form.full_name,
        form.phone,
        form.plate,
        form.bike_model,
        form.bike_color,
        form.photo_file_id,
    ) else {
        // Cannot happen through the steps; start over rather than save a hole.
        return start_form(cx, DriverForm::default()).await;
    };
    let profile = Profile {
        alias,
        full_name: Some(full_name),
        phone,
        plate,
        bike_model,
        bike_color,
        photo_file_id: photo,
        bike_photo_file_id: form.bike_photo_file_id,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn photos_use_a_medium_size() {
        // What Telegram usually sends: thumbnail, small, medium, large.
        assert_eq!(medium_size(&[(90, 67), (320, 240), (800, 600), (1280, 960)]), Some(1));
        assert_eq!(medium_size(&[(90, 67), (320, 240), (640, 480), (1280, 960)]), Some(2));
        // Portrait photos count their longest side.
        assert_eq!(medium_size(&[(240, 320), (480, 640), (960, 1280)]), Some(1));
        // All bigger: the smallest.
        assert_eq!(medium_size(&[(1280, 960), (800, 600)]), Some(1));
        assert_eq!(medium_size(&[]), None);
    }
}
