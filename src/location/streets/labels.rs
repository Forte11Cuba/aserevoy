//! How candidates are shown: place kinds in Spanish and button labels.

use super::{Candidate, Place};
use super::parse::normalize;

/// Kinds of place people put before the name ("heladería Coppelia"), as
/// `normalize` leaves them (accents gone, doubled letters once).
pub(super) const PLACE_KINDS: [&str; 16] = [
    "heladeria", "helados", "hotel", "hospital", "policlinico", "restaurante", "cafeteria", "parque", "cine",
    "teatro", "museo", "iglesia", "escuela", "tienda", "mercado", "estadio",
];

pub(super) fn place_candidate(p: &Place) -> Candidate {
    Candidate {
        label: match kind_label(&p.kind) {
            // "Hospital Ameijeiras (hospital)" says it twice.
            Some(kind) if !normalize(&p.name).contains(&normalize(kind)) => {
                format!("{} ({kind}) en {}", p.name, p.municipality)
            }
            _ => format!("{} en {}", p.name, p.municipality),
        },
        point: p.point,
        municipality: p.municipality.clone(),
        inferred: false,
        short: String::new(),
    }
}

/// The OSM kind of place in Spanish, for the common ones; the rest (and
/// vague ones like "information") are left out of the label.
fn kind_label(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "guest_house" | "chalet" => "casa de renta",
        "hostel" => "hostal",
        "hotel" | "motel" | "resort" | "beach_resort" => "hotel",
        "restaurant" | "restaurant;fast_food" => "restaurante",
        "fast_food" | "food_court" => "comida rápida",
        "cafe" | "internet_cafe" => "cafetería",
        "bar" | "pub" | "biergarten" => "bar",
        "nightclub" | "nightclub;bar" => "discoteca",
        "ice_cream" => "heladería",
        "bakery" | "pastry" | "confectionery" => "panadería",
        "school" | "college" | "language_school" | "music_school" => "escuela",
        "university" => "universidad",
        "kindergarten" | "childcare" => "círculo infantil",
        "hospital" => "hospital",
        "clinic" => "policlínico",
        "doctors" | "doctor" => "consultorio",
        "dentist" => "dentista",
        "pharmacy" | "chemist" => "farmacia",
        "veterinary" => "veterinaria",
        "place_of_worship" | "monastery" => "iglesia",
        "monument" | "memorial" => "monumento",
        "museum" | "museum;attraction" => "museo",
        "gallery" | "arts_centre" => "galería",
        "theatre" => "teatro",
        "cinema" => "cine",
        "library" => "biblioteca",
        "park" | "garden" | "playground" => "parque",
        "stadium" | "sports_centre" | "pitch" => "instalación deportiva",
        "fitness_centre" => "gimnasio",
        "swimming_pool" => "piscina",
        "attraction" | "viewpoint" | "theme_park" | "castle" | "zoo" | "aquarium" => "lugar turístico",
        "supermarket" | "department_store" | "mall" => "tienda",
        "convenience" | "variety_store" | "general" | "kiosk" => "tienda",
        "marketplace" | "greengrocer" => "mercado",
        "butcher" => "carnicería",
        "clothes" | "shoes" => "tienda de ropa",
        "mobile_phone" | "electronics" | "computer" => "tienda de electrónica",
        "hardware" => "ferretería",
        "bank" => "banco",
        "atm" => "cajero",
        "bureau_de_change" => "casa de cambio",
        "post_office" => "correos",
        "police" => "policía",
        "fire_station" => "bomberos",
        "townhall" => "gobierno",
        "courthouse" => "tribunal",
        "fuel" => "servicentro",
        "car_repair" | "motorcycle_repair" | "tyres" => "taller",
        "car_rental" => "renta de autos",
        "bus_station" => "terminal de ómnibus",
        "railway_station" => "estación de trenes",
        "ferry_terminal" => "terminal de lanchas",
        "parking" => "parqueo",
        "travel_agency" => "agencia de viajes",
        "hairdresser" | "beauty" => "peluquería",
        "community_centre" | "social_centre" | "social_club" => "casa de cultura",
        "farm" => "finca",
        "beach" => "playa",
        _ => return None,
    })
}

/// "Playa · Miramar · 12 entre 3ra y 5ta": the municipality (short name,
/// left out when `with_municipality` is false), the reparto unless it is
/// named like the municipality, and the label without "Calle" at the start
/// or "en <municipality>" at the end.
pub(super) fn button_label(c: &Candidate, reparto: Option<&str>, with_municipality: bool) -> String {
    let municipality = short_municipality(&c.municipality);
    let rest = c.label.strip_suffix(&format!(" en {}", c.municipality)).unwrap_or(&c.label);
    let rest = rest.strip_prefix("Calle ").unwrap_or(rest);
    let mut parts = Vec::new();
    if with_municipality {
        parts.push(municipality);
    }
    if let Some(reparto) = reparto
        && reparto != c.municipality
        && reparto != municipality
    {
        parts.push(reparto);
    }
    parts.push(rest);
    parts.join(" · ")
}

/// The name people say, shorter for a button.
fn short_municipality(name: &str) -> &str {
    match name {
        "Plaza de la Revolución" => "Plaza",
        "Diez de Octubre" => "10 de Octubre",
        "San Miguel del Padrón" => "San Miguel",
        "La Habana Vieja" => "Habana Vieja",
        other => other,
    }
}

/// "Calle 23" → "23", for the short corner shown in the channel.
pub(super) fn short(name: &str) -> &str {
    name.strip_prefix("Calle ").unwrap_or(name)
}
