//! Fixed texts, one variant per message.

use super::Lang;

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
    AddressNotFound,
    AskReparto,
    ChooseReparto,
    MapPoint,
    MapLinkFailed,
    OutsideHavana,
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
    /// The active vehicle does not fit; the bot wrote with a button to switch.
    SwitchOffered,
    AlreadyClosed,
    KeptWaiting,
    Withdrawn,
    CounterofferHow,
    RaiseFirst,
    PanelHome,
    FareOutOfRange,
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
    StaffBlocked,
    StaffUnblocked,
    DriverRemoved,
    YouWereRemoved,
    YouWereUnblocked,
    NoActiveRequest,
    // Driver profile.
    AskAlias,
    HistoryDeleted,
    ConfirmFinished,
    ChannelNotActive,
    TripMenuBelow,
    NoActiveTrip,
    Notified,
    Settings,
    DetailsSaved,
    DetailsDeleted,
    AskClientName,
    AskClientPhone,
    AskClientPhoto,
    AskRidePhone,
    RidePhoneSaved,
    RidePhoneSkipped,
    WorkWithUs,
    ManagerSoon,
    AskFullName,
    InvalidFullName,
    DriverMenu,
    FullNameMissing,
    EditWhich,
    NameSentForReview,
    NameApproved,
    NameRejected,
    AskDriverPhone,
    AskPlate,
    AskBikeModel,
    AskBikeColor,
    AskPhoto,
    AskBikePhoto,
    AskVehicleKind,
    PickVehicleKind,
    ApplicationPending,
    NoVehiclePhoto,
    RemoveVehicleSure,
    VehicleRemoved,
    VehicleRemovedPickAnother,
    VehicleSent,
    VehicleUpdated,
    VehicleChangeSent,
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
            "📍 ¿Dónde te recogemos? Pulsa «Enviar mi ubicación», marca otro punto con 📎 → Ubicación, o escribe la dirección o un lugar conocido (por ejemplo: 23 e/ L y M, Vedado, o Coppelia)."
        }
        (Lang::En, AskPickup) => {
            "📍 Where should we pick you up? Tap \"Send my location\", mark another point with 📎 → Location, or type the address or a known place (for example: 23 e/ L y M, Vedado, or Coppelia)."
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
            "🏁 ¿A dónde vas? Marca el punto con 📎 → Ubicación, o escribe la dirección o un lugar conocido (por ejemplo: 23 e/ L y M, Vedado, o Coppelia)."
        }
        (Lang::En, AskDropoff) => {
            "🏁 Where are you going? Mark the point with 📎 → Location, or type the address or a known place (for example: 23 e/ L y M, Vedado, or Coppelia)."
        }
        (Lang::Es, ConfirmDropoff) => "¿Es aquí el destino?",
        (Lang::En, ConfirmDropoff) => "Is this the destination?",
        (Lang::Es, AddressNotFound) => {
            "No encontré esa dirección en el mapa. Envía tu ubicación 📍, o elige tu municipio y le daré tu descripción al chofer cuando lo confirmes (el precio se calcula aproximado)."
        }
        (Lang::En, AddressNotFound) => {
            "I couldn't find that address on the map. Send your location 📍, or pick your municipality and I'll give your description to the driver once you confirm them (the price is estimated)."
        }
        (Lang::Es, AskReparto) => {
            "¿En qué reparto o zona? Elígelo o escríbelo (por ejemplo: Alamar zona 6). También puedes enviar tu ubicación 📍."
        }
        (Lang::En, AskReparto) => {
            "Which neighborhood (reparto) or zone? Pick it or type it (for example: Alamar zona 6). You can also send your location 📍."
        }
        (Lang::Es, MapPoint) => "El punto del enlace o de las coordenadas",
        (Lang::En, MapPoint) => "The point of the link or coordinates",
        (Lang::Es, MapLinkFailed) => "No pude abrir ese enlace. Envía tu ubicación 📍 o escribe la dirección.",
        (Lang::En, MapLinkFailed) => "I couldn't open that link. Send your location 📍 or type the address.",
        (Lang::Es, OutsideHavana) => "Ese punto está fuera de La Habana. Revisa el enlace o las coordenadas, o envía tu ubicación 📍.",
        (Lang::En, OutsideHavana) => "That point is outside Havana. Check the link or coordinates, or send your location 📍.",
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
        (Lang::Es, SwitchOffered) => "Tu vehículo activo no es de este tipo. Te escribí por privado para cambiarlo.",
        (Lang::En, SwitchOffered) => "Your active vehicle is not of this kind. I wrote to you privately to switch it.",
        (Lang::Es, AcceptedAlert) => "¡Reservado! El cliente tiene 5 min para confirmarte. Te escribí por privado.",
        (Lang::En, AcceptedAlert) => "Reserved! The client has 5 min to confirm you. I've messaged you privately.",
        (Lang::Es, AlreadyClosed) => "Este viaje ya se cerró.",
        (Lang::En, AlreadyClosed) => "This trip is already closed.",
        (Lang::Es, KeptWaiting) => "👍 Perfecto, tu solicitud sigue publicada.",
        (Lang::En, KeptWaiting) => "👍 Great, your request stays published.",
        (Lang::Es, FareUsage) => {
            "Uso: /tarifa base 100, /tarifa km 150 o /tarifa minimo 80 (porcentaje del precio sugerido), para la moto; para los otros, el tipo delante: /tarifa carro km 200, /tarifa triciclo base 80."
        }
        (Lang::En, FareUsage) => {
            "Usage: /tarifa base 100, /tarifa km 150 or /tarifa minimo 80 (percentage of the suggested price), for the moto; for the others, the kind first: /tarifa carro km 200, /tarifa triciclo base 80."
        }
        (Lang::Es, PanelHome) => "⚙️ Administración: elige abajo qué quieres gestionar.",
        (Lang::En, PanelHome) => "⚙️ Administration: pick below what you want to manage.",
        (Lang::Es, FareOutOfRange) => "Ese valor no es válido (el mínimo va de 0 a 100 %, los precios deben ser mayores que 0).",
        (Lang::En, FareOutOfRange) => "That value isn't valid (the minimum goes from 0 to 100 %, prices must be above 0).",
        (Lang::Es, CounterofferHow) => {
            "💬 Contraoferta: ajusta el monto con los botones + y −, o escríbelo. Cuando te cuadre, pulsa ✅ Enviar; ❌ Cancelar para salir."
        }
        (Lang::En, CounterofferHow) => {
            "💬 Counteroffer: adjust the amount with the + and − buttons, or type it. When it suits you, tap ✅ Send; ❌ Cancel to leave."
        }
        (Lang::Es, RaiseFirst) => "Sube el monto por encima del precio del cliente, o usa Aceptar en el canal.",
        (Lang::En, RaiseFirst) => "Raise the amount above the client's price, or use Accept in the channel.",
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
        (Lang::Es, NoActiveRequest) => "No tienes ninguna solicitud abierta.",
        (Lang::En, NoActiveRequest) => "You have no open request.",

        (Lang::Es, AskFullName) => {
            "🪪 Tu nombre completo, como en el carnet de identidad. Lo ven los admins y, por seguridad, el cliente cuando te asignan su viaje."
        }
        (Lang::En, AskFullName) => {
            "🪪 Your full name, as on your ID card. Admins see it and, for safety, the client once you're assigned their trip."
        }
        (Lang::Es, InvalidFullName) => "Escribe tu nombre y tus apellidos (al menos dos palabras).",
        (Lang::En, InvalidFullName) => "Type your name and surname (at least two words).",
        (Lang::Es, DriverMenu) => "🧢 Menú de chofer.",
        (Lang::En, DriverMenu) => "🧢 Driver menu.",
        (Lang::Es, FullNameMissing) => "⚠️ Falta tu nombre completo: pulsa «Nombre completo» para añadirlo.",
        (Lang::En, FullNameMissing) => "⚠️ Your full name is missing: tap \"Full name\" to add it.",
        (Lang::Es, EditWhich) => "✏️ Para cambiar un dato, pulsa su botón.",
        (Lang::En, EditWhich) => "✏️ To change something, tap its button.",
        (Lang::Es, NameSentForReview) => "⏳ Nombre enviado. Un admin lo revisará; mientras, sigue el anterior.",
        (Lang::En, NameSentForReview) => "⏳ Name sent. An admin will review it; until then the previous one stays.",
        (Lang::Es, NameApproved) => "✅ Tu nombre completo fue aprobado.",
        (Lang::En, NameApproved) => "✅ Your full name was approved.",
        (Lang::Es, NameRejected) => "❌ Tu cambio de nombre no fue aprobado. Si crees que es un error, escribe a soporte.",
        (Lang::En, NameRejected) => "❌ Your name change wasn't approved. If you think it's a mistake, write to support.",
        (Lang::Es, WorkWithUs) => {
            "🤝 Trabaja con nosotros\n\n🧢 Chofer: recibe ofertas de viaje en el canal, acepta las que te convengan o propón tu precio.\n\n🏢 Gestor (próximamente): conecta a tus clientes con nuestros choferes. Pones tu tarifa para el chofer y la de tu cliente, y cuando un chofer acepta, ganas la diferencia.\n\nElige abajo."
        }
        (Lang::En, WorkWithUs) => {
            "🤝 Work with us\n\n🧢 Driver: get trip offers in the channel, take the ones that suit you or propose your price.\n\n🏢 Manager (coming soon): connect your clients with our drivers. You set your rate for the driver and the one for your client, and when a driver accepts, you keep the difference.\n\nChoose below."
        }
        (Lang::Es, ManagerSoon) => "🏢 Los gestores llegan pronto. Ya avisé al equipo de que te interesa; te escribiremos.",
        (Lang::En, ManagerSoon) => "🏢 Managers are coming soon. I've told the team you're interested; we'll write to you.",
        (Lang::Es, Settings) => "🔧 Ajustes",
        (Lang::En, Settings) => "🔧 Settings",
        (Lang::Es, DetailsSaved) => "✅ Guardado.",
        (Lang::En, DetailsSaved) => "✅ Saved.",
        (Lang::Es, DetailsDeleted) => "🗑 Tus datos se borraron.",
        (Lang::En, DetailsDeleted) => "🗑 Your details were deleted.",
        (Lang::Es, AskClientName) => "🙋 ¿Cómo te llamas? Así te reconoce el chofer.",
        (Lang::En, AskClientName) => "🙋 What's your name? So the driver knows who to look for.",
        (Lang::Es, AskClientPhone) => "📞 Tu celular, para que el chofer te llame. Compártelo con el botón o escríbelo.",
        (Lang::En, AskClientPhone) => "📞 Your phone, so the driver can call you. Share it with the button or type it.",
        (Lang::Es, AskClientPhoto) => "📷 Manda una foto tuya, para que el chofer te reconozca al recogerte.",
        (Lang::En, AskClientPhoto) => "📷 Send a photo of yourself, so the driver recognizes you at the pickup.",
        (Lang::Es, AskRidePhone) => {
            "📞 ¿Quieres poner tu celular para que el chofer te contacte más fácil? Lo guardamos en 🔧 Ajustes para tus próximos viajes y puedes borrarlo cuando quieras. Si no, pulsa Omitir: te contactará por Telegram."
        }
        (Lang::En, AskRidePhone) => {
            "📞 Would you like to add your phone so the driver can reach you more easily? We keep it in 🔧 Settings for your next trips and you can delete it anytime. Otherwise tap Skip: they'll reach you on Telegram."
        }
        (Lang::Es, RidePhoneSaved) => "📞 Guardado. El chofer lo recibirá cuando lo confirmes.",
        (Lang::En, RidePhoneSaved) => "📞 Saved. The driver will get it once you confirm them.",
        (Lang::Es, RidePhoneSkipped) => "Sin celular: el chofer te contactará por Telegram.",
        (Lang::En, RidePhoneSkipped) => "No phone: the driver will reach you on Telegram.",
        (Lang::Es, TripMenuBelow) => "🧭 Mientras dure el viaje, tienes sus botones en el menú de abajo.",
        (Lang::En, TripMenuBelow) => "🧭 While the trip lasts, its buttons are in the menu below.",
        (Lang::Es, NoActiveTrip) => "No tienes ningún viaje en curso. Aquí tienes el menú.",
        (Lang::En, NoActiveTrip) => "You have no trip in progress. Here's the menu.",
        (Lang::Es, Notified) => "✅ Aviso enviado.",
        (Lang::En, Notified) => "✅ Notice sent.",
        (Lang::Es, ChannelNotActive) => {
            "El canal de ofertas es solo para choferes activos. Si tu ficha está pendiente, te aviso cuando un admin la apruebe."
        }
        (Lang::En, ChannelNotActive) => {
            "The offers channel is only for active drivers. If your profile is pending, I'll tell you when an admin approves it."
        }
        (Lang::Es, ConfirmFinished) => "🏁 ¿El viaje terminó? Al confirmarlo se cierra y te pido que califiques.",
        (Lang::En, ConfirmFinished) => "🏁 Is the trip over? Once you confirm, it closes and I'll ask you to rate.",
        (Lang::Es, HistoryDeleted) => "🗑 Tu historial de viajes se borró.",
        (Lang::En, HistoryDeleted) => "🗑 Your trip history was deleted.",
        (Lang::Es, AskAlias) => "👤 ¿Cómo quieres que te vean los clientes? Escribe tu alias.",
        (Lang::En, AskAlias) => "👤 How should clients see you? Type your alias.",
        (Lang::Es, AskDriverPhone) => "📞 Tu celular, para que los clientes te llamen. Compártelo con el botón o escríbelo.",
        (Lang::En, AskDriverPhone) => "📞 Your mobile number, so clients can call you. Share it with the button or type it.",
        (Lang::Es, AskVehicleKind) => "🚘 ¿Con qué vehículo trabajas? Elige el tipo con los botones de abajo.",
        (Lang::En, AskVehicleKind) => "🚘 What vehicle do you drive? Pick the kind with the buttons below.",
        (Lang::Es, PickVehicleKind) => "Elige el tipo con los botones: 🏍 Moto, 🚗 Carro o 🛺 Triciclo.",
        (Lang::En, PickVehicleKind) => "Pick the kind with the buttons: 🏍 Moto, 🚗 Car or 🛺 Tricycle.",
        (Lang::Es, AskPlate) => "🔢 Número de chapa del vehículo:",
        (Lang::En, AskPlate) => "🔢 The vehicle's plate number:",
        (Lang::Es, AskBikeModel) => "🚘 Marca y modelo del vehículo:",
        (Lang::En, AskBikeModel) => "🚘 The vehicle's make and model:",
        (Lang::Es, AskBikeColor) => "🎨 Color del vehículo:",
        (Lang::En, AskBikeColor) => "🎨 The vehicle's colour:",
        (Lang::Es, AskPhoto) => "🤳 Envía una foto tuya. Los clientes la verán cuando aceptes su viaje.",
        (Lang::En, AskPhoto) => "🤳 Send a photo of yourself. Clients will see it when you accept their trip.",
        (Lang::Es, AskBikePhoto) => "📷 Envía una foto del vehículo, o pulsa Omitir.",
        (Lang::En, AskBikePhoto) => "📷 Send a photo of the vehicle, or tap Skip.",
        (Lang::Es, ApplicationPending) => "⏳ Tu solicitud de chofer espera la aprobación de un admin.",
        (Lang::En, ApplicationPending) => "⏳ Your driver application is waiting for an admin's approval.",
        (Lang::Es, NoVehiclePhoto) => "Este vehículo no tiene foto.",
        (Lang::En, NoVehiclePhoto) => "This vehicle has no photo.",
        (Lang::Es, RemoveVehicleSure) => "¿Quitar este vehículo? Para volver a usarlo tendrás que añadirlo de nuevo y esperar la aprobación.",
        (Lang::En, RemoveVehicleSure) => "Remove this vehicle? To use it again you'll have to add it again and wait for approval.",
        (Lang::Es, VehicleRemoved) => "🗑 Vehículo quitado.",
        (Lang::En, VehicleRemoved) => "🗑 Vehicle removed.",
        (Lang::Es, VehicleRemovedPickAnother) => "🗑 Vehículo quitado. Sin vehículo activo no puedes tomar viajes: elige otro con ✅ Usar este.",
        (Lang::En, VehicleRemovedPickAnother) => "🗑 Vehicle removed. Without an active vehicle you can't take trips: pick another with ✅ Use this one.",
        (Lang::Es, VehicleSent) => "📨 Vehículo enviado. Un admin lo revisará y te avisaré; mientras, no lo puedes usar.",
        (Lang::En, VehicleSent) => "📨 Vehicle sent. An admin will review it and I'll let you know; until then you can't use it.",
        (Lang::Es, VehicleUpdated) => "✅ Vehículo actualizado.",
        (Lang::En, VehicleUpdated) => "✅ Vehicle updated.",
        (Lang::Es, VehicleChangeSent) => "📨 Cambio enviado. Hasta que un admin lo apruebe, sigue valiendo el dato anterior.",
        (Lang::En, VehicleChangeSent) => "📨 Change sent. Until an admin approves it, the previous one still counts.",
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
