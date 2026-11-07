//! A driver's vehicles (decisions 120 to 130): 🚘 My vehicles (list, card,
//! use, remove, edit, add), the admins' approval of new vehicles and of
//! plate and photo changes, and the shortcut to switch to the right one for
//! a request (decision 128).

use teloxide::prelude::*;
use teloxide::types::{FileId, InputFile, MessageId};

use super::client::Ctx;
use super::dialog::{Dialog, DriverForm, FormStep};
use super::{App, auction, chat, driver, keyboards, now, panel, support, trip};
use crate::db;
use crate::db::vehicles::{Field, Saved};
use crate::domain::driver::DriverStatus;
use crate::domain::ids::{RequestId, UserId};
use crate::domain::request::{DomainError, Request};
use crate::domain::vehicle::{self, NewVehicle, Vehicle, VehicleId, VehicleKind};
use crate::i18n::{self, Button, Lang, Text, parse_button};

/// What the driver wanted to do with the request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Then {
    Accept,
    Counteroffer,
}

impl Then {
    /// In the callback: `vsa:<request>:<vehicle>` or `vsc:…`.
    pub fn code(self) -> &'static str {
        match self {
            Then::Accept => "vsa",
            Then::Counteroffer => "vsc",
        }
    }
}

/// The driver's active vehicle does not fit `request` (`error`). If they have
/// an approved one that does, they get a private message saying why, with a
/// button to switch to it and go on, and `None` is returned. Otherwise, why.
pub async fn offer_switch(
    bot: &Bot,
    app: &App,
    driver: UserId,
    lang: Lang,
    request: &Request,
    error: &DomainError,
    then: Then,
) -> anyhow::Result<Option<String>> {
    let vehicles = db::vehicles::for_driver(&app.pool, driver).await?;
    let explained = i18n::domain_error(lang, error);
    let Some(fit) = vehicle::switch_candidate(request.vehicle_kind, &vehicles) else {
        return Ok(Some(explained));
    };
    let markup = keyboards::switch_vehicle(lang, &request.id, fit, then);
    bot.send_message(chat(driver), explained).reply_markup(markup).await?;
    Ok(None)
}

/// `vsa`/`vsc`: makes `vehicle` active, then accepts or opens the
/// counteroffer. Returns the alert, if any.
pub async fn switch_and(cx: &Ctx<'_>, id: &RequestId, vehicle: VehicleId, then: Then) -> anyhow::Result<Option<String>> {
    let app = cx.app;
    let now = now();
    if !db::vehicles::set_active(&app.pool, cx.user, vehicle, now).await? {
        let busy = db::requests::driver_busy(&app.pool, cx.user).await?;
        let bidding = db::vehicles::bidding(&app.pool, cx.user, now).await?;
        let error = match db::vehicles::get(&app.pool, vehicle).await?.filter(|v| v.driver == cx.user) {
            Some(v) => vehicle::check_switch(&v, busy, bidding).err().unwrap_or(DomainError::VehicleLocked),
            None => DomainError::NoActiveVehicle,
        };
        return Ok(Some(i18n::domain_error(cx.lang, &error)));
    }
    tracing::info!(request = %id, "vehicle switched to take a request");
    match then {
        Then::Accept => Ok(Some(trip::accept(cx.bot, app, cx.user, cx.lang, id).await?)),
        Then::Counteroffer => {
            auction::start_counteroffer(cx, id.as_str()).await?;
            Ok(None)
        }
    }
}

/// Only an approved driver manages their vehicles; the alert otherwise. A
/// pending one waits: their first vehicle is in the application.
async fn manager_problem(cx: &Ctx<'_>) -> anyhow::Result<Option<Text>> {
    Ok(match cx.app.driver_status(cx.user).await? {
        None => Some(Text::NotAllowed),
        Some(DriverStatus::Pending) => Some(Text::ApplicationPending),
        Some(DriverStatus::Blocked) => Some(Text::DriverBlocked),
        Some(DriverStatus::Active) => None,
    })
}

/// 🚘 My vehicles: one button per vehicle (✅ active, ⏳ waiting) and ➕ Add.
pub async fn list(cx: &Ctx<'_>, pressed: Option<MessageId>) -> anyhow::Result<()> {
    if let Some(problem) = manager_problem(cx).await? {
        cx.bot.send_message(chat(cx.user), i18n::text(cx.lang, problem)).await?;
        return Ok(());
    }
    let vehicles = db::vehicles::for_driver(&cx.app.pool, cx.user).await?;
    let active = db::vehicles::active(&cx.app.pool, cx.user).await?.map(|v| v.id);
    let text = i18n::vehicles_list(cx.lang, &vehicles, active);
    let markup = keyboards::vehicles(cx.lang, &vehicles, active);
    panel::edit_or_send(cx, pressed, text, markup).await
}

/// A note, the driver keyboard back, and the list again.
pub async fn back_to_list(cx: &Ctx<'_>, note: Text) -> anyhow::Result<()> {
    cx.bot.send_message(chat(cx.user), i18n::text(cx.lang, note)).reply_markup(keyboards::driver_menu(cx.lang)).await?;
    list(cx, None).await
}

/// The driver's own vehicle, or `None` (gone, or someone else's).
async fn own(cx: &Ctx<'_>, id: VehicleId) -> anyhow::Result<Option<Vehicle>> {
    Ok(db::vehicles::get(&cx.app.pool, id).await?.filter(|v| v.driver == cx.user))
}

/// `vh:<id>`: one vehicle's card, with Use, the field buttons, Remove, Back.
pub async fn card(cx: &Ctx<'_>, id: VehicleId, pressed: Option<MessageId>) -> anyhow::Result<Option<String>> {
    if let Some(problem) = manager_problem(cx).await? {
        return Ok(Some(i18n::text(cx.lang, problem).into()));
    }
    let Some(v) = own(cx, id).await? else {
        list(cx, pressed).await?;
        return Ok(None);
    };
    let active = db::vehicles::active(&cx.app.pool, cx.user).await?.map(|a| a.id) == Some(v.id);
    let text = i18n::vehicle_card(cx.lang, &v, active);
    panel::edit_or_send(cx, pressed, text, keyboards::vehicle_card(cx.lang, &v, active)).await?;
    Ok(None)
}

/// `vph:<id>`: the vehicle's photo, as a message of its own.
pub async fn photo(cx: &Ctx<'_>, id: VehicleId) -> anyhow::Result<Option<String>> {
    match own(cx, id).await?.and_then(|v| v.photo_file_id) {
        Some(file) => {
            cx.bot.send_photo(chat(cx.user), InputFile::file_id(FileId(file))).await?;
            Ok(None)
        }
        None => Ok(Some(i18n::text(cx.lang, Text::NoVehiclePhoto).into())),
    }
}

/// `vuse:<id>`: ✅ Use this one, unless busy (decision 120).
pub async fn use_it(cx: &Ctx<'_>, id: VehicleId, pressed: Option<MessageId>) -> anyhow::Result<Option<String>> {
    if let Some(problem) = manager_problem(cx).await? {
        return Ok(Some(i18n::text(cx.lang, problem).into()));
    }
    let Some(v) = own(cx, id).await? else {
        return Ok(None);
    };
    let now = now();
    if !db::vehicles::set_active(&cx.app.pool, cx.user, id, now).await? {
        let busy = db::requests::driver_busy(&cx.app.pool, cx.user).await?;
        let bidding = db::vehicles::bidding(&cx.app.pool, cx.user, now).await?;
        let error = vehicle::check_switch(&v, busy, bidding).err().unwrap_or(DomainError::VehicleLocked);
        return Ok(Some(i18n::domain_error(cx.lang, &error)));
    }
    tracing::info!("active vehicle switched");
    card(cx, id, pressed).await?;
    Ok(Some(i18n::working_with(cx.lang, Some(&v))))
}

/// `vdel:<id>`: asks first; `vdok:<id>` removes it.
pub async fn ask_remove(cx: &Ctx<'_>, id: VehicleId, pressed: Option<MessageId>) -> anyhow::Result<Option<String>> {
    let Some(v) = own(cx, id).await? else {
        return Ok(None);
    };
    let text = format!("{}\n\n{}", i18n::vehicle_line(&v), i18n::text(cx.lang, Text::RemoveVehicleSure));
    panel::edit_or_send(cx, pressed, text, keyboards::remove_vehicle(cx.lang, id)).await?;
    Ok(None)
}

pub async fn remove(cx: &Ctx<'_>, id: VehicleId, pressed: Option<MessageId>) -> anyhow::Result<Option<String>> {
    let Some(v) = own(cx, id).await? else {
        list(cx, pressed).await?;
        return Ok(None);
    };
    let now = now();
    let active = db::vehicles::active(&cx.app.pool, cx.user).await?.map(|a| a.id);
    if !db::vehicles::remove(&cx.app.pool, cx.user, id, now).await? {
        let busy = db::requests::driver_busy(&cx.app.pool, cx.user).await?;
        let bidding = db::vehicles::bidding(&cx.app.pool, cx.user, now).await?;
        let error = vehicle::check_remove(&v, active, busy, bidding).err().unwrap_or(DomainError::VehicleLocked);
        return Ok(Some(i18n::domain_error(cx.lang, &error)));
    }
    tracing::info!("vehicle removed");
    list(cx, pressed).await?;
    // Still a driver; without an active vehicle until they pick one.
    let note = if active == Some(id) { Text::VehicleRemovedPickAnother } else { Text::VehicleRemoved };
    Ok(Some(i18n::text(cx.lang, note).into()))
}

/// `vadd`: ➕ Add a vehicle: the kind, plate, model, color and photo.
pub async fn start_add(cx: &Ctx<'_>) -> anyhow::Result<Option<String>> {
    if let Some(problem) = manager_problem(cx).await? {
        return Ok(Some(i18n::text(cx.lang, problem).into()));
    }
    let count = db::vehicles::for_driver(&cx.app.pool, cx.user).await?.len();
    if let Err(e) = vehicle::check_add(count) {
        return Ok(Some(i18n::domain_error(cx.lang, &e)));
    }
    let form = DriverForm { vehicle_only: true, ..DriverForm::default() };
    driver::ask(cx, form, FormStep::Kind).await?;
    Ok(None)
}

/// The vehicle in a finished form; `None` if a step is missing.
pub fn new_vehicle(form: &DriverForm) -> Option<NewVehicle> {
    Some(NewVehicle {
        kind: form.kind?,
        plate: form.plate.clone()?,
        model: form.bike_model.clone()?,
        color: form.bike_color.clone()?,
        photo_file_id: form.bike_photo_file_id.clone(),
    })
}

/// The last step of ➕ Add: saved as pending; the admins decide (decision 122).
pub async fn finish_add(cx: &Ctx<'_>, form: DriverForm) -> anyhow::Result<()> {
    let app = cx.app;
    db::dialogs::clear(&app.pool, cx.user).await?;
    let Some(new) = new_vehicle(&form) else {
        return back_to_list(cx, Text::Cancelled).await;
    };
    let Some(id) = db::vehicles::add(&app.pool, cx.user, &new, now()).await? else {
        let note = i18n::domain_error(cx.lang, &DomainError::TooManyVehicles);
        cx.bot.send_message(chat(cx.user), note).reply_markup(keyboards::driver_menu(cx.lang)).await?;
        return list(cx, None).await;
    };
    tracing::info!("vehicle added");
    if let (Some(v), Some(d)) = (db::vehicles::get(&app.pool, id).await?, db::drivers::get(&app.pool, &app.cipher, cx.user).await?) {
        let text = |lang| i18n::new_vehicle(lang, &d.profile.alias, &v);
        support::alert(cx.bot, app, text, v.photo_file_id.clone(), |lang| keyboards::review_vehicle(lang, id)).await?;
    }
    back_to_list(cx, Text::VehicleSent).await
}

/// `vf:<id>:<field>`: asks for the new plate, model, color or photo.
pub async fn edit_field(cx: &Ctx<'_>, id: VehicleId, code: &str) -> anyhow::Result<Option<String>> {
    if let Some(problem) = manager_problem(cx).await? {
        return Ok(Some(i18n::text(cx.lang, problem).into()));
    }
    let field = FormStep::parse(code).filter(|f| field_of(*f).is_some());
    let (Some(field), Some(_)) = (field, own(cx, id).await?) else {
        return Ok(None);
    };
    db::dialogs::set(&cx.app.pool, cx.user, &Dialog::EditVehicle { vehicle: id, field }, now()).await?;
    cx.bot
        .send_message(chat(cx.user), i18n::text(cx.lang, driver::question(field)))
        .reply_markup(keyboards::step(cx.lang, &[]))
        .await?;
    Ok(None)
}

fn field_of(step: FormStep) -> Option<Field> {
    match step {
        FormStep::Plate => Some(Field::Plate),
        FormStep::BikeModel => Some(Field::Model),
        FormStep::BikeColor => Some(Field::Color),
        FormStep::BikePhoto => Some(Field::Photo),
        _ => None,
    }
}

/// The new value: the model and color at once; a plate or photo of an
/// approved vehicle waits for an admin, with the before and after.
pub async fn on_edit_message(cx: &Ctx<'_>, id: VehicleId, step: FormStep, msg: &Message) -> anyhow::Result<()> {
    let app = cx.app;
    if msg.text().map(str::trim).and_then(parse_button) == Some(Button::StepBack) {
        db::dialogs::clear(&app.pool, cx.user).await?;
        return back_to_card(cx, id, Text::Cancelled).await;
    }
    let (Some(field), Some(value)) = (field_of(step), driver::read_value(step, msg)) else {
        cx.bot.send_message(chat(cx.user), i18n::text(cx.lang, driver::complaint(step))).await?;
        return Ok(());
    };
    db::dialogs::clear(&app.pool, cx.user).await?;
    let Some(before) = own(cx, id).await? else {
        return back_to_list(cx, Text::Cancelled).await;
    };
    let note = match db::vehicles::change(&app.pool, cx.user, id, field, &value).await? {
        None => Text::Cancelled,
        Some(Saved::Applied) => Text::VehicleUpdated,
        Some(Saved::Waiting) => {
            tracing::info!("vehicle change sent for review");
            let alias = db::drivers::get(&app.pool, &app.cipher, cx.user).await?.map(|d| d.profile.alias).unwrap_or_default();
            let markup = |lang| keyboards::review_vehicle_change(lang, id, step.code());
            match field {
                Field::Photo => {
                    // The old one first, then the new one with the buttons.
                    if let Some(old) = before.photo_file_id.clone() {
                        let text = |lang| i18n::vehicle_photo_change(lang, &alias, &before, false);
                        support::alert(cx.bot, app, text, Some(old), |_| Default::default()).await?;
                    }
                    let text = |lang| i18n::vehicle_photo_change(lang, &alias, &before, true);
                    support::alert(cx.bot, app, text, Some(value.clone()), markup).await?;
                }
                _ => {
                    let text = |lang| i18n::vehicle_plate_change(lang, &alias, &before, &value);
                    support::alert(cx.bot, app, text, None, markup).await?;
                }
            }
            Text::VehicleChangeSent
        }
    };
    back_to_card(cx, id, note).await
}

async fn back_to_card(cx: &Ctx<'_>, id: VehicleId, note: Text) -> anyhow::Result<()> {
    cx.bot.send_message(chat(cx.user), i18n::text(cx.lang, note)).reply_markup(keyboards::driver_menu(cx.lang)).await?;
    card(cx, id, None).await.map(|_| ())
}

/// `vok:<id>` / `vno:<id>`: staff approve or reject a new vehicle; the first
/// decision wins. A rejected one is deleted (decision 129).
pub async fn review(cx: &Ctx<'_>, approve: bool, id: VehicleId, pressed: Option<MessageId>) -> anyhow::Result<Option<String>> {
    let app = cx.app;
    if !app.is_staff(cx.user).await? {
        return Ok(Some(i18n::text(cx.lang, Text::NotAllowed).into()));
    }
    if let Some(msg) = pressed {
        let _ = cx.bot.edit_message_reply_markup(chat(cx.user), msg).await;
    }
    let Some(v) = db::vehicles::get(&app.pool, id).await? else {
        return Ok(Some(i18n::text(cx.lang, Text::AlreadyDecided).into()));
    };
    let decided = if approve {
        db::vehicles::approve(&app.pool, id, cx.user).await?
    } else {
        db::vehicles::reject(&app.pool, id).await?
    };
    if !decided {
        return Ok(Some(i18n::text(cx.lang, Text::AlreadyDecided).into()));
    }
    tracing::info!(approve, "vehicle decided");
    let lang = app.lang_of(v.driver).await;
    let active = db::vehicles::active(&app.pool, v.driver).await?.map(|a| a.id) == Some(id);
    let to_driver = i18n::vehicle_decided(lang, &v, approve, active);
    if let Err(e) = cx.bot.send_message(chat(v.driver), to_driver).await {
        tracing::warn!(error = %e, "cannot tell the driver about the vehicle");
    }
    let done = if approve { Text::StaffApproved } else { Text::StaffRejected };
    Ok(Some(i18n::text(cx.lang, done).into()))
}

/// `vck:<id>:<field>` / `vcn:…`: staff decide on a waiting plate or photo.
pub async fn review_change(
    cx: &Ctx<'_>,
    approve: bool,
    id: VehicleId,
    code: &str,
    pressed: Option<MessageId>,
) -> anyhow::Result<Option<String>> {
    let app = cx.app;
    if !app.is_staff(cx.user).await? {
        return Ok(Some(i18n::text(cx.lang, Text::NotAllowed).into()));
    }
    let Some(field) = FormStep::parse(code).and_then(field_of) else {
        return Ok(None);
    };
    if let Some(msg) = pressed {
        let _ = cx.bot.edit_message_reply_markup(chat(cx.user), msg).await;
    }
    let before = db::vehicles::get(&app.pool, id).await?;
    if before.is_none() || !db::vehicles::decide_change(&app.pool, id, field, approve).await? {
        return Ok(Some(i18n::text(cx.lang, Text::AlreadyDecided).into()));
    }
    tracing::info!(approve, "vehicle change decided");
    if let Some(v) = db::vehicles::get(&app.pool, id).await? {
        let lang = app.lang_of(v.driver).await;
        let to_driver = i18n::vehicle_change_decided(lang, &v, field == Field::Photo, approve);
        if let Err(e) = cx.bot.send_message(chat(v.driver), to_driver).await {
            tracing::warn!(error = %e, "cannot tell the driver about the vehicle change");
        }
    }
    let done = if approve { Text::StaffApproved } else { Text::StaffRejected };
    Ok(Some(i18n::text(cx.lang, done).into()))
}

/// The kind named on a vehicle step's button, in either language.
pub fn kind_named(text: &str) -> Option<VehicleKind> {
    VehicleKind::ALL.into_iter().find(|&k| [Lang::Es, Lang::En].into_iter().any(|lang| i18n::vehicle_kind(lang, k) == text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_kind_buttons_are_read_back_in_both_languages() {
        assert_eq!(kind_named("🚗 Carro"), Some(VehicleKind::Car));
        assert_eq!(kind_named("🛺 Tricycle"), Some(VehicleKind::Tricycle));
        assert_eq!(kind_named("Carro"), None);
    }

    #[test]
    fn a_form_without_a_step_has_no_vehicle() {
        let mut form = DriverForm {
            kind: Some(VehicleKind::Car),
            plate: Some("P1".into()),
            bike_model: Some("Lada".into()),
            ..DriverForm::default()
        };
        assert_eq!(new_vehicle(&form), None);
        form.bike_color = Some("rojo".into());
        let v = new_vehicle(&form).unwrap();
        assert_eq!((v.kind, v.photo_file_id), (VehicleKind::Car, None));
    }
}
