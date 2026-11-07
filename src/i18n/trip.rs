//! Trip cards, notices while a trip runs, how it ends, ratings and sharing.

use crate::domain::driver::Profile;
use crate::domain::effect::Notice;
use crate::domain::fare::Cup;
use crate::domain::ids::RequestId;
use crate::domain::reputation::{Role, Shown};
use crate::domain::request::{CancelReason, DomainError, PaymentMethod};
use crate::domain::vehicle::{MAX_PER_DRIVER, Vehicle, VehicleKind};
use super::{Lang, payment, reputation, vehicle_kind, vehicle_name};

/// The two trip cards (the client's and the driver's) use the same lines
/// and emojis, each detail once: the trip with its kind's emoji, 👤 the other person with their
/// reputation, 📞 their phone, 💰 what is paid at the end, 💬 who to write to.
pub struct TripCard<'a> {
    pub request: &'a RequestId,
    /// Its emoji opens the card: 🏍, 🚗 or 🛺.
    pub kind: VehicleKind,
    pub price: Cup,
    pub pay: PaymentMethod,
    /// "📏 3.5 km · ⏱ unos 9 min".
    pub distance: String,
    /// The other person's names: the driver's alias and full name, or the
    /// client's name (none: "Client").
    pub names: Vec<&'a str>,
    pub reputation: &'a Shown,
    pub phone: Option<&'a str>,
    pub username: Option<&'a str>,
}

/// What the client sees once their driver is set: `accepted` when it came
/// from a counteroffer (or a driver they had not confirmed yet).
pub fn client_trip_card(lang: Lang, c: &TripCard, kind: VehicleKind, vehicle: Option<&Vehicle>, accepted: bool) -> String {
    let (request, e) = (c.request, c.kind.emoji());
    let title = match (lang, accepted) {
        (Lang::Es, false) => format!("{e} Viaje {request} confirmado: tu chofer va hacia ti."),
        (Lang::Es, true) => format!("{e} ¡Un chofer aceptó tu viaje {request}! Va hacia ti."),
        (Lang::En, false) => format!("{e} Trip {request} confirmed: your driver is on the way."),
        (Lang::En, true) => format!("{e} A driver accepted your trip {request}! They're on the way."),
    };
    let name = vehicle_name(lang, kind, vehicle);
    let bike = match (lang, vehicle) {
        (Lang::Es, Some(v)) => format!("{name} · Chapa {}", v.plate),
        (Lang::En, Some(v)) => format!("{name} · Plate {}", v.plate),
        (_, None) => name,
    };
    let money = match lang {
        Lang::Es => format!("💰 Al terminar le pagas {} CUP · {}", c.price, payment(lang, c.pay)),
        Lang::En => format!("💰 When it ends you pay {} CUP · {}", c.price, payment(lang, c.pay)),
    };
    trip_card(lang, title, c, &[bike], money)
}

/// What the driver sees once the trip is theirs.
pub fn driver_trip_card(lang: Lang, c: &TripCard, reference: Option<&str>) -> String {
    let (request, e) = (c.request, c.kind.emoji());
    let title = match lang {
        Lang::Es => format!("{e} Viaje {request}: es tuyo."),
        Lang::En => format!("{e} Trip {request}: it's yours."),
    };
    let extra: Vec<String> = reference
        .map(|r| match lang {
            Lang::Es => format!("📝 Referencia: {r}"),
            Lang::En => format!("📝 Landmark: {r}"),
        })
        .into_iter()
        .collect();
    let money = match lang {
        Lang::Es => format!("💰 Al terminar cobras {} CUP · {}", c.price, payment(lang, c.pay)),
        Lang::En => format!("💰 When it ends you charge {} CUP · {}", c.price, payment(lang, c.pay)),
    };
    let mut card = trip_card(lang, title, c, &extra, money);
    card.push_str(match lang {
        Lang::Es => "\nAbajo tienes la recogida y el destino.",
        Lang::En => "\nThe pickup and destination are below.",
    });
    card
}

fn trip_card(lang: Lang, title: String, c: &TripCard, extra: &[String], money: String) -> String {
    let fallback = match lang {
        Lang::Es => "Cliente",
        Lang::En => "Client",
    };
    let names = if c.names.is_empty() { fallback.to_owned() } else { c.names.join(" · ") };
    let mut lines = vec![title, c.distance.clone(), format!("👤 {names}")];
    lines.extend(extra.iter().cloned());
    if let Some(phone) = c.phone {
        lines.push(format!("📞 {phone}"));
    }
    lines.push(money);
    lines.push(match (lang, c.username, c.phone) {
        (Lang::Es, Some(u), _) => format!("💬 Escríbele a @{u} para terminar de coordinar el viaje."),
        (Lang::En, Some(u), _) => format!("💬 Write to @{u} to finish arranging the trip."),
        (Lang::Es, None, Some(p)) => format!("💬 Llámale al {p} para terminar de coordinar el viaje."),
        (Lang::En, None, Some(p)) => format!("💬 Call {p} to finish arranging the trip."),
        (Lang::Es, None, None) => "💬 Te esperará en el punto de recogida.".into(),
        (Lang::En, None, None) => "💬 They'll wait at the pickup point.".into(),
    });
    lines.push(reputation(lang, c.reputation));
    lines.join("\n")
}

pub fn you_completed(lang: Lang, request: &RequestId) -> String {
    notice(lang, &Notice::Completed { request: request.clone() })
}

pub fn you_cancelled(lang: Lang, request: &RequestId) -> String {
    match lang {
        Lang::Es => format!("❌ Cancelaste el viaje {request}."),
        Lang::En => format!("❌ You cancelled trip {request}."),
    }
}

/// "👤 My details": what the driver of a trip gets (all optional).
pub fn client_details(lang: Lang, name: Option<&str>, phone: Option<&str>, has_photo: bool) -> String {
    let none = "—";
    let photo = match (lang, has_photo) {
        (Lang::Es, true) => "sí",
        (Lang::En, true) => "yes",
        (_, false) => none,
    };
    let (name, phone) = (name.unwrap_or(none), phone.unwrap_or(none));
    match lang {
        Lang::Es => format!(
            "👤 Mis datos\n\n🙋 Nombre: {name}\n📞 Celular: {phone}\n📷 Foto: {photo}\n\nTodo es opcional. Solo los ve el chofer de tu viaje, cuando lo confirmas."
        ),
        Lang::En => format!(
            "👤 My details\n\n🙋 Name: {name}\n📞 Phone: {phone}\n📷 Photo: {photo}\n\nAll optional. Only the driver of your trip sees them, once you confirm them."
        ),
    }
}

/// Base text of a notice. The cards (`ConfirmDriver`, `DriverConfirmed`,
/// `DriverAssigned`, `TripAssigned`) are rendered with their own functions,
/// which have the driver's or client's details.
pub fn notice(lang: Lang, notice: &Notice) -> String {
    use Notice::*;
    match (lang, notice) {
        (Lang::Es, Published { request }) => format!(
            "📣 Tu solicitud {request} ya está publicada (hasta 60 min). Te aviso cuando un chofer la acepte o contraoferte."
        ),
        (Lang::En, Published { request }) => format!(
            "📣 Your request {request} is live (up to 60 min). I'll let you know when a driver accepts or counteroffers."
        ),
        (Lang::Es, SuggestRaise { request }) => format!("⏳ Ningún chofer ha tomado aún tu viaje {request}."),
        (Lang::En, SuggestRaise { request }) => format!("⏳ No driver has taken your trip {request} yet."),
        (Lang::Es, PriceRaised { request, price }) => {
            format!("⬆️ Subiste el precio de la solicitud {request} a {price} CUP.")
        }
        (Lang::En, PriceRaised { request, price }) => format!("⬆️ You raised request {request} to {price} CUP."),
        (Lang::Es, StillWaiting { request }) => format!(
            "⏳ Tu solicitud {request} lleva 30 min publicada. ¿Sigues esperando? Si no respondes en 10 min, la retiro."
        ),
        (Lang::En, StillWaiting { request }) => format!(
            "⏳ Your request {request} has been live for 30 min. Still waiting? If you don't answer within 10 min, I'll withdraw it."
        ),
        (Lang::Es, ConfirmDriver { request, .. }) => format!("👤 Un chofer quiere hacer tu viaje {request}."),
        (Lang::En, ConfirmDriver { request, .. }) => format!("👤 A driver wants to take your trip {request}."),
        (Lang::Es, AwaitingClient { request }) => format!(
            "⏳ Reservaste la solicitud {request}. El cliente tiene 5 min para confirmarte; te aviso enseguida."
        ),
        (Lang::En, AwaitingClient { request }) => format!(
            "⏳ You reserved request {request}. The client has 5 min to confirm you; I'll let you know."
        ),
        (Lang::Es, DriverConfirmed { request, .. }) => format!("✅ Confirmaste al chofer del viaje {request}."),
        (Lang::En, DriverConfirmed { request, .. }) => format!("✅ You confirmed the driver for trip {request}."),
        (Lang::Es, DriverAssigned { request, .. }) => format!("✅ ¡Un chofer aceptó tu viaje {request}!"),
        (Lang::En, DriverAssigned { request, .. }) => format!("✅ A driver accepted your trip {request}!"),
        (Lang::Es, ClientOnTheWay { request }) => format!("🚶 El cliente del viaje {request} va hacia el punto de recogida."),
        (Lang::En, ClientOnTheWay { request }) => format!("🚶 The client of trip {request} is heading to the pickup."),
        (Lang::Es, DriverArrived { request }) => format!("📍 Tu chofer del viaje {request} llegó al punto de recogida."),
        (Lang::En, DriverArrived { request }) => format!("📍 Your driver for trip {request} is at the pickup."),
        (Lang::Es, TripAssigned { request, .. }) => format!("✅ El viaje {request} es tuyo."),
        (Lang::En, TripAssigned { request, .. }) => format!("✅ Trip {request} is yours."),
        (Lang::Es, ClientDeclined { request }) => format!("❌ El cliente prefirió no confirmar la solicitud {request}."),
        (Lang::En, ClientDeclined { request }) => format!("❌ The client chose not to confirm request {request}."),
        (Lang::Es, ClientDidNotConfirm { request }) => {
            format!("⌛ El cliente no confirmó la solicitud {request} a tiempo. Ya estás libre.")
        }
        (Lang::En, ClientDidNotConfirm { request }) => {
            format!("⌛ The client didn't confirm request {request} in time. You're free again.")
        }
        (Lang::Es, CounterofferRejected { request }) => {
            format!("❌ El cliente rechazó tu contraoferta en la solicitud {request}. Puedes hacer otra.")
        }
        (Lang::En, CounterofferRejected { request }) => {
            format!("❌ The client rejected your counteroffer on request {request}. You can make another.")
        }
        (Lang::Es, RequestGone { request }) => format!("La solicitud {request} ya no está disponible."),
        (Lang::En, RequestGone { request }) => format!("Request {request} is no longer available."),
        (Lang::Es, OfferOutbid { request, price }) => format!(
            "⬆️ El cliente subió la solicitud {request} a {price} CUP, así que tu contraoferta se retiró. Puedes aceptarla directamente en el canal."
        ),
        (Lang::En, OfferOutbid { request, price }) => format!(
            "⬆️ The client raised request {request} to {price} CUP, so your counteroffer was dropped. You can accept it directly in the channel."
        ),
        (Lang::Es, DriverWithdrew { request }) => {
            format!("🚪 El chofer se retiró. Tu solicitud {request} vuelve a estar publicada.")
        }
        (Lang::En, DriverWithdrew { request }) => {
            format!("🚪 The driver withdrew. Your request {request} is published again.")
        }
        (Lang::Es, Completed { request }) => format!("🏁 Viaje {request} completado. ¡Gracias!"),
        (Lang::En, Completed { request }) => format!("🏁 Trip {request} completed. Thanks!"),
        (Lang::Es, CancelledByClient { request }) => format!("❌ El cliente canceló el viaje {request}."),
        (Lang::En, CancelledByClient { request }) => format!("❌ The client cancelled trip {request}."),
        (Lang::Es, CancelledByDriver { request }) => format!("❌ El chofer canceló el viaje {request}."),
        (Lang::En, CancelledByDriver { request }) => format!("❌ The driver cancelled trip {request}."),
        (Lang::Es, ConfirmTimedOut { request }) => {
            format!("⌛ No confirmaste al chofer a tiempo, así que la solicitud {request} se canceló.")
        }
        (Lang::En, ConfirmTimedOut { request }) => {
            format!("⌛ You didn't confirm the driver in time, so request {request} was cancelled.")
        }
        (Lang::Es, DeclinedDriver { request }) => format!("❌ Cancelaste la solicitud {request}."),
        (Lang::En, DeclinedDriver { request }) => format!("❌ You cancelled request {request}."),
        (Lang::Es, Expired { request }) => format!("⌛ La solicitud {request} se retiró del canal."),
        (Lang::En, Expired { request }) => format!("⌛ Request {request} was withdrawn from the channel."),
    }
}

/// Appended to the notices that offer the client to publish the same trip again.
pub fn republish_hint(lang: Lang) -> &'static str {
    match lang {
        Lang::Es => "Puedes publicarla de nuevo con el mismo precio.",
        Lang::En => "You can publish it again at the same price.",
    }
}

pub fn domain_error(lang: Lang, error: &DomainError) -> String {
    use DomainError::*;
    match (lang, error) {
        (Lang::Es, PriceBelowMinimum { minimum, .. }) => {
            format!("El precio mínimo para este viaje es {minimum} CUP.")
        }
        (Lang::En, PriceBelowMinimum { minimum, .. }) => {
            format!("The minimum price for this trip is {minimum} CUP.")
        }
        (Lang::Es, PriceNotHigher { current }) => format!("El precio tiene que ser mayor que {current} CUP."),
        (Lang::En, PriceNotHigher { current }) => format!("The price must be higher than {current} CUP."),
        (Lang::Es, NotOpen) => "Esta solicitud ya no está disponible.".into(),
        (Lang::En, NotOpen) => "This request is no longer available.".into(),
        (Lang::Es, NotAssigned) => "Este viaje no está asignado.".into(),
        (Lang::En, NotAssigned) => "This trip is not assigned.".into(),
        (Lang::Es, OwnRequest) => "No puedes tomar tu propia solicitud.".into(),
        (Lang::En, OwnRequest) => "You can't take your own request.".into(),
        (Lang::Es, NotParticipant) => "Solo el cliente o el chofer del viaje pueden hacer esto.".into(),
        (Lang::En, NotParticipant) => "Only the trip's client or driver can do this.".into(),
        (Lang::Es, OfferGone) => "Esa contraoferta ya no es válida.".into(),
        (Lang::En, OfferGone) => "That counteroffer is no longer valid.".into(),
        (Lang::Es, ReasonNeeded) => "Elige un motivo para cancelar.".into(),
        (Lang::En, ReasonNeeded) => "Pick a reason to cancel.".into(),
        (Lang::Es, AlreadyDone) => "Ya lo habías marcado.".into(),
        (Lang::En, AlreadyDone) => "You already did that.".into(),
        (Lang::Es, WrongVehicle { wanted, active }) => format!(
            "Esta solicitud es para {} y tu vehículo activo es {}. Cámbialo en 🧢 Chofer → 🚘 Mis vehículos.",
            vehicle_kind(lang, *wanted),
            vehicle_kind(lang, *active)
        ),
        (Lang::En, WrongVehicle { wanted, active }) => format!(
            "This request is for a {} and your active vehicle is a {}. Change it in 🧢 Driver → 🚘 My vehicles.",
            vehicle_kind(lang, *wanted),
            vehicle_kind(lang, *active)
        ),
        (Lang::Es, NoActiveVehicle) => {
            "No tienes un vehículo activo aprobado. Elige uno en 🧢 Chofer → 🚘 Mis vehículos.".into()
        }
        (Lang::En, NoActiveVehicle) => {
            "You have no approved active vehicle. Pick one in 🧢 Driver → 🚘 My vehicles.".into()
        }
        (Lang::Es, VehicleNotApproved) => "Este vehículo todavía espera la aprobación de un admin.".into(),
        (Lang::En, VehicleNotApproved) => "This vehicle is still waiting for an admin's approval.".into(),
        (Lang::Es, VehicleLocked) => "No puedes cambiar de vehículo con una solicitud en confirmación, \
             un viaje asignado o una contraoferta vigente."
            .into(),
        (Lang::En, VehicleLocked) => "You can't change vehicles while a request is waiting for confirmation, \
             a trip is assigned or a counteroffer is live."
            .into(),
        (Lang::Es, TooManyVehicles) => format!("Puedes tener hasta {MAX_PER_DRIVER} vehículos. Quita uno para añadir otro."),
        (Lang::En, TooManyVehicles) => format!("You can have up to {MAX_PER_DRIVER} vehicles. Remove one to add another."),
    }
}

pub fn cancel_reason(lang: Lang, reason: CancelReason) -> &'static str {
    match (lang, reason) {
        (Lang::Es, CancelReason::DriverNotComing) => "🕒 El chofer no llega",
        (Lang::En, CancelReason::DriverNotComing) => "🕒 The driver isn't coming",
        (Lang::Es, CancelReason::ChangedMind) => "🙅 Ya no lo necesito",
        (Lang::En, CancelReason::ChangedMind) => "🙅 I don't need it anymore",
        (Lang::Es, CancelReason::ClientNoShow) => "🙈 El cliente no apareció",
        (Lang::En, CancelReason::ClientNoShow) => "🙈 The client didn't show up",
        (Lang::Es, CancelReason::Unsafe) => "⚠️ Me sentí inseguro",
        (Lang::En, CancelReason::Unsafe) => "⚠️ I felt unsafe",
        (Lang::Es, CancelReason::Other) => "Otro motivo",
        (Lang::En, CancelReason::Other) => "Another reason",
    }
}

/// Asks for stars about the other side of a finished trip.
pub fn rate_prompt(lang: Lang, rated: Role) -> &'static str {
    match (lang, rated) {
        (Lang::Es, Role::Driver) => "🏁 ¿Qué tal fue tu chofer? Tu nota es anónima y la otra persona no la verá.",
        (Lang::En, Role::Driver) => "🏁 How was your driver? Your rating is anonymous; they won't see it.",
        (Lang::Es, Role::Client) => "🏁 ¿Qué tal fue el cliente? Tu nota es anónima y la otra persona no la verá.",
        (Lang::En, Role::Client) => "🏁 How was the client? Your rating is anonymous; they won't see it.",
    }
}

/// To the client after "Share trip", to forward to someone they trust.
pub fn share_trip(
    lang: Lang,
    request: &RequestId,
    p: &Profile,
    (kind, vehicle): (VehicleKind, Option<&Vehicle>),
    time: &str,
) -> String {
    let (noun, label) = match (lang, kind) {
        (Lang::Es, VehicleKind::Moto) => ("moto", "Moto"),
        (Lang::Es, VehicleKind::Car) => ("carro", "Carro"),
        (Lang::Es, VehicleKind::Tricycle) => ("triciclo", "Triciclo"),
        (Lang::En, VehicleKind::Moto) => ("a motorbike", "Motorbike"),
        (Lang::En, VehicleKind::Car) => ("a car", "Car"),
        (Lang::En, VehicleKind::Tricycle) => ("a tricycle", "Tricycle"),
    };
    let alias = &p.alias;
    let mut text = match lang {
        Lang::Es => format!("{} Voy en {noun} con Asere Voy.\nChofer: {alias}", kind.emoji()),
        Lang::En => format!("{} I'm riding {noun} with Asere Voy.\nDriver: {alias}", kind.emoji()),
    };
    if let Some(v) = vehicle {
        text.push_str(&match lang {
            Lang::Es => format!("\n{label}: {}, {}\nChapa: {}", v.model, v.color, v.plate),
            Lang::En => format!("\n{label}: {}, {}\nPlate: {}", v.model, v.color, v.plate),
        });
    }
    text.push_str(&match lang {
        Lang::Es => format!("\nSalida: {time}\nViaje: {request}"),
        Lang::En => format!("\nDeparture: {time}\nTrip: {request}"),
    });
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::distance;

    #[test]
    fn both_trip_cards_say_each_thing_once_with_the_same_emojis() {
        let id = RequestId::parse("K7X2").unwrap();
        let bike = Vehicle {
            id: 1,
            driver: crate::domain::ids::UserId(7),
            kind: VehicleKind::Moto,
            plate: "P123".into(),
            model: "Suzuki".into(),
            color: "roja".into(),
            photo_file_id: None,
            status: crate::domain::vehicle::VehicleStatus::Approved,
            pending_plate: None,
            pending_photo_file_id: None,
        };
        let rep = Shown { stars: Some(4.8), trips: 23, days: 100 };
        let card = TripCard {
            request: &id,
            kind: VehicleKind::Moto,
            price: 3300,
            pay: PaymentMethod::Cash,
            distance: distance(Lang::Es, 3.5, 9.4, false),
            names: vec!["Pepe", "José Pérez Díaz"],
            reputation: &rep,
            phone: Some("+5351234567"),
            username: Some("pepe"),
        };
        let client = client_trip_card(Lang::Es, &card, VehicleKind::Moto, Some(&bike), false);
        assert_eq!(
            client,
            "🏍 Viaje K7X2 confirmado: tu chofer va hacia ti.\n📏 3.5 km · ⏱ unos 9 min\n👤 Pepe · José Pérez Díaz\n🏍 Suzuki, roja · Chapa P123\n📞 +5351234567\n💰 Al terminar le pagas 3300 CUP · 💵 Efectivo\n💬 Escríbele a @pepe para terminar de coordinar el viaje.\n⭐4.8 · 23 viajes\n📅 100 días usando Asere Voy"
        );
        assert_eq!(client.matches("@pepe").count(), 1);
        // The vehicle was removed after the trip was assigned: only the kind.
        let gone = client_trip_card(Lang::Es, &card, VehicleKind::Car, None, false);
        assert!(gone.contains("\n🚗 Carro\n📞"), "{gone}");

        let rep = Shown { stars: None, trips: 2, days: 3 };
        let card = TripCard { names: vec![], reputation: &rep, phone: None, username: Some("ana"), ..card };
        let driver = driver_trip_card(Lang::Es, &card, Some("frente a la bodega"));
        assert!(driver.starts_with("🏍 Viaje K7X2: es tuyo.\n📏 3.5 km · ⏱ unos 9 min\n👤 Cliente\n📝 Referencia: frente a la bodega\n💰 Al terminar cobras 3300 CUP"), "{driver}");
        assert!(!driver.contains("📞"), "{driver}");
    }
}
