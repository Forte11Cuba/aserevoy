//! "How it works" guides and the terms.

use super::Lang;

/// The guides behind "❓ How it works".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Guide {
    About,
    Ride,
    Driver,
    Manager,
}

impl Guide {
    pub const ALL: [Guide; 4] = [Guide::About, Guide::Ride, Guide::Driver, Guide::Manager];

    pub fn code(self) -> &'static str {
        match self {
            Guide::About => "about",
            Guide::Ride => "ride",
            Guide::Driver => "driver",
            Guide::Manager => "manager",
        }
    }

    pub fn parse(code: &str) -> Option<Guide> {
        Guide::ALL.into_iter().find(|g| g.code() == code)
    }
}

pub fn guide_button(lang: Lang, g: Guide) -> &'static str {
    match (lang, g) {
        (Lang::Es, Guide::About) => "🤔 ¿Qué es Asere Voy?",
        (Lang::En, Guide::About) => "🤔 What is Asere Voy?",
        (Lang::Es, Guide::Ride) => "🚕 Pedir un viaje",
        (Lang::En, Guide::Ride) => "🚕 Requesting a ride",
        (Lang::Es, Guide::Driver) => "🧢 Ser chofer",
        (Lang::En, Guide::Driver) => "🧢 Being a driver",
        (Lang::Es, Guide::Manager) => "🏢 Ser gestor",
        (Lang::En, Guide::Manager) => "🏢 Being a manager",
    }
}

/// One guide; `None` is the index ("pick a guide").
pub fn guide(lang: Lang, g: Option<Guide>) -> &'static str {
    match (lang, g) {
        (Lang::Es, None) => "❓ Cómo funciona\n\nElige una guía:",
        (Lang::En, None) => "❓ How it works\n\nPick a guide:",
        (Lang::Es, Some(Guide::About)) => "🤔 ¿Qué es Asere Voy?\n\n\
Un bot de Telegram que conecta pasajeros y choferes de moto, carro o triciclo en La Habana.\n\n\
• Tú propones el precio: los choferes lo aceptan o te hacen una contraoferta.\n\
• El pago es en CUP, directo al chofer (efectivo, transferencia o bitcoin). El bot no maneja dinero.\n\
• Ves la foto, el vehículo, la chapa, el nombre y la reputación del chofer antes de confirmarlo.\n\
• Tus viajes completados quedan 45 días en 📅 Mis viajes, cifrados y sin ubicaciones exactas; solo los ves tú y puedes borrarlos.",
        (Lang::En, Some(Guide::About)) => "🤔 What is Asere Voy?\n\n\
A Telegram bot that connects passengers with motorbike, car and tricycle drivers in Havana.\n\n\
• You set the price: drivers accept it or send you a counteroffer.\n\
• You pay in CUP, directly to the driver (cash, transfer or bitcoin). The bot handles no money.\n\
• You see the driver's photo, vehicle, plate, name and reputation before confirming them.\n\
• Your completed trips stay 45 days in 📅 My trips, encrypted and without exact locations; only you see them and you can delete them.",
        (Lang::Es, Some(Guide::Ride)) => "🚕 Pedir un viaje\n\n\
1. Pulsa «🚕 Pedir viaje».\n\
2. Marca la recogida y el destino: tu ubicación, un punto en el mapa, una dirección («23 e/ L y M, Vedado») o un lugar («Coppelia»). Confirma cada punto en el mapa.\n\
3. Si hay choferes de varios tipos, elige en qué quieres ir (🏍 moto, 🚗 carro o 🛺 triciclo), cada uno con su precio sugerido. Pon tu precio con los botones −/+ y elige cómo pagas.\n\
4. Publica. Los choferes ven solo la zona, nunca tu ubicación exacta.\n\
5. Un chofer acepta o te hace una contraoferta. Mira su ficha y confírmalo.\n\
6. Recibes su contacto. Al terminar, pulsa «🏁 Completado» y califícalo.\n\n\
Mientras esperas puedes subir el precio o cancelar. En «🔧 Ajustes» puedes poner tu nombre, celular y foto para que el chofer te encuentre más fácil.",
        (Lang::En, Some(Guide::Ride)) => "🚕 Requesting a ride\n\n\
1. Tap \"🚕 Request a ride\".\n\
2. Mark the pickup and the destination: your location, a point on the map, an address (\"23 e/ L y M, Vedado\") or a place (\"Coppelia\"). Confirm each point on the map.\n\
3. If there are drivers of several kinds, pick what to go in (🏍 motorbike, 🚗 car or 🛺 tricycle), each with its suggested price. Set your price with the −/+ buttons and choose how you pay.\n\
4. Publish. Drivers only see the area, never your exact location.\n\
5. A driver accepts or sends a counteroffer. Check their profile and confirm them.\n\
6. You get their contact. When you arrive, tap \"🏁 Completed\" and rate them.\n\n\
While you wait you can raise the price or cancel. In \"🔧 Settings\" you can add your name, phone and photo so the driver finds you more easily.",
        (Lang::Es, Some(Guide::Driver)) => "🧢 Ser chofer\n\n\
1. En «🤝 Trabaja con nosotros», pulsa «Aplicar a ser chofer» y completa tu ficha: alias, nombre completo, celular, tu vehículo (tipo, chapa, modelo y color) y fotos.\n\
2. Un admin la revisa. Al aprobarte, el bot te manda el enlace del canal de ofertas.\n\
3. En el canal, «✅ Aceptar» toma el viaje al precio del cliente; «💬 Contraofertar» te deja proponer otro con −/+ y «✅ Enviar».\n\
4. Cuando el cliente te confirma, recibes la recogida, el destino y su contacto. Marca «📍 Llegué» y «🏁 Completado».\n\
5. En «🧢 Chofer» tienes tus estadísticas, tu ficha y «🚘 Mis vehículos»: hasta 5 (moto, carro o triciclo) y uno activo, con el que tomas los viajes de su tipo. Un vehículo nuevo, o una chapa o foto nueva, la aprueba un admin.\n\n\
Cobras directamente al cliente. Tu reputación (estrellas y viajes) la ven los clientes.",
        (Lang::En, Some(Guide::Driver)) => "🧢 Being a driver\n\n\
1. In \"🤝 Work with us\", tap \"Apply to drive\" and fill in your profile: alias, full name, phone, your vehicle (kind, plate, model and colour) and photos.\n\
2. An admin reviews it. Once approved, the bot sends you the link to the offers channel.\n\
3. In the channel, \"✅ Accept\" takes the trip at the client's price; \"💬 Counteroffer\" lets you propose another with −/+ and \"✅ Send\".\n\
4. When the client confirms you, you get the pickup, the destination and their contact. Tap \"📍 I'm here\" and \"🏁 Completed\".\n\
5. In \"🧢 Driver\" you have your stats, your profile and \"🚘 My vehicles\": up to 5 (motorbike, car or tricycle) and one active, which takes the trips of its kind. An admin approves a new vehicle, or a new plate or photo.\n\n\
You get paid directly by the client. Clients see your reputation (stars and trips).",
        (Lang::Es, Some(Guide::Manager)) => "🏢 Ser gestor (próximamente)\n\n\
Si tienes una agencia, podrás conectar a tus clientes con nuestros choferes:\n\n\
• Pides el viaje por tu cliente y en el canal sale como «Viaje de agencia: tu nombre».\n\
• Acuerdas con tu cliente lo que paga y fijas cuánto pagas al chofer.\n\
• El chofer cobra a tu cliente y te entrega la diferencia. Ejemplo: tu cliente paga 3500 CUP y el viaje se cierra en 3300; el chofer te entrega 200.\n\n\
Si te interesa, pulsa «🤝 Trabaja con nosotros» → «Aplicar a gestor» y te avisaremos.",
        (Lang::En, Some(Guide::Manager)) => "🏢 Being a manager (coming soon)\n\n\
If you run an agency, you'll be able to connect your clients with our drivers:\n\n\
• You request the ride for your client and the channel shows it as \"Agency trip: your name\".\n\
• You agree with your client what they pay and set what you pay the driver.\n\
• The driver charges your client and hands you the difference. Example: your client pays 3500 CUP and the trip closes at 3300; the driver hands you 200.\n\n\
If you're interested, tap \"🤝 Work with us\" → \"Apply as a manager\" and we'll let you know.",
    }
}

/// Version of the terms below; raising it asks everyone to accept again.
pub const TERMS_VERSION: i64 = 1;

/// Terms of use and privacy notice, accepted on first use.
pub fn terms(lang: Lang) -> &'static str {
    match lang {
        Lang::Es => {
            "📜 Términos y privacidad de Asere Voy\n\n\
1. Asere Voy solo conecta a pasajeros y choferes. No es una empresa de transporte, no es dueña de los vehículos y no asegura los viajes. El precio y el pago (efectivo, transferencia o bitcoin) se acuerdan directamente entre cliente y chofer; el bot no maneja dinero.\n\n\
2. Qué guardamos: de los choferes, su ficha (alias, nombre completo y celular cifrados, sus vehículos con chapa y fotos). De los clientes que quieran, su nombre, su celular (cifrados) y su foto, hasta que los borren en 🔧 Ajustes. De todos, la reputación como totales (estrellas, viajes y días en el bot), los bloqueos, las cancelaciones de los últimos 30 días, tu código de soporte y el idioma si lo cambiaste. Tus viajes completados de los últimos 45 días (fecha, origen y destino como texto, sin ubicaciones exactas, precio, la otra persona y tu nota, cifrados), que solo ves tú en 📅 Mis viajes y puedes borrar; y, de los choferes, sus viajes y ganancias totales por mes.\n\n\
3. Qué no guardamos: no hay historial de viajes. Cada solicitud (ubicaciones y referencia) se borra al cerrarse, y nunca guardamos el teléfono de los clientes. Las calificaciones son ciegas y solo se suman a los totales.\n\n\
4. Quién ve qué: en el canal de ofertas los choferes ven el municipio, la distancia, el precio y tu reputación, nunca tu ubicación exacta ni tu contacto. Esos datos solo los recibe el chofer que confirmes.\n\n\
5. Normas: respeto mutuo y nada ilegal. Tres cancelaciones en 7 días bloquean la cuenta 24 horas. Los admins pueden bloquear cuentas o quitar choferes.\n\n\
6. Para usar el bot necesitas un @usuario de Telegram: el chofer que confirmes y tú podrán escribirse por él.\n\n\
7. Ante cualquier problema (accidentes, robos o cobros abusivos), pulsa 💬 Soporte: te llevará a la cuenta de soporte, que verá tu cuenta de Telegram mientras te atiende.\n\n\
Datos del mapa © colaboradores de OpenStreetMap (licencia ODbL)."
        }
        Lang::En => {
            "📜 Asere Voy terms and privacy\n\n\
1. Asere Voy only connects riders and drivers. It is not a transport company, does not own the vehicles and does not insure the trips. Price and payment (cash, transfer or bitcoin) are agreed directly between client and driver; the bot handles no money.\n\n\
2. What we keep: for drivers, their profile (alias, encrypted full name and phone, their vehicles with plates and photos). For clients who choose to, their name, phone (encrypted) and photo, until they delete them in 🔧 Settings. For everyone, reputation as totals (stars, trips and days in the bot), blocks, cancellations from the last 30 days, your support code, and your language if you changed it. Your completed trips of the last 45 days (date, origin and destination as text, no exact locations, price, the other person and your rating, encrypted), which only you see in 📅 My trips and can delete; and, for drivers, their trips and earnings per month.\n\n\
3. What we don't keep: there is no trip history. Each request (locations and landmark) is deleted when it closes, and we never keep clients' phone numbers. Ratings are blind and only added to the totals.\n\n\
4. Who sees what: in the offers channel drivers see the municipality, distance, price and your reputation, never your exact location or contact. Only the driver you confirm gets those.\n\n\
5. Rules: mutual respect and nothing illegal. Three cancellations in 7 days block the account for 24 hours. Admins can block accounts or remove drivers.\n\n\
6. You need a Telegram @username to use the bot: you and the driver you confirm can message each other through it.\n\n\
7. For any problem (accidents, theft or overcharging), tap 💬 Support: it takes you to the support account, which will see your Telegram account while helping you.\n\n\
Map data © OpenStreetMap contributors (ODbL license)."
        }
    }
}
