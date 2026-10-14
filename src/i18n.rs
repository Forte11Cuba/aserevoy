//! User-facing texts in Spanish (default) and English. Every text is a `match`
//! over `Lang`, so the compiler rejects a message missing a translation.

use crate::domain::driver::{DriverStatus, PaymentMethods, Profile};
use crate::domain::effect::{Notice, Place};
use crate::domain::fare::Cup;
use crate::domain::ids::RequestId;
use crate::domain::request::{DomainError, PaymentMethod};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    #[default]
    Es,
    En,
}

impl Lang {
    /// Picks English only when Telegram says the user's app is in English.
    pub fn from_telegram(language_code: Option<&str>) -> Lang {
        match language_code {
            Some(code) if code.to_ascii_lowercase().starts_with("en") => Lang::En,
            _ => Lang::Es,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Lang::Es => "es",
            Lang::En => "en",
        }
    }

    pub fn parse(s: &str) -> Option<Lang> {
        match s {
            "es" => Some(Lang::Es),
            "en" => Some(Lang::En),
            _ => None,
        }
    }

    pub fn other(self) -> Lang {
        match self {
            Lang::Es => Lang::En,
            Lang::En => Lang::Es,
        }
    }
}

/// Labels of keyboard buttons. Reply-keyboard labels come back as plain text,
/// so `parse_button` recognizes them in either language.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    // Main menu (reply keyboard).
    RequestRide,
    ApplyDriver,
    MyProfile,
    InviteDriver,
    /// Shows the other language's name.
    SwitchLang,
    // Inside a conversation (reply keyboard).
    Cancel,
    Skip,
    SendMyLocation,
    SharePhone,
    // Inline buttons.
    ItsHere,
    Fix,
    Cash,
    Transfer,
    BothPayments,
    Accept,
    Complete,
    CancelTrip,
    Approve,
    Reject,
    EditProfile,
}

const REPLY_BUTTONS: [Button; 9] = [
    Button::RequestRide,
    Button::ApplyDriver,
    Button::MyProfile,
    Button::InviteDriver,
    Button::SwitchLang,
    Button::Cancel,
    Button::Skip,
    Button::SendMyLocation,
    Button::SharePhone,
];

pub fn button(lang: Lang, b: Button) -> &'static str {
    use Button::*;
    match (lang, b) {
        (Lang::Es, RequestRide) => "🛵 Pedir viaje",
        (Lang::En, RequestRide) => "🛵 Request a ride",
        (Lang::Es, ApplyDriver) => "🧑‍✈️ Aplicar a ser chofer",
        (Lang::En, ApplyDriver) => "🧑‍✈️ Apply to drive",
        (Lang::Es, MyProfile) => "🪪 Mi ficha",
        (Lang::En, MyProfile) => "🪪 My profile",
        (Lang::Es, InviteDriver) => "➕ Invitar chofer",
        (Lang::En, InviteDriver) => "➕ Invite a driver",
        (Lang::Es, SwitchLang) => "🌐 English",
        (Lang::En, SwitchLang) => "🌐 Español",
        (Lang::Es, Cancel) => "❌ Cancelar",
        (Lang::En, Cancel) => "❌ Cancel",
        (Lang::Es, Skip) => "⏭ Omitir",
        (Lang::En, Skip) => "⏭ Skip",
        (Lang::Es, SendMyLocation) => "📍 Enviar mi ubicación",
        (Lang::En, SendMyLocation) => "📍 Send my location",
        (Lang::Es, SharePhone) => "📱 Compartir mi número",
        (Lang::En, SharePhone) => "📱 Share my number",
        (Lang::Es, ItsHere) => "✅ Sí, es aquí",
        (Lang::En, ItsHere) => "✅ Yes, it's here",
        (Lang::Es, Fix) => "✏️ Corregir",
        (Lang::En, Fix) => "✏️ Fix it",
        (Lang::Es, Cash) => "💵 Efectivo",
        (Lang::En, Cash) => "💵 Cash",
        (Lang::Es, Transfer) => "📲 Transferencia",
        (Lang::En, Transfer) => "📲 Transfer",
        (Lang::Es, BothPayments) => "💵📲 Los dos",
        (Lang::En, BothPayments) => "💵📲 Both",
        (Lang::Es, Accept) => "✅ Aceptar",
        (Lang::En, Accept) => "✅ Accept",
        (Lang::Es, Complete) => "🏁 Completado",
        (Lang::En, Complete) => "🏁 Completed",
        (Lang::Es, CancelTrip) => "❌ Cancelar viaje",
        (Lang::En, CancelTrip) => "❌ Cancel trip",
        (Lang::Es, Approve) => "✅ Aprobar",
        (Lang::En, Approve) => "✅ Approve",
        (Lang::Es, Reject) => "❌ Rechazar",
        (Lang::En, Reject) => "❌ Reject",
        (Lang::Es, EditProfile) => "✏️ Editar ficha",
        (Lang::En, EditProfile) => "✏️ Edit profile",
    }
}

/// Recognizes a reply-keyboard label typed back by Telegram, in any language.
pub fn parse_button(text: &str) -> Option<Button> {
    REPLY_BUTTONS
        .into_iter()
        .find(|&b| [Lang::Es, Lang::En].into_iter().any(|lang| button(lang, b) == text))
}

/// Fixed texts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Text {
    Welcome,
    Menu,
    Cancelled,
    LangChanged,
    SomethingFailed,
    NotAllowed,
    // Ride request.
    AskPickup,
    ConfirmPickup,
    AskReference,
    AskDropoff,
    ConfirmDropoff,
    NeedLocation,
    RouteFailed,
    AmountNotNumber,
    AskPayment,
    AskPhone,
    InvalidPhone,
    PublishFailed,
    // Channel and trip.
    NotActiveDriver,
    AlreadyTaken,
    DriverBusy,
    AcceptedAlert,
    AlreadyClosed,
    // Driver profile.
    AskAlias,
    AskDriverPhone,
    AskPlate,
    AskBikeModel,
    AskBikeColor,
    AskPhoto,
    AskBikePhoto,
    AskPaymentMethods,
    InvalidField,
    NeedPhoto,
    ApplicationSent,
    ProfileUpdated,
    InviteInvalid,
    AlreadyDriver,
    DriverBlocked,
    Rejected,
    StaffApproved,
    StaffRejected,
    AlreadyDecided,
}

pub fn text(lang: Lang, t: Text) -> &'static str {
    use Text::*;
    match (lang, t) {
        (Lang::Es, Welcome) => {
            "¡Hola! Soy Asere Voy. Conectamos pasajeros y choferes: tú propones el precio.\n\nUsa el menú de abajo."
        }
        (Lang::En, Welcome) => {
            "Hi! I'm Asere Voy. We connect riders and drivers: you set the price.\n\nUse the menu below."
        }
        (Lang::Es, Menu) => "¿Qué quieres hacer?",
        (Lang::En, Menu) => "What would you like to do?",
        (Lang::Es, Cancelled) => "Listo, cancelado.",
        (Lang::En, Cancelled) => "OK, cancelled.",
        (Lang::Es, LangChanged) => "Idioma cambiado a español.",
        (Lang::En, LangChanged) => "Language changed to English.",
        (Lang::Es, SomethingFailed) => "Algo salió mal. Inténtalo de nuevo en un momento.",
        (Lang::En, SomethingFailed) => "Something went wrong. Please try again in a moment.",
        (Lang::Es, NotAllowed) => "No tienes permiso para hacer esto.",
        (Lang::En, NotAllowed) => "You are not allowed to do this.",

        (Lang::Es, AskPickup) => {
            "📍 ¿Dónde te recogemos? Pulsa «Enviar mi ubicación» o adjunta 📎 → Ubicación para marcar otro punto en el mapa."
        }
        (Lang::En, AskPickup) => {
            "📍 Where should we pick you up? Tap \"Send my location\" or attach 📎 → Location to mark another point on the map."
        }
        (Lang::Es, ConfirmPickup) => "¿Es aquí la recogida?",
        (Lang::En, ConfirmPickup) => "Is this the pickup point?",
        (Lang::Es, AskReference) => {
            "✍️ Escribe una referencia para el chofer (por ejemplo: frente a la bodega) o pulsa Omitir."
        }
        (Lang::En, AskReference) => {
            "✍️ Write a landmark for the driver (for example: in front of the corner store) or tap Skip."
        }
        (Lang::Es, AskDropoff) => "🏁 ¿A dónde vas? Adjunta 📎 → Ubicación y marca el destino en el mapa.",
        (Lang::En, AskDropoff) => "🏁 Where are you going? Attach 📎 → Location and mark the destination on the map.",
        (Lang::Es, ConfirmDropoff) => "¿Es aquí el destino?",
        (Lang::En, ConfirmDropoff) => "Is this the destination?",
        (Lang::Es, NeedLocation) => "Necesito un pin de ubicación 📍.",
        (Lang::En, NeedLocation) => "I need a location pin 📍.",
        (Lang::Es, RouteFailed) => "No pude calcular la ruta ahora mismo. Inténtalo de nuevo en unos minutos.",
        (Lang::En, RouteFailed) => "I couldn't work out the route right now. Please try again in a few minutes.",
        (Lang::Es, AmountNotNumber) => "Escribe solo el monto en CUP, por ejemplo 600.",
        (Lang::En, AmountNotNumber) => "Type just the amount in CUP, for example 600.",
        (Lang::Es, AskPayment) => "¿Cómo vas a pagar?",
        (Lang::En, AskPayment) => "How will you pay?",
        (Lang::Es, AskPhone) => {
            "📱 No tienes @usuario en Telegram, así que el chofer no podrá escribirte. Si quieres, comparte o escribe un teléfono para que te llame (es opcional)."
        }
        (Lang::En, AskPhone) => {
            "📱 You have no Telegram @username, so the driver won't be able to message you. If you like, share or type a phone number so they can call you (optional)."
        }
        (Lang::Es, InvalidPhone) => "Ese número no parece válido. Escríbelo de nuevo o pulsa Omitir.",
        (Lang::En, InvalidPhone) => "That number doesn't look valid. Type it again or tap Skip.",
        (Lang::Es, PublishFailed) => "No pude publicar tu solicitud. Inténtalo de nuevo.",
        (Lang::En, PublishFailed) => "I couldn't publish your request. Please try again.",

        (Lang::Es, NotActiveDriver) => "Solo los choferes activos pueden aceptar viajes.",
        (Lang::En, NotActiveDriver) => "Only active drivers can accept trips.",
        (Lang::Es, AlreadyTaken) => "Ya fue tomado.",
        (Lang::En, AlreadyTaken) => "Already taken.",
        (Lang::Es, DriverBusy) => "Ya tienes un viaje asignado. Complétalo o cancélalo primero.",
        (Lang::En, DriverBusy) => "You already have an assigned trip. Complete or cancel it first.",
        (Lang::Es, AcceptedAlert) => "¡Es tuyo! Te escribí por privado.",
        (Lang::En, AcceptedAlert) => "It's yours! I've messaged you privately.",
        (Lang::Es, AlreadyClosed) => "Este viaje ya se cerró.",
        (Lang::En, AlreadyClosed) => "This trip is already closed.",

        (Lang::Es, AskAlias) => "🧑 ¿Cómo quieres que te vean los clientes? Escribe tu alias.",
        (Lang::En, AskAlias) => "🧑 How should clients see you? Type your alias.",
        (Lang::Es, AskDriverPhone) => "📞 Tu celular, para que los clientes te llamen. Compártelo con el botón o escríbelo.",
        (Lang::En, AskDriverPhone) => "📞 Your mobile number, so clients can call you. Share it with the button or type it.",
        (Lang::Es, AskPlate) => "🔢 Número de chapa de la moto:",
        (Lang::En, AskPlate) => "🔢 Motorbike plate number:",
        (Lang::Es, AskBikeModel) => "🏍 Marca y modelo de la moto:",
        (Lang::En, AskBikeModel) => "🏍 Motorbike make and model:",
        (Lang::Es, AskBikeColor) => "🎨 Color de la moto:",
        (Lang::En, AskBikeColor) => "🎨 Motorbike colour:",
        (Lang::Es, AskPhoto) => "🤳 Envía una foto tuya. Los clientes la verán cuando aceptes su viaje.",
        (Lang::En, AskPhoto) => "🤳 Send a photo of yourself. Clients will see it when you accept their trip.",
        (Lang::Es, AskBikePhoto) => "📷 Envía una foto de la moto, o pulsa Omitir.",
        (Lang::En, AskBikePhoto) => "📷 Send a photo of the motorbike, or tap Skip.",
        (Lang::Es, AskPaymentMethods) => "💳 ¿Qué pagos aceptas?",
        (Lang::En, AskPaymentMethods) => "💳 Which payments do you take?",
        (Lang::Es, InvalidField) => "Respuesta vacía o demasiado larga. Inténtalo de nuevo.",
        (Lang::En, InvalidField) => "Empty or too long. Please try again.",
        (Lang::Es, NeedPhoto) => "Necesito una foto.",
        (Lang::En, NeedPhoto) => "I need a photo.",
        (Lang::Es, ApplicationSent) => "📨 Ficha enviada. Un admin la revisará y te avisaré.",
        (Lang::En, ApplicationSent) => "📨 Profile sent. An admin will review it and I'll let you know.",
        (Lang::Es, ProfileUpdated) => "✅ Ficha actualizada.",
        (Lang::En, ProfileUpdated) => "✅ Profile updated.",
        (Lang::Es, InviteInvalid) => "Esta invitación no es válida, ya se usó o venció. Pide otra a un admin.",
        (Lang::En, InviteInvalid) => "This invitation is invalid, used or expired. Ask an admin for a new one.",
        (Lang::Es, AlreadyDriver) => "Ya tienes una ficha de chofer. Puedes verla en 🪪 Mi ficha.",
        (Lang::En, AlreadyDriver) => "You already have a driver profile. See it in 🪪 My profile.",
        (Lang::Es, DriverBlocked) => "Tu cuenta de chofer está bloqueada.",
        (Lang::En, DriverBlocked) => "Your driver account is blocked.",
        (Lang::Es, Rejected) => "Tu solicitud para ser chofer no fue aprobada.",
        (Lang::En, Rejected) => "Your driver application was not approved.",
        (Lang::Es, StaffApproved) => "✅ Aprobado.",
        (Lang::En, StaffApproved) => "✅ Approved.",
        (Lang::Es, StaffRejected) => "❌ Rechazado.",
        (Lang::En, StaffRejected) => "❌ Rejected.",
        (Lang::Es, AlreadyDecided) => "Otro admin ya decidió sobre esta solicitud.",
        (Lang::En, AlreadyDecided) => "Another admin already decided on this application.",
    }
}

pub fn payment(lang: Lang, p: PaymentMethod) -> &'static str {
    match p {
        PaymentMethod::Cash => button(lang, Button::Cash),
        PaymentMethod::Transfer => button(lang, Button::Transfer),
    }
}

fn payment_methods(lang: Lang, m: PaymentMethods) -> &'static str {
    match (m.cash, m.transfer) {
        (true, true) => button(lang, Button::BothPayments),
        (false, true) => button(lang, Button::Transfer),
        _ => button(lang, Button::Cash),
    }
}

pub fn already_active(lang: Lang, request: &RequestId) -> String {
    match lang {
        Lang::Es => format!("Ya tienes la solicitud {request} en curso."),
        Lang::En => format!("You already have request {request} in progress."),
    }
}

pub fn price_prompt(lang: Lang, km: f64, minutes: f64, suggested: Cup, minimum: Cup) -> String {
    let minutes = minutes.round().max(1.0);
    match lang {
        Lang::Es => format!(
            "📏 {km:.1} km (unos {minutes} min)\n💰 Precio sugerido: {suggested} CUP\n\nElige tu oferta o escribe un monto (mínimo {minimum} CUP)."
        ),
        Lang::En => format!(
            "📏 {km:.1} km (about {minutes} min)\n💰 Suggested price: {suggested} CUP\n\nPick your offer or type an amount (minimum {minimum} CUP)."
        ),
    }
}

/// The offer as drivers see it in the channel: approximate zones (the
/// municipality of each pin), never the exact location or the contact.
pub fn channel_post(
    lang: Lang,
    request: &RequestId,
    km: f64,
    price: Cup,
    pay: PaymentMethod,
    from: Option<&str>,
    to: Option<&str>,
) -> String {
    let pay = payment(lang, pay);
    let mut post = match lang {
        Lang::Es => format!("🛵 Solicitud {request}\n"),
        Lang::En => format!("🛵 Request {request}\n"),
    };
    if from.is_some() || to.is_some() {
        post.push_str(&format!("📍 {} → {}\n", from.unwrap_or("?"), to.unwrap_or("?")));
    }
    post.push_str(&format!("📏 {km:.1} km\n💰 {price} CUP · {pay}"));
    post
}

/// Title of a pin sent to the driver.
pub fn place(lang: Lang, place: Place) -> &'static str {
    match (lang, place) {
        (Lang::Es, Place::Pickup) => "📍 Recogida",
        (Lang::En, Place::Pickup) => "📍 Pickup",
        (Lang::Es, Place::Dropoff) => "🏁 Destino",
        (Lang::En, Place::Dropoff) => "🏁 Destination",
    }
}

pub fn channel_taken(lang: Lang, request: &RequestId) -> String {
    match lang {
        Lang::Es => format!("✅ Solicitud {request}: tomada."),
        Lang::En => format!("✅ Request {request}: taken."),
    }
}

/// To the client when a driver accepts: who is coming and how to reach them.
pub fn driver_assigned(lang: Lang, request: &RequestId, driver: &Profile) -> String {
    let p = driver;
    match lang {
        Lang::Es => format!(
            "✅ ¡Un chofer aceptó tu viaje {request}!\n\n🧑 {}\n🏍 {}, {}\n🔢 Chapa: {}\n📞 {}",
            p.alias, p.bike_model, p.bike_color, p.plate, p.phone
        ),
        Lang::En => format!(
            "✅ A driver accepted your trip {request}!\n\n🧑 {}\n🏍 {}, {}\n🔢 Plate: {}\n📞 {}",
            p.alias, p.bike_model, p.bike_color, p.plate, p.phone
        ),
    }
}

/// To the driver who won: the client's contact and the details of the trip.
pub fn trip_assigned(
    lang: Lang,
    request: &RequestId,
    price: Cup,
    pay: PaymentMethod,
    reference: Option<&str>,
    username: Option<&str>,
    phone: Option<&str>,
) -> String {
    let pay = payment(lang, pay);
    let mut lines = match lang {
        Lang::Es => vec![format!("🛵 El viaje {request} es tuyo.\n💰 {price} CUP · {pay}")],
        Lang::En => vec![format!("🛵 Trip {request} is yours.\n💰 {price} CUP · {pay}")],
    };
    if let Some(r) = reference {
        lines.push(match lang {
            Lang::Es => format!("📝 Referencia: {r}"),
            Lang::En => format!("📝 Landmark: {r}"),
        });
    }
    if let Some(u) = username {
        lines.push(format!("👤 @{u}"));
    }
    if let Some(p) = phone {
        lines.push(format!("📞 {p}"));
    }
    if username.is_none() && phone.is_none() {
        lines.push(
            match lang {
                Lang::Es => "El cliente no dejó contacto: te esperará en el punto de recogida.",
                Lang::En => "The client left no contact: they'll wait at the pickup point.",
            }
            .into(),
        );
    }
    lines.push(
        match lang {
            Lang::Es => "Abajo tienes la recogida y el destino.",
            Lang::En => "The pickup and destination are below.",
        }
        .into(),
    );
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

pub fn profile_card(lang: Lang, p: &Profile, status: Option<DriverStatus>) -> String {
    let methods = payment_methods(lang, p.payment_methods);
    let status = status.map(|s| match (lang, s) {
        (Lang::Es, DriverStatus::Pending) => "⏳ Pendiente de aprobación",
        (Lang::En, DriverStatus::Pending) => "⏳ Waiting for approval",
        (Lang::Es, DriverStatus::Active) => "🟢 Activo",
        (Lang::En, DriverStatus::Active) => "🟢 Active",
        (Lang::Es, DriverStatus::Blocked) => "⛔ Bloqueado",
        (Lang::En, DriverStatus::Blocked) => "⛔ Blocked",
    });
    let mut card = match lang {
        Lang::Es => format!(
            "🧑 {}\n📞 {}\n🏍 {}, {}\n🔢 Chapa: {}\n💳 {methods}",
            p.alias, p.phone, p.bike_model, p.bike_color, p.plate
        ),
        Lang::En => format!(
            "🧑 {}\n📞 {}\n🏍 {}, {}\n🔢 Plate: {}\n💳 {methods}",
            p.alias, p.phone, p.bike_model, p.bike_color, p.plate
        ),
    };
    if let Some(status) = status {
        card.push('\n');
        card.push_str(status);
    }
    card
}

/// To admins: a new driver application to approve or reject.
pub fn application(lang: Lang, p: &Profile, invited: bool) -> String {
    let card = profile_card(lang, p, None);
    match (lang, invited) {
        (Lang::Es, true) => format!("🆕 Solicitud de chofer (por invitación)\n\n{card}"),
        (Lang::Es, false) => format!("🆕 Solicitud de chofer\n\n{card}"),
        (Lang::En, true) => format!("🆕 Driver application (invited)\n\n{card}"),
        (Lang::En, false) => format!("🆕 Driver application\n\n{card}"),
    }
}

pub fn approved(lang: Lang, channel_link: &str) -> String {
    match lang {
        Lang::Es => format!(
            "🎉 ¡Ya eres chofer de Asere Voy!\n\nÚnete al canal de ofertas: {channel_link}\nCuando pidas unirte, te apruebo enseguida."
        ),
        Lang::En => format!(
            "🎉 You're now an Asere Voy driver!\n\nJoin the offers channel: {channel_link}\nWhen you ask to join, I'll approve you right away."
        ),
    }
}

pub fn invite_created(lang: Lang, link: &str) -> String {
    match lang {
        Lang::Es => format!("🔗 Enlace de invitación para un chofer (un solo uso, vence en 48 h):\n{link}"),
        Lang::En => format!("🔗 Driver invitation link (single use, expires in 48 h):\n{link}"),
    }
}

/// Base text of a notice. The bot appends details such as the driver's card
/// or the client's contact when it renders `DriverAssigned` / `TripAssigned`.
pub fn notice(lang: Lang, notice: &Notice) -> String {
    use Notice::*;
    match (lang, notice) {
        (Lang::Es, Published { request }) => {
            format!("📣 Tu solicitud {request} ya está publicada. Te aviso cuando un chofer la acepte.")
        }
        (Lang::En, Published { request }) => {
            format!("📣 Your request {request} is live. I'll let you know when a driver accepts it.")
        }
        (Lang::Es, DriverAssigned { request, .. }) => format!("✅ ¡Un chofer aceptó tu viaje {request}!"),
        (Lang::En, DriverAssigned { request, .. }) => format!("✅ A driver accepted your trip {request}!"),
        (Lang::Es, TripAssigned { request, .. }) => format!("🛵 El viaje {request} es tuyo."),
        (Lang::En, TripAssigned { request, .. }) => format!("🛵 Trip {request} is yours."),
        (Lang::Es, Completed { request }) => format!("🏁 Viaje {request} completado. ¡Gracias!"),
        (Lang::En, Completed { request }) => format!("🏁 Trip {request} completed. Thanks!"),
        (Lang::Es, CancelledByClient { request }) => format!("❌ El cliente canceló el viaje {request}."),
        (Lang::En, CancelledByClient { request }) => format!("❌ The client cancelled trip {request}."),
        (Lang::Es, CancelledByDriver { request }) => {
            format!("❌ El chofer canceló el viaje {request}. Puedes pedir otro con 🛵 Pedir viaje.")
        }
        (Lang::En, CancelledByDriver { request }) => {
            format!("❌ The driver cancelled trip {request}. You can request another with 🛵 Request a ride.")
        }
        (Lang::Es, Expired { request }) => {
            format!("⌛ Nadie tomó la solicitud {request} a tiempo. Puedes pedir otro viaje con 🛵 Pedir viaje.")
        }
        (Lang::En, Expired { request }) => {
            format!("⌛ Nobody took request {request} in time. You can try again with 🛵 Request a ride.")
        }
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
        (Lang::Es, NotOpen) => "Esta solicitud ya no está disponible.".into(),
        (Lang::En, NotOpen) => "This request is no longer available.".into(),
        (Lang::Es, NotAssigned) => "Este viaje no está asignado.".into(),
        (Lang::En, NotAssigned) => "This trip is not assigned.".into(),
        (Lang::Es, OwnRequest) => "No puedes aceptar tu propia solicitud.".into(),
        (Lang::En, OwnRequest) => "You can't accept your own request.".into(),
        (Lang::Es, NotParticipant) => "Solo el cliente o el chofer del viaje pueden hacer esto.".into(),
        (Lang::En, NotParticipant) => "Only the trip's client or driver can do this.".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spanish_is_the_default() {
        assert_eq!(Lang::from_telegram(None), Lang::Es);
        assert_eq!(Lang::from_telegram(Some("es")), Lang::Es);
        assert_eq!(Lang::from_telegram(Some("pt-br")), Lang::Es);
        assert_eq!(Lang::from_telegram(Some("en")), Lang::En);
        assert_eq!(Lang::from_telegram(Some("EN-us")), Lang::En);
    }

    #[test]
    fn reply_buttons_are_recognized_in_both_languages() {
        for b in REPLY_BUTTONS {
            assert_eq!(parse_button(button(Lang::Es, b)), Some(b));
            assert_eq!(parse_button(button(Lang::En, b)), Some(b));
        }
        assert_eq!(parse_button("hola"), None);
    }

    #[test]
    fn lang_round_trips_through_its_code() {
        for lang in [Lang::Es, Lang::En] {
            assert_eq!(Lang::parse(lang.as_str()), Some(lang));
        }
    }
}
