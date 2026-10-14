//! User-facing texts in Spanish (default) and English. Every text is a `match`
//! over `Lang`, so the compiler rejects a message missing a translation.

use crate::db::metrics::Day;
use crate::domain::driver::{DriverStatus, PaymentMethods, Profile};
use crate::domain::effect::{Notice, Place};
use crate::domain::fare::{Cup, Fares};
use crate::domain::ids::RequestId;
use crate::domain::reputation::{Role, Shown, Tag};
use crate::domain::request::{CancelReason, DomainError, PaymentMethod};

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
    Confirm,
    RaisePrice,
    KeepWaiting,
    Counteroffer,
    Republish,
    Withdraw,
    NoneOfThese,
    Support,
    Admins,
    OnMyWay,
    Arrived,
    ShareTrip,
    InviteAdmin,
    RemoveAdmin,
    Block24h,
    Block,
    Unblock,
    RemoveDriver,
    UsernameSet,
    WriteSupport,
    AcceptTerms,
    RatingDone,
    Back,
}

const REPLY_BUTTONS: [Button; 11] = [
    Button::RequestRide,
    Button::ApplyDriver,
    Button::MyProfile,
    Button::InviteDriver,
    Button::Support,
    Button::Admins,
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
        (Lang::Es, Confirm) => "✅ Confirmar",
        (Lang::En, Confirm) => "✅ Confirm",
        (Lang::Es, RaisePrice) => "⬆️ Subir precio",
        (Lang::En, RaisePrice) => "⬆️ Raise price",
        (Lang::Es, KeepWaiting) => "✅ Sí, sigo esperando",
        (Lang::En, KeepWaiting) => "✅ Yes, still waiting",
        (Lang::Es, Counteroffer) => "💬 Contraofertar",
        (Lang::En, Counteroffer) => "💬 Counteroffer",
        (Lang::Es, Republish) => "🔁 Publicar de nuevo",
        (Lang::En, Republish) => "🔁 Publish again",
        (Lang::Es, Withdraw) => "🚪 Retirarme",
        (Lang::En, Withdraw) => "🚪 Withdraw",
        (Lang::Es, NoneOfThese) => "❌ Ninguna de estas",
        (Lang::En, NoneOfThese) => "❌ None of these",
        (Lang::Es, Support) => "💬 Soporte",
        (Lang::En, Support) => "💬 Support",
        (Lang::Es, Admins) => "👮 Admins",
        (Lang::En, Admins) => "👮 Admins",
        (Lang::Es, OnMyWay) => "🚶 Ya voy",
        (Lang::En, OnMyWay) => "🚶 On my way",
        (Lang::Es, Arrived) => "📍 Llegué",
        (Lang::En, Arrived) => "📍 I'm here",
        (Lang::Es, ShareTrip) => "📤 Compartir viaje",
        (Lang::En, ShareTrip) => "📤 Share trip",
        (Lang::Es, InviteAdmin) => "➕ Invitar admin",
        (Lang::En, InviteAdmin) => "➕ Invite an admin",
        (Lang::Es, RemoveAdmin) => "❌ Quitar",
        (Lang::En, RemoveAdmin) => "❌ Remove",
        (Lang::Es, Block24h) => "⛔ Bloquear 24 h",
        (Lang::En, Block24h) => "⛔ Block 24 h",
        (Lang::Es, Block) => "⛔ Bloquear",
        (Lang::En, Block) => "⛔ Block",
        (Lang::Es, Unblock) => "✅ Desbloquear",
        (Lang::En, Unblock) => "✅ Unblock",
        (Lang::Es, RemoveDriver) => "🚫 Quitar chofer",
        (Lang::En, RemoveDriver) => "🚫 Remove driver",
        (Lang::Es, UsernameSet) => "✅ Ya lo puse",
        (Lang::En, UsernameSet) => "✅ I've set it",
        (Lang::Es, WriteSupport) => "💬 Escribir a soporte",
        (Lang::En, WriteSupport) => "💬 Message support",
        (Lang::Es, AcceptTerms) => "✅ Acepto",
        (Lang::En, AcceptTerms) => "✅ I accept",
        (Lang::Es, RatingDone) => "✅ Listo",
        (Lang::En, RatingDone) => "✅ Done",
        (Lang::Es, Back) => "↩️ Volver",
        (Lang::En, Back) => "↩️ Back",
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
    ChooseAddress,
    AddressNotFound,
    AskReparto,
    ChooseReparto,
    RouteFailed,
    AmountNotNumber,
    AskPayment,
    InvalidPhone,
    PublishFailed,
    // Channel and trip.
    NotActiveDriver,
    AlreadyTaken,
    DriverBusy,
    AcceptedAlert,
    AlreadyClosed,
    KeptWaiting,
    Withdrawn,
    FareUsage,
    ChooseCancelReason,
    SupportUnavailable,
    UnknownCode,
    NeedUsername,
    StillNoUsername,
    TermsAccepted,
    TermsFirst,
    RatingThanks,
    RatingTags,
    AdminWelcome,
    AdminRemovedYou,
    NoAdmins,
    StaffBlocked,
    StaffUnblocked,
    DriverRemoved,
    YouWereRemoved,
    YouWereUnblocked,
    CounterofferSent,
    NoActiveRequest,
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
            "📍 ¿Dónde te recogemos? Pulsa «Enviar mi ubicación», adjunta 📎 → Ubicación para marcar otro punto, o escribe la dirección (por ejemplo: 23 e/ L y M, Vedado)."
        }
        (Lang::En, AskPickup) => {
            "📍 Where should we pick you up? Tap \"Send my location\", attach 📎 → Location to mark another point, or type the address (for example: 23 e/ L y M, Vedado)."
        }
        (Lang::Es, ConfirmPickup) => "¿Es aquí la recogida?",
        (Lang::En, ConfirmPickup) => "Is this the pickup point?",
        (Lang::Es, AskReference) => {
            "✍️ Escribe una referencia para el chofer (por ejemplo: frente a la bodega) o pulsa Omitir."
        }
        (Lang::En, AskReference) => {
            "✍️ Write a landmark for the driver (for example: in front of the corner store) or tap Skip."
        }
        (Lang::Es, AskDropoff) => {
            "🏁 ¿A dónde vas? Adjunta 📎 → Ubicación y marca el destino, o escribe la dirección o un lugar conocido (por ejemplo: Hospital Ameijeiras)."
        }
        (Lang::En, AskDropoff) => {
            "🏁 Where are you going? Attach 📎 → Location and mark the destination, or type the address or a known place (for example: Hospital Ameijeiras)."
        }
        (Lang::Es, ConfirmDropoff) => "¿Es aquí el destino?",
        (Lang::En, ConfirmDropoff) => "Is this the destination?",
        (Lang::Es, ChooseAddress) => "Encontré varios lugares con ese nombre. ¿Cuál es?",
        (Lang::En, ChooseAddress) => "I found several places by that name. Which one is it?",
        (Lang::Es, AddressNotFound) => {
            "No encontré esa dirección en el mapa. Envía tu ubicación 📍, o elige tu municipio y le daré tu descripción al chofer cuando lo confirmes (el precio se calcula aproximado)."
        }
        (Lang::En, AddressNotFound) => {
            "I couldn't find that address on the map. Send your location 📍, or pick your municipality and I'll give your description to the driver once you confirm them (the price is estimated)."
        }
        (Lang::Es, AskReparto) => "¿En qué reparto o zona? Escríbelo, por ejemplo: Alamar zona 6. También puedes enviar tu ubicación 📍.",
        (Lang::En, AskReparto) => "Which neighborhood (reparto) or zone? Type it, for example: Alamar zona 6. You can also send your location 📍.",
        (Lang::Es, ChooseReparto) => "¿En qué reparto?",
        (Lang::En, ChooseReparto) => "Which neighborhood?",
        (Lang::Es, NeedLocation) => "Necesito un pin de ubicación 📍 o una dirección escrita.",
        (Lang::En, NeedLocation) => "I need a location pin 📍 or a written address.",
        (Lang::Es, RouteFailed) => "No pude calcular la ruta ahora mismo. Inténtalo de nuevo en unos minutos.",
        (Lang::En, RouteFailed) => "I couldn't work out the route right now. Please try again in a few minutes.",
        (Lang::Es, AmountNotNumber) => "Escribe solo el monto en CUP, por ejemplo 600.",
        (Lang::En, AmountNotNumber) => "Type just the amount in CUP, for example 600.",
        (Lang::Es, AskPayment) => "¿Cómo vas a pagar?",
        (Lang::En, AskPayment) => "How will you pay?",
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
        (Lang::Es, AcceptedAlert) => "¡Reservado! El cliente tiene 5 min para confirmarte. Te escribí por privado.",
        (Lang::En, AcceptedAlert) => "Reserved! The client has 5 min to confirm you. I've messaged you privately.",
        (Lang::Es, AlreadyClosed) => "Este viaje ya se cerró.",
        (Lang::En, AlreadyClosed) => "This trip is already closed.",
        (Lang::Es, KeptWaiting) => "👍 Perfecto, tu solicitud sigue publicada.",
        (Lang::En, KeptWaiting) => "👍 Great, your request stays published.",
        (Lang::Es, FareUsage) => "Uso: /tarifa base 100, /tarifa km 150 o /tarifa minimo 80 (porcentaje del precio sugerido).",
        (Lang::En, FareUsage) => "Usage: /tarifa base 100, /tarifa km 150 or /tarifa minimo 80 (percentage of the suggested price).",
        (Lang::Es, ChooseCancelReason) => "¿Por qué cancelas el viaje?",
        (Lang::En, ChooseCancelReason) => "Why are you cancelling the trip?",
        (Lang::Es, SupportUnavailable) => "El soporte no está disponible ahora mismo. Inténtalo más tarde.",
        (Lang::En, SupportUnavailable) => "Support isn't available right now. Please try again later.",
        (Lang::Es, UnknownCode) => "No encontré ese código, o ya venció.",
        (Lang::En, UnknownCode) => "I couldn't find that code, or it expired.",
        (Lang::Es, NeedUsername) => {
            "👤 Para usar Asere Voy necesitas un nombre de usuario de Telegram (@usuario). Así el chofer y el cliente pueden escribirse o llamarse por Telegram sin compartir el teléfono.\n\nCómo ponerlo: Ajustes → Nombre de usuario (o Editar perfil → Nombre de usuario), elige uno y guarda. Luego pulsa «Ya lo puse»."
        }
        (Lang::En, NeedUsername) => {
            "👤 To use Asere Voy you need a Telegram username (@username), so drivers and clients can message or call each other on Telegram without sharing phone numbers.\n\nHow: Settings → Username (or Edit profile → Username), pick one and save. Then tap \"I've set it\"."
        }
        (Lang::Es, StillNoUsername) => "Todavía no veo tu @usuario. Ponlo en Ajustes → Nombre de usuario y vuelve a intentarlo.",
        (Lang::En, StillNoUsername) => "I still don't see your @username. Set it in Settings → Username and try again.",
        (Lang::Es, TermsAccepted) => "¡Gracias! Ya puedes usar Asere Voy.",
        (Lang::En, TermsAccepted) => "Thanks! You can now use Asere Voy.",
        (Lang::Es, TermsFirst) => "Abre el bot y acepta los términos primero.",
        (Lang::En, TermsFirst) => "Open the bot and accept the terms first.",
        (Lang::Es, RatingThanks) => "⭐ ¡Gracias por calificar! La otra persona no verá tu nota.",
        (Lang::En, RatingThanks) => "⭐ Thanks for rating! The other person won't see your rating.",
        (Lang::Es, RatingTags) => "Si quieres, añade etiquetas y pulsa Listo.",
        (Lang::En, RatingTags) => "If you like, add tags and tap Done.",
        (Lang::Es, AdminWelcome) => "👮 Ya eres admin de Asere Voy.",
        (Lang::En, AdminWelcome) => "👮 You're now an Asere Voy admin.",
        (Lang::Es, AdminRemovedYou) => "Ya no eres admin de Asere Voy.",
        (Lang::En, AdminRemovedYou) => "You're no longer an Asere Voy admin.",
        (Lang::Es, NoAdmins) => "No hay admins nombrados todavía.",
        (Lang::En, NoAdmins) => "No admins appointed yet.",
        (Lang::Es, StaffBlocked) => "⛔ Bloqueado.",
        (Lang::En, StaffBlocked) => "⛔ Blocked.",
        (Lang::Es, StaffUnblocked) => "✅ Desbloqueado.",
        (Lang::En, StaffUnblocked) => "✅ Unblocked.",
        (Lang::Es, DriverRemoved) => "🚫 Chofer quitado.",
        (Lang::En, DriverRemoved) => "🚫 Driver removed.",
        (Lang::Es, YouWereRemoved) => "Ya no eres chofer de Asere Voy. Si crees que es un error, escribe a 💬 Soporte.",
        (Lang::En, YouWereRemoved) => "You're no longer an Asere Voy driver. If you think it's a mistake, write to 💬 Support.",
        (Lang::Es, YouWereUnblocked) => "✅ Tu cuenta fue desbloqueada.",
        (Lang::En, YouWereUnblocked) => "✅ Your account was unblocked.",
        (Lang::Es, Withdrawn) => "🚪 Te retiraste. La solicitud volvió al canal.",
        (Lang::En, Withdrawn) => "🚪 You withdrew. The request is back in the channel.",
        (Lang::Es, CounterofferSent) => "💬 Contraoferta enviada. Vale 5 min; te aviso si el cliente la acepta.",
        (Lang::En, CounterofferSent) => "💬 Counteroffer sent. It's valid for 5 min; I'll tell you if the client accepts.",
        (Lang::Es, NoActiveRequest) => "No tienes ninguna solicitud abierta.",
        (Lang::En, NoActiveRequest) => "You have no open request.",

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

/// The offer being adjusted, with the trip, the suggested price and the minimum.
pub fn price_prompt(
    lang: Lang,
    km: f64,
    minutes: f64,
    suggested: Cup,
    minimum: Cup,
    offer: Cup,
    approx: bool,
) -> String {
    let about = if approx { "≈" } else { "" };
    // The whole offer over the km (the base fare included): easy to compare.
    let per_km = if km > 0.0 { (offer as f64 / km).round() as Cup } else { offer };
    let time = match (lang, minutes > 0.0) {
        (Lang::Es, true) => format!(" (unos {} min)", minutes.round().max(1.0)),
        (Lang::En, true) => format!(" (about {} min)", minutes.round().max(1.0)),
        (_, false) => String::new(),
    };
    match lang {
        Lang::Es => format!(
            "📏 {about}{km:.1} km{time}\n💰 Tu oferta: {offer} CUP (≈{per_km} CUP por km)\n(sugerido {suggested} CUP · mínimo {minimum} CUP)\n\nAjústala con los botones o escribe un monto."
        ),
        Lang::En => format!(
            "📏 {about}{km:.1} km{time}\n💰 Your offer: {offer} CUP (≈{per_km} CUP per km)\n(suggested {suggested} CUP · minimum {minimum} CUP)\n\nAdjust it with the buttons or type an amount."
        ),
    }
}

pub fn offer_button(lang: Lang, offer: Cup) -> String {
    match lang {
        Lang::Es => format!("✅ Ofrecer {offer} CUP"),
        Lang::En => format!("✅ Offer {offer} CUP"),
    }
}

pub fn reparto_not_found(lang: Lang, municipality: &str) -> String {
    match lang {
        Lang::Es => format!("No encontré ese reparto en {municipality}. Escríbelo de otra forma o envía tu ubicación 📍."),
        Lang::En => format!("I couldn't find that neighborhood in {municipality}. Type it another way or send your location 📍."),
    }
}

/// "Is it here?" under the place found for a written address.
pub fn confirm_written(lang: Lang, pickup: bool, label: &str) -> String {
    match (lang, pickup) {
        (Lang::Es, true) => format!("📍 {label}\n¿Es aquí la recogida?"),
        (Lang::En, true) => format!("📍 {label}\nIs this the pickup point?"),
        (Lang::Es, false) => format!("🏁 {label}\n¿Es aquí el destino?"),
        (Lang::En, false) => format!("🏁 {label}\nIs this the destination?"),
    }
}

/// To the driver, for a point without an exact location.
pub fn approx_point(lang: Lang, place: Place, note: &str, municipality: Option<&str>) -> String {
    let municipality = municipality.unwrap_or("?");
    match (lang, place) {
        (Lang::Es, Place::Pickup) => format!("📍 Recogida (sin ubicación exacta, en {municipality}):\n{note}"),
        (Lang::En, Place::Pickup) => format!("📍 Pickup (no exact location, in {municipality}):\n{note}"),
        (Lang::Es, Place::Dropoff) => format!("🏁 Destino (sin ubicación exacta, en {municipality}):\n{note}"),
        (Lang::En, Place::Dropoff) => format!("🏁 Destination (no exact location, in {municipality}):\n{note}"),
    }
}

/// What the channel shows of a request.
pub struct Post<'a> {
    pub request: &'a RequestId,
    pub km: f64,
    pub price: Cup,
    pub payment: PaymentMethod,
    /// Municipalities of pickup and destination, when known.
    pub from: Option<&'a str>,
    pub to: Option<&'a str>,
    /// Nearest corners, for exact points ("23 y L").
    pub from_corner: Option<String>,
    pub to_corner: Option<String>,
    /// A point is a written description: distance and price are estimates.
    pub approx: bool,
    /// The client's reputation.
    pub client: Option<Shown>,
}

/// The offer as drivers see it in the channel: approximate zones (the
/// municipality of each pin), never the exact location or the contact.
pub fn channel_post(lang: Lang, p: &Post) -> String {
    let (request, km, price) = (p.request, p.km, p.price);
    let pay = payment(lang, p.payment);
    let mut post = match lang {
        Lang::Es => format!("🛵 Solicitud {request}\n"),
        Lang::En => format!("🛵 Request {request}\n"),
    };
    if p.from.is_some() || p.to.is_some() {
        let zone = |m: Option<&str>, corner: &Option<String>| match corner {
            Some(c) => format!("{} ({c})", m.unwrap_or("?")),
            None => m.unwrap_or("?").to_owned(),
        };
        post.push_str(&format!("📍 {} → {}\n", zone(p.from, &p.from_corner), zone(p.to, &p.to_corner)));
    }
    let about = if p.approx { "≈" } else { "" };
    post.push_str(&format!("📏 {about}{km:.1} km\n💰 {price} CUP · {pay}"));
    if p.approx {
        post.push_str(match lang {
            Lang::Es => "\n📝 Dirección escrita, sin ubicación exacta",
            Lang::En => "\n📝 Written address, no exact location",
        });
    }
    if let Some(client) = &p.client {
        post.push_str(&format!("\n👤 {}", reputation(lang, client, true)));
    }
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
pub fn driver_assigned(lang: Lang, request: &RequestId, p: &Profile, driver: &Shown, username: Option<&str>) -> String {
    let rep = reputation(lang, driver, false);
    let contact = username.map(|u| format!("\n💬 @{u}")).unwrap_or_default();
    let card = match lang {
        Lang::Es => format!(
            "✅ ¡Un chofer aceptó tu viaje {request}!\n\n🧑 {} · {rep}\n🏍 {}, {}\n🔢 Chapa: {}\n📞 {}",
            p.alias, p.bike_model, p.bike_color, p.plate, p.phone
        ),
        Lang::En => format!(
            "✅ A driver accepted your trip {request}!\n\n🧑 {} · {rep}\n🏍 {}, {}\n🔢 Plate: {}\n📞 {}",
            p.alias, p.bike_model, p.bike_color, p.plate, p.phone
        ),
    };
    card + &contact
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
        (Lang::Es, PriceRaised { request }) => format!("⬆️ Precio actualizado en la solicitud {request}."),
        (Lang::En, PriceRaised { request }) => format!("⬆️ Price updated on request {request}."),
        (Lang::Es, StillWaiting { request }) => format!(
            "⏳ Tu solicitud {request} lleva 30 min publicada. ¿Sigues esperando? Si no respondes en 10 min, la retiro."
        ),
        (Lang::En, StillWaiting { request }) => format!(
            "⏳ Your request {request} has been live for 30 min. Still waiting? If you don't answer within 10 min, I'll withdraw it."
        ),
        (Lang::Es, ConfirmDriver { request, .. }) => format!("🛵 Un chofer quiere hacer tu viaje {request}."),
        (Lang::En, ConfirmDriver { request, .. }) => format!("🛵 A driver wants to take your trip {request}."),
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
        (Lang::Es, TripAssigned { request, .. }) => format!("🛵 El viaje {request} es tuyo."),
        (Lang::En, TripAssigned { request, .. }) => format!("🛵 Trip {request} is yours."),
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
    }
}

pub fn channel_confirming(lang: Lang, request: &RequestId) -> String {
    match lang {
        Lang::Es => format!("⏳ Solicitud {request}: un chofer espera la confirmación del cliente."),
        Lang::En => format!("⏳ Request {request}: a driver is waiting for the client's confirmation."),
    }
}

/// To the client when a driver accepts: who wants the trip, without the
/// phone until they confirm.
pub fn confirm_driver(lang: Lang, request: &RequestId, p: &Profile, driver: &Shown) -> String {
    let rep = reputation(lang, driver, false);
    match lang {
        Lang::Es => format!(
            "🛵 Un chofer quiere hacer tu viaje {request}:\n\n🧑 {} · {rep}\n🏍 {}, {}\n🔢 Chapa: {}\n\nTienes 5 min para confirmarlo o cancelar.",
            p.alias, p.bike_model, p.bike_color, p.plate
        ),
        Lang::En => format!(
            "🛵 A driver wants to take your trip {request}:\n\n🧑 {} · {rep}\n🏍 {}, {}\n🔢 Plate: {}\n\nYou have 5 min to confirm or cancel.",
            p.alias, p.bike_model, p.bike_color, p.plate
        ),
    }
}

/// To the client after confirming: how to reach the driver.
pub fn driver_confirmed(lang: Lang, request: &RequestId, p: &Profile, username: Option<&str>) -> String {
    let mut text = match lang {
        Lang::Es => format!("✅ Viaje {request} confirmado. {} va hacia ti.\n📞 {}", p.alias, p.phone),
        Lang::En => format!("✅ Trip {request} confirmed. {} is on the way.\n📞 {}", p.alias, p.phone),
    };
    if let Some(u) = username {
        text.push_str(&format!("\n💬 @{u}"));
    }
    text
}

/// One counteroffer, as its own message to the client.
pub fn offer_card(lang: Lang, request: &RequestId, p: &Profile, amount: Cup, driver: &Shown) -> String {
    let rep = reputation(lang, driver, false);
    match lang {
        Lang::Es => format!(
            "💬 Contraoferta para tu viaje {request}\n\n💰 {amount} CUP\n🧑 {} · {rep}\n🏍 {}, {}\n\nVale 5 min.",
            p.alias, p.bike_model, p.bike_color
        ),
        Lang::En => format!(
            "💬 Counteroffer for your trip {request}\n\n💰 {amount} CUP\n🧑 {} · {rep}\n🏍 {}, {}\n\nValid for 5 min.",
            p.alias, p.bike_model, p.bike_color
        ),
    }
}

/// To a driver who opened "Counteroffer" from the channel.
pub fn counteroffer_prompt(lang: Lang, post: &Post) -> String {
    let post = channel_post(lang, post);
    match lang {
        Lang::Es => format!("{post}\n\n¿Por cuánto lo harías?"),
        Lang::En => format!("{post}\n\nWhat would you do it for?"),
    }
}

pub fn raise_prompt(lang: Lang, price: Cup) -> String {
    match lang {
        Lang::Es => format!("Ahora ofreces {price} CUP. ¿A cuánto lo subes?"),
        Lang::En => format!("You're offering {price} CUP. How much do you raise it to?"),
    }
}

/// Version of the terms below; raising it asks everyone to accept again.
pub const TERMS_VERSION: i64 = 1;

/// Terms of use and privacy notice, accepted on first use.
pub fn terms(lang: Lang) -> &'static str {
    match lang {
        Lang::Es => {
            "📜 Términos y privacidad de Asere Voy\n\n\
1. Asere Voy solo conecta a pasajeros y choferes. No es una empresa de transporte, no es dueña de las motos y no asegura los viajes. El precio y el pago (efectivo o transferencia) se acuerdan directamente entre cliente y chofer; el bot no maneja dinero.\n\n\
2. Qué guardamos: de los choferes, su ficha (alias, celular cifrado, moto, chapa, fotos y métodos de pago). De todos, la reputación como totales (estrellas, viajes y días en el bot), los bloqueos, las cancelaciones de los últimos 30 días, tu código de soporte y el idioma si lo cambiaste.\n\n\
3. Qué no guardamos: no hay historial de viajes. Cada solicitud (ubicaciones y referencia) se borra al cerrarse, y nunca guardamos el teléfono de los clientes. Las calificaciones son ciegas y solo se suman a los totales.\n\n\
4. Quién ve qué: en el canal de ofertas los choferes ven el municipio, la distancia, el precio y tu reputación, nunca tu ubicación exacta ni tu contacto. Esos datos solo los recibe el chofer que confirmes.\n\n\
5. Normas: respeto mutuo y nada ilegal. Tres cancelaciones en 7 días bloquean la cuenta 24 horas. Los admins pueden bloquear cuentas o quitar choferes.\n\n\
6. Para usar el bot necesitas un @usuario de Telegram: el chofer que confirmes y tú podrán escribirse por él.\n\n\
7. Ante cualquier problema (accidentes, robos o cobros abusivos), pulsa 💬 Soporte: te llevará a la cuenta de soporte, que verá tu cuenta de Telegram mientras te atiende.\n\n\
Datos del mapa © colaboradores de OpenStreetMap (licencia ODbL)."
        }
        Lang::En => {
            "📜 Asere Voy terms and privacy\n\n\
1. Asere Voy only connects riders and drivers. It is not a transport company, does not own the motorbikes and does not insure the trips. Price and payment (cash or transfer) are agreed directly between client and driver; the bot handles no money.\n\n\
2. What we keep: for drivers, their profile (alias, encrypted phone, motorbike, plate, photos and payment methods). For everyone, reputation as totals (stars, trips and days in the bot), blocks, cancellations from the last 30 days, your support code, and your language if you changed it.\n\n\
3. What we don't keep: there is no trip history. Each request (locations and landmark) is deleted when it closes, and we never keep clients' phone numbers. Ratings are blind and only added to the totals.\n\n\
4. Who sees what: in the offers channel drivers see the municipality, distance, price and your reputation, never your exact location or contact. Only the driver you confirm gets those.\n\n\
5. Rules: mutual respect and nothing illegal. Three cancellations in 7 days block the account for 24 hours. Admins can block accounts or remove drivers.\n\n\
6. You need a Telegram @username to use the bot: you and the driver you confirm can message each other through it.\n\n\
7. For any problem (accidents, theft or overcharging), tap 💬 Support: it takes you to the support account, which will see your Telegram account while helping you.\n\n\
Map data © OpenStreetMap contributors (ODbL license)."
        }
    }
}

/// "⭐4.7 · 23 trips · 120 days", or "New" instead of the stars.
pub fn reputation(lang: Lang, shown: &Shown, with_days: bool) -> String {
    let stars = match (lang, shown.stars) {
        (_, Some(avg)) => format!("⭐{avg:.1}"),
        (Lang::Es, None) => "🆕 Nuevo".to_owned(),
        (Lang::En, None) => "🆕 New".to_owned(),
    };
    let trips = match lang {
        Lang::Es => format!("{} viajes", shown.trips),
        Lang::En => format!("{} trips", shown.trips),
    };
    if !with_days {
        return format!("{stars} · {trips}");
    }
    let days = match lang {
        Lang::Es => format!("{} días en el bot", shown.days),
        Lang::En => format!("{} days in the bot", shown.days),
    };
    format!("{stars} · {trips} · {days}")
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

pub fn tag(lang: Lang, tag: Tag) -> &'static str {
    match (lang, tag) {
        (Lang::Es, Tag::Punctual) => "⏱ Puntual",
        (Lang::En, Tag::Punctual) => "⏱ Punctual",
        (Lang::Es, Tag::Kind) => "😊 Amable",
        (Lang::En, Tag::Kind) => "😊 Kind",
        (Lang::Es, Tag::Overcharged) => "💸 Cobró de más",
        (Lang::En, Tag::Overcharged) => "💸 Overcharged",
        (Lang::Es, Tag::NoShow) => "🙈 No apareció",
        (Lang::En, Tag::NoShow) => "🙈 Didn't show up",
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

pub fn blocked(lang: Lang, until: Option<&str>) -> String {
    match (lang, until) {
        (Lang::Es, Some(t)) => format!("⛔ Tu cuenta está bloqueada hasta el {t}. Si crees que es un error, escribe a 💬 Soporte."),
        (Lang::En, Some(t)) => format!("⛔ Your account is blocked until {t}. If you think it's a mistake, write to 💬 Support."),
        (Lang::Es, None) => "⛔ Tu cuenta está bloqueada. Si crees que es un error, escribe a 💬 Soporte.".into(),
        (Lang::En, None) => "⛔ Your account is blocked. If you think it's a mistake, write to 💬 Support.".into(),
    }
}

/// To the client after "Share trip", to forward to someone they trust.
pub fn share_trip(lang: Lang, request: &RequestId, p: &Profile, time: &str) -> String {
    match lang {
        Lang::Es => format!(
            "🛵 Voy en moto con Asere Voy.\nChofer: {}\nMoto: {}, {}\nChapa: {}\nSalida: {time}\nViaje: {request}",
            p.alias, p.bike_model, p.bike_color, p.plate
        ),
        Lang::En => format!(
            "🛵 I'm riding with Asere Voy.\nDriver: {}\nMotorbike: {}, {}\nPlate: {}\nDeparture: {time}\nTrip: {request}",
            p.alias, p.bike_model, p.bike_color, p.plate
        ),
    }
}

/// 💬 Support: the support account to write to, and the code to include.
pub fn support(lang: Lang, username: &str, code: &str) -> String {
    match lang {
        Lang::Es => format!(
            "💬 Escribe a nuestro soporte, @{username}, e incluye tu código {code} para que podamos ayudarte. Si es sobre un viaje, añade también su código."
        ),
        Lang::En => format!(
            "💬 Message our support, @{username}, and include your code {code} so we can help you. If it's about a trip, add its code too."
        ),
    }
}

/// `/caso <code>`: who the person is, for staff.
pub fn case_summary(
    lang: Lang,
    code: &str,
    driver: Option<DriverStatus>,
    as_client: Option<&Shown>,
    as_driver: Option<&Shown>,
    blocked: Option<Option<String>>,
) -> String {
    let none = match lang {
        Lang::Es => "sin viajes",
        Lang::En => "no trips",
    };
    let rep = |s: Option<&Shown>| s.map(|s| reputation(lang, s, true)).unwrap_or_else(|| none.into());
    let mut lines = vec![match lang {
        Lang::Es => format!("🔎 Código {code}"),
        Lang::En => format!("🔎 Code {code}"),
    }];
    lines.push(match lang {
        Lang::Es => format!("Como cliente: {}", rep(as_client)),
        Lang::En => format!("As a client: {}", rep(as_client)),
    });
    if let Some(status) = driver {
        let status = match (lang, status) {
            (Lang::Es, DriverStatus::Pending) => "pendiente",
            (Lang::En, DriverStatus::Pending) => "pending",
            (Lang::Es, DriverStatus::Active) => "activo",
            (Lang::En, DriverStatus::Active) => "active",
            (Lang::Es, DriverStatus::Blocked) => "bloqueado",
            (Lang::En, DriverStatus::Blocked) => "blocked",
        };
        lines.push(match lang {
            Lang::Es => format!("Chofer ({status}): {}", rep(as_driver)),
            Lang::En => format!("Driver ({status}): {}", rep(as_driver)),
        });
    }
    if let Some(until) = blocked {
        lines.push(match (lang, until) {
            (Lang::Es, Some(t)) => format!("⛔ Bloqueado hasta el {t}"),
            (Lang::En, Some(t)) => format!("⛔ Blocked until {t}"),
            (Lang::Es, None) => "⛔ Bloqueado".into(),
            (Lang::En, None) => "⛔ Blocked".into(),
        });
    }
    lines.join("\n")
}

pub fn alert_unsafe(lang: Lang, request: &RequestId) -> String {
    match lang {
        Lang::Es => format!("⚠️ Un chofer canceló el viaje {request} porque se sintió inseguro. Los botones actúan sobre el cliente."),
        Lang::En => format!("⚠️ A driver cancelled trip {request} because they felt unsafe. The buttons act on the client."),
    }
}

pub fn alert_cancellations(lang: Lang) -> String {
    match lang {
        Lang::Es => "⛔ Un cliente acumuló 3 cancelaciones en 7 días y quedó bloqueado 24 horas.".into(),
        Lang::En => "⛔ A client reached 3 cancellations in 7 days and is blocked for 24 hours.".into(),
    }
}

pub fn alert_low_rating(lang: Lang, role: Role, shown: &Shown) -> String {
    let rep = reputation(lang, shown, false);
    match (lang, role) {
        (Lang::Es, Role::Client) => format!("📉 Un cliente bajó de 3.5 estrellas ({rep})."),
        (Lang::En, Role::Client) => format!("📉 A client fell below 3.5 stars ({rep})."),
        (Lang::Es, Role::Driver) => format!("📉 Un chofer bajó de 3.5 estrellas ({rep})."),
        (Lang::En, Role::Driver) => format!("📉 A driver fell below 3.5 stars ({rep})."),
    }
}

/// `/tarifas`: the current fares.
pub fn fares(lang: Lang, f: &Fares) -> String {
    match lang {
        Lang::Es => format!(
            "💰 Tarifas\nBase: {} CUP\nPor km: {} CUP\nMínimo: {} % del precio sugerido\n\nPara cambiarlas (superadmins): /tarifa base|km|minimo <valor>",
            f.base, f.per_km, f.min_percent
        ),
        Lang::En => format!(
            "💰 Fares\nBase: {} CUP\nPer km: {} CUP\nMinimum: {} % of the suggested price\n\nTo change them (superadmins): /tarifa base|km|minimo <value>",
            f.base, f.per_km, f.min_percent
        ),
    }
}

/// `/metricas`: one block per day, newest first.
pub fn metrics(lang: Lang, days: &[Day]) -> String {
    if days.is_empty() {
        return match lang {
            Lang::Es => "📊 Todavía no hay métricas.".into(),
            Lang::En => "📊 No metrics yet.".into(),
        };
    }
    let num = |v: Option<f64>, digits: usize| v.map(|v| format!("{v:.digits$}")).unwrap_or_else(|| "–".into());
    let mut lines = vec![match lang {
        Lang::Es => "📊 Métricas (últimos días)".to_owned(),
        Lang::En => "📊 Metrics (last days)".to_owned(),
    }];
    for d in days {
        lines.push(match lang {
            Lang::Es => format!(
                "\n{}\nPublicadas {} · asignadas {} · completadas {} · canceladas {} · expiradas {}\nMedia: {} km · {} CUP · final/sugerido {}",
                d.day, d.published, d.assigned, d.completed, d.cancelled, d.expired,
                num(d.avg_km(), 1), num(d.avg_price(), 0), num(d.final_over_suggested(), 2)
            ),
            Lang::En => format!(
                "\n{}\nPublished {} · assigned {} · completed {} · cancelled {} · expired {}\nAverage: {} km · {} CUP · final/suggested {}",
                d.day, d.published, d.assigned, d.completed, d.cancelled, d.expired,
                num(d.avg_km(), 1), num(d.avg_price(), 0), num(d.final_over_suggested(), 2)
            ),
        });
    }
    lines.join("\n")
}

pub fn admin_invite_created(lang: Lang, link: &str) -> String {
    match lang {
        Lang::Es => format!("🔗 Enlace para nombrar un admin (un solo uso, vence en 48 h):\n{link}"),
        Lang::En => format!("🔗 Link to appoint an admin (single use, expires in 48 h):\n{link}"),
    }
}

pub fn admins_list(lang: Lang, names: &[String]) -> String {
    let head = match lang {
        Lang::Es => "👮 Admins:",
        Lang::En => "👮 Admins:",
    };
    std::iter::once(head.to_owned()).chain(names.iter().map(|n| format!("• {n}"))).collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_offer_shows_its_price_per_km() {
        let text = price_prompt(Lang::Es, 52.8, 68.0, 8010, 6410, 8010, true);
        assert!(text.contains("8010 CUP (≈152 CUP por km)"), "{text}");
        assert!(text.contains("≈52.8 km (unos 68 min)"), "{text}");
    }

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
