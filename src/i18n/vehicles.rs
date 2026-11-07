//! Drivers' vehicles: cards, lists, changes and approvals.

use crate::domain::vehicle::{MAX_PER_DRIVER, Vehicle, VehicleId, VehicleKind};
use super::Lang;

/// A vehicle in one line: "🚗 Lada rojo · P123456".
pub fn vehicle_line(v: &Vehicle) -> String {
    format!("{} {} {} · {}", v.kind.emoji(), v.model, v.color, v.plate)
}

/// In the 🧢 Driver menu: "Trabajando con: 🚗 Lada rojo · P123456".
pub fn working_with(lang: Lang, active: Option<&Vehicle>) -> String {
    match (lang, active) {
        (Lang::Es, Some(v)) => format!("Trabajando con: {}", vehicle_line(v)),
        (Lang::En, Some(v)) => format!("Working with: {}", vehicle_line(v)),
        (Lang::Es, None) => "Sin vehículo activo: elige uno en 🚘 Mis vehículos para tomar viajes.".into(),
        (Lang::En, None) => "No active vehicle: pick one in 🚘 My vehicles to take trips.".into(),
    }
}

/// 🚘 My vehicles, above one button per vehicle.
pub fn vehicles_list(lang: Lang, vehicles: &[Vehicle], active: Option<VehicleId>) -> String {
    let current = working_with(lang, vehicles.iter().find(|v| Some(v.id) == active).filter(|v| v.approved()));
    let n = vehicles.len();
    match (lang, n) {
        (Lang::Es, 0) => "🚘 Mis vehículos\n\nNo tienes vehículos. Añade uno para tomar viajes.".into(),
        (Lang::En, 0) => "🚘 My vehicles\n\nYou have no vehicles. Add one to take trips.".into(),
        (Lang::Es, _) => format!(
            "🚘 Mis vehículos ({n} de {MAX_PER_DRIVER})\n{current}\n\n✅ activo · ⏳ espera aprobación. Pulsa uno para verlo, usarlo o cambiarlo."
        ),
        (Lang::En, _) => format!(
            "🚘 My vehicles ({n} of {MAX_PER_DRIVER})\n{current}\n\n✅ active · ⏳ waiting for approval. Tap one to see, use or change it."
        ),
    }
}

/// A vehicle's button in the list: "✅ 🚗 Lada rojo · P123456".
pub fn vehicle_button(v: &Vehicle, active: bool) -> String {
    let mark = if !v.approved() { "⏳ " } else if active { "✅ " } else { "" };
    format!("{mark}{}", vehicle_line(v))
}

/// One vehicle's card in 🚘 My vehicles.
pub fn vehicle_card(lang: Lang, v: &Vehicle, active: bool) -> String {
    let status = match (lang, v.approved(), active) {
        (Lang::Es, false, _) => "⏳ Espera la aprobación de un admin",
        (Lang::En, false, _) => "⏳ Waiting for an admin's approval",
        (Lang::Es, true, true) => "✅ Es tu vehículo activo",
        (Lang::En, true, true) => "✅ Your active vehicle",
        (Lang::Es, true, false) => "Aprobado",
        (Lang::En, true, false) => "Approved",
    };
    let mut card = format!("{}\n{}\n{status}", vehicle_kind(lang, v.kind), vehicle_lines(lang, v.kind, Some(v)));
    if let Some(plate) = &v.pending_plate {
        card.push_str(&match lang {
            Lang::Es => format!("\n⏳ Chapa nueva en revisión: {plate}"),
            Lang::En => format!("\n⏳ New plate under review: {plate}"),
        });
    }
    if v.pending_photo_file_id.is_some() {
        card.push_str(match lang {
            Lang::Es => "\n⏳ Foto nueva en revisión",
            Lang::En => "\n⏳ New photo under review",
        });
    }
    card.push_str(match lang {
        Lang::Es => "\n\n✏️ Para cambiar un dato, pulsa su botón. La chapa y la foto las revisa un admin.",
        Lang::En => "\n\n✏️ To change something, tap its button. An admin reviews the plate and the photo.",
    });
    card
}

/// To admins: a new vehicle of an approved driver (decision 122).
pub fn new_vehicle(lang: Lang, alias: &str, v: &Vehicle) -> String {
    let lines = vehicle_lines(lang, v.kind, Some(v));
    match lang {
        Lang::Es => format!("🆕 {alias} añadió un vehículo:\n\n{lines}\n\nNo lo puede usar hasta que lo apruebes."),
        Lang::En => format!("🆕 {alias} added a vehicle:\n\n{lines}\n\nThey can't use it until you approve it."),
    }
}

/// To admins: a new plate for an approved vehicle, the old one still counts.
pub fn vehicle_plate_change(lang: Lang, alias: &str, v: &Vehicle, new: &str) -> String {
    let name = vehicle_name(lang, v.kind, Some(v));
    match lang {
        Lang::Es => format!("🔢 {alias} quiere cambiar la chapa de {name}:\n{} → {new}", v.plate),
        Lang::En => format!("🔢 {alias} wants to change the plate of {name}:\n{} → {new}", v.plate),
    }
}

/// To admins, with the old photo (`new` false) and then the new one.
pub fn vehicle_photo_change(lang: Lang, alias: &str, v: &Vehicle, new: bool) -> String {
    let name = vehicle_name(lang, v.kind, Some(v));
    match (lang, new) {
        (Lang::Es, false) => format!("📷 {alias} quiere cambiar la foto de {name}. Esta es la de ahora."),
        (Lang::Es, true) => format!("📷 {alias} quiere cambiar la foto de {name}. Esta es la nueva."),
        (Lang::En, false) => format!("📷 {alias} wants to change the photo of {name}. This is the current one."),
        (Lang::En, true) => format!("📷 {alias} wants to change the photo of {name}. This is the new one."),
    }
}

/// To the driver once an admin decides on a new vehicle.
pub fn vehicle_decided(lang: Lang, v: &Vehicle, approved: bool, active: bool) -> String {
    let line = vehicle_line(v);
    match (lang, approved, active) {
        (Lang::Es, true, true) => format!("✅ Tu vehículo {line} fue aprobado y es tu vehículo activo."),
        (Lang::Es, true, false) => format!("✅ Tu vehículo {line} fue aprobado. Úsalo cuando quieras en 🚘 Mis vehículos."),
        (Lang::Es, false, _) => format!("❌ Tu vehículo {line} no fue aprobado. Puedes añadirlo de nuevo en 🚘 Mis vehículos."),
        (Lang::En, true, true) => format!("✅ Your vehicle {line} was approved and is your active vehicle."),
        (Lang::En, true, false) => format!("✅ Your vehicle {line} was approved. Use it whenever you like in 🚘 My vehicles."),
        (Lang::En, false, _) => format!("❌ Your vehicle {line} was not approved. You can add it again in 🚘 My vehicles."),
    }
}

/// To the driver once an admin decides on a new plate or photo.
pub fn vehicle_change_decided(lang: Lang, v: &Vehicle, photo: bool, approved: bool) -> String {
    let line = vehicle_line(v);
    match (lang, photo, approved) {
        (Lang::Es, false, true) => format!("✅ La chapa nueva fue aprobada: {line}"),
        (Lang::Es, true, true) => format!("✅ La foto nueva de {line} fue aprobada."),
        (Lang::Es, false, false) => format!("❌ La chapa nueva no fue aprobada; sigue la de antes: {line}"),
        (Lang::Es, true, false) => format!("❌ La foto nueva de {line} no fue aprobada; sigue la de antes."),
        (Lang::En, false, true) => format!("✅ The new plate was approved: {line}"),
        (Lang::En, true, true) => format!("✅ The new photo of {line} was approved."),
        (Lang::En, false, false) => format!("❌ The new plate was not approved; the old one stays: {line}"),
        (Lang::En, true, false) => format!("❌ The new photo of {line} was not approved; the old one stays."),
    }
}

/// "🚗 Usar Lada rojo y aceptar" (decision 128).
pub fn use_vehicle_and(lang: Lang, v: &Vehicle, accept: bool) -> String {
    let (emoji, name) = (v.kind.emoji(), format!("{} {}", v.model, v.color));
    match (lang, accept) {
        (Lang::Es, true) => format!("{emoji} Usar {name} y aceptar"),
        (Lang::Es, false) => format!("{emoji} Usar {name} y contraofertar"),
        (Lang::En, true) => format!("{emoji} Use {name} and accept"),
        (Lang::En, false) => format!("{emoji} Use {name} and counteroffer"),
    }
}

/// The kind with its emoji: "🚗 Carro".
pub fn vehicle_kind(lang: Lang, kind: VehicleKind) -> String {
    let name = match (lang, kind) {
        (Lang::Es, VehicleKind::Moto) | (Lang::En, VehicleKind::Moto) => "Moto",
        (Lang::Es, VehicleKind::Car) => "Carro",
        (Lang::En, VehicleKind::Car) => "Car",
        (Lang::Es, VehicleKind::Tricycle) => "Triciclo",
        (Lang::En, VehicleKind::Tricycle) => "Tricycle",
    };
    format!("{} {name}", kind.emoji())
}

/// "🚗 Lada, rojo"; only the kind ("🚗 Carro") if the vehicle is gone.
pub fn vehicle_name(lang: Lang, kind: VehicleKind, vehicle: Option<&Vehicle>) -> String {
    match vehicle {
        Some(v) => format!("{} {}, {}", v.kind.emoji(), v.model, v.color),
        None => vehicle_kind(lang, kind),
    }
}

/// The vehicle and, on its own line, its plate.
pub fn vehicle_lines(lang: Lang, kind: VehicleKind, vehicle: Option<&Vehicle>) -> String {
    let name = vehicle_name(lang, kind, vehicle);
    match (lang, vehicle) {
        (Lang::Es, Some(v)) => format!("{name}\n🔢 Chapa: {}", v.plate),
        (Lang::En, Some(v)) => format!("{name}\n🔢 Plate: {}", v.plate),
        (_, None) => name,
    }
}
