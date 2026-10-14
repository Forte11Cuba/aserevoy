//! Written addresses and corners, from `data/streets.sqlite` (built by
//! `scripts/prepare_osm.py` from OpenStreetMap): named streets, the corners
//! where two of them cross, neighborhoods (repartos) and named places.
//!
//! The rules here turn what a client writes into candidate points:
//! - "23 e/ L y M" or "23 entre L y M": between two corners of the street;
//! - "23 esquina L", "23 esq. L" or "23 y L": a corner;
//! - "Coppelia", "hospital Ameijeiras": a named place.
//!
//! A municipality or reparto in the text narrows the candidates. Choosing
//! among several is left to Jev (see `jev.rs`); one is used directly.

use std::str::FromStr;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

use crate::domain::request::Point;

/// A corner farther than this from a pin is not shown in the channel.
const CORNER_MAX_KM: f64 = 0.25;
/// Two corners of the same streets closer than this are the same corner
/// (wide avenues are drawn as two roads).
const SAME_CORNER_KM: f64 = 0.15;
/// "X between A and B" uses both corners only if they are this close.
const BLOCK_MAX_KM: f64 = 1.0;
/// A place "in" a reparto is in its municipality and within this distance of
/// the reparto's point (repartos like Miramar are several km long).
const NEAR_REPARTO_KM: f64 = 3.5;
/// At most this many candidates are kept for Jev.
const MAX_CANDIDATES: usize = 8;

#[derive(Debug, Clone)]
struct Corner {
    a: String,
    b: String,
    a_norm: String,
    b_norm: String,
    point: Point,
    municipality: String,
}

#[derive(Debug, Clone)]
struct Place {
    name: String,
    norm: String,
    kind: String,
    point: Point,
    municipality: String,
}

/// A point the text may refer to, with a description for Jev and the client.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub label: String,
    pub point: Point,
    pub municipality: String,
}

#[derive(Debug, Default)]
pub struct Streets {
    corners: Vec<Corner>,
    places: Vec<Place>,
    /// Municipality names, normalized, for hints in the text.
    municipalities: Vec<(String, String)>,
}

impl Streets {
    /// Loads the whole file into memory (a few MB); it never changes at run time.
    pub async fn load(path: &str) -> anyhow::Result<Streets> {
        let options = SqliteConnectOptions::from_str(&format!("sqlite://{path}"))?.read_only(true);
        let pool = SqlitePoolOptions::new().max_connections(1).connect_with(options).await?;
        let corners: Vec<(String, String, f64, f64, String)> =
            sqlx::query_as("SELECT street_a, street_b, lon, lat, municipality FROM corners")
                .fetch_all(&pool)
                .await?;
        let places: Vec<(String, String, f64, f64, String)> =
            sqlx::query_as("SELECT name, kind, lon, lat, municipality FROM places").fetch_all(&pool).await?;
        pool.close().await;

        let mut streets = Streets::default();
        for (a, b, lon, lat, municipality) in corners {
            let (a_norm, b_norm) = (normalize(&a), normalize(&b));
            // A street that changes name ("Calle 23" / "23 (La Rampa)") is not a corner.
            if same_street(&a_norm, &b_norm) {
                continue;
            }
            streets.corners.push(Corner { a, b, a_norm, b_norm, point: Point { lat, lon }, municipality });
        }
        for (name, kind, lon, lat, municipality) in places {
            let norm = normalize(&name);
            streets.places.push(Place { name, norm, kind, point: Point { lat, lon }, municipality });
        }
        let mut municipalities: Vec<String> = streets.places.iter().map(|p| p.municipality.clone()).collect();
        municipalities.sort();
        municipalities.dedup();
        streets.municipalities = municipalities.into_iter().map(|m| (normalize(&m), m)).collect();
        Ok(streets)
    }

    /// "23 y L": the nearest corner to a pin, if close enough.
    pub fn nearest_corner(&self, p: Point) -> Option<String> {
        self.corners
            .iter()
            .filter(|c| (c.point.lat - p.lat).abs() < 0.005 && (c.point.lon - p.lon).abs() < 0.005)
            .map(|c| (km_between(p, c.point), c))
            .filter(|(km, _)| *km <= CORNER_MAX_KM)
            .min_by(|x, y| x.0.total_cmp(&y.0))
            .map(|(_, c)| format!("{} y {}", short(&c.a), short(&c.b)))
    }

    /// The points a written address may refer to, best first.
    pub fn candidates(&self, text: &str) -> Vec<Candidate> {
        let address = parse(text);
        let area = self.area_hint(&address.hints);
        let mut found = match &address.kind {
            AddressKind::Between { street, a, b } => self.between(street, a, b),
            AddressKind::Corner { street, cross } => {
                self.corners_of(street, cross).into_iter().map(|c| self.corner_candidate(c)).collect()
            }
            AddressKind::Place(name) => self.places_named(name),
        };
        // A named municipality or reparto rules out places elsewhere, even if
        // none is left: better to ask for the pin than to suggest the wrong one.
        if let Some(area) = &area {
            found.retain(|c| area.contains(c));
        }
        dedup_nearby(&mut found);
        found.truncate(MAX_CANDIDATES);
        found
    }

    fn corners_of(&self, street: &str, cross: &str) -> Vec<&Corner> {
        self.corners
            .iter()
            .filter(|c| {
                (name_matches(&c.a_norm, street) && name_matches(&c.b_norm, cross))
                    || (name_matches(&c.b_norm, street) && name_matches(&c.a_norm, cross))
            })
            .collect()
    }

    fn corner_candidate(&self, c: &Corner) -> Candidate {
        Candidate {
            label: format!("Esquina de {} y {} en {}", c.a, c.b, c.municipality),
            point: c.point,
            municipality: c.municipality.clone(),
        }
    }

    /// The middle of the block between the two corners, or one of the corners
    /// if only one is known.
    fn between(&self, street: &str, a: &str, b: &str) -> Vec<Candidate> {
        let (first, second) = (self.corners_of(street, a), self.corners_of(street, b));
        let mut found = Vec::new();
        for x in &first {
            for y in &second {
                if x.municipality == y.municipality && km_between(x.point, y.point) <= BLOCK_MAX_KM {
                    let street_name = if name_matches(&x.a_norm, street) { &x.a } else { &x.b };
                    let (cross_a, cross_b) = (other(x, street), other(y, street));
                    found.push(Candidate {
                        label: format!("{street_name} entre {cross_a} y {cross_b} en {}", x.municipality),
                        point: Point { lat: (x.point.lat + y.point.lat) / 2.0, lon: (x.point.lon + y.point.lon) / 2.0 },
                        municipality: x.municipality.clone(),
                    });
                }
            }
        }
        if found.is_empty() {
            found = first.into_iter().chain(second).map(|c| self.corner_candidate(c)).collect();
        }
        found
    }

    fn places_named(&self, name: &str) -> Vec<Candidate> {
        if name.is_empty() {
            return Vec::new();
        }
        let mut found: Vec<&Place> =
            self.places.iter().filter(|p| p.kind != "neighborhood" && name_matches(&p.norm, name)).collect();
        // Exact names first, then shorter ones (closer to what was written).
        found.sort_by_key(|p| (p.norm != name, p.norm.len()));
        found
            .into_iter()
            .map(|p| Candidate {
                label: format!("{} ({}) en {}", p.name, kind_label(&p.kind), p.municipality),
                point: p.point,
                municipality: p.municipality.clone(),
            })
            .collect()
    }

    /// A municipality or reparto named in the text.
    fn area_hint(&self, hints: &[String]) -> Option<Area> {
        for hint in hints {
            if let Some((_, m)) = self.municipalities.iter().find(|(norm, _)| norm == hint) {
                return Some(Area::Municipality(m.clone()));
            }
            if let Some(p) = self.places.iter().find(|p| p.kind == "neighborhood" && &p.norm == hint) {
                return Some(Area::Near(p.point, p.municipality.clone()));
            }
        }
        None
    }
}

enum Area {
    Municipality(String),
    Near(Point, String),
}

impl Area {
    fn contains(&self, c: &Candidate) -> bool {
        match self {
            Area::Municipality(m) => &c.municipality == m,
            Area::Near(p, m) => &c.municipality == m && km_between(*p, c.point) <= NEAR_REPARTO_KM,
        }
    }
}

fn other<'a>(c: &'a Corner, street: &str) -> &'a str {
    if name_matches(&c.a_norm, street) { &c.b } else { &c.a }
}

/// Keeps one candidate per place: corners of wide avenues and repeated
/// places a few metres apart are the same for a client.
fn dedup_nearby(found: &mut Vec<Candidate>) {
    let mut kept: Vec<Candidate> = Vec::new();
    for c in found.drain(..) {
        if !kept.iter().any(|k| km_between(k.point, c.point) <= SAME_CORNER_KM) {
            kept.push(c);
        }
    }
    *found = kept;
}

fn kind_label(kind: &str) -> String {
    kind.replace('_', " ")
}

/// "Calle 23" → "23", for the short corner shown in the channel.
fn short(name: &str) -> &str {
    name.strip_prefix("Calle ").unwrap_or(name)
}

#[derive(Debug, Clone, PartialEq)]
enum AddressKind {
    Between { street: String, a: String, b: String },
    Corner { street: String, cross: String },
    Place(String),
}

#[derive(Debug, Clone, PartialEq)]
struct Address {
    kind: AddressKind,
    /// What came after commas: maybe a reparto or municipality.
    hints: Vec<String>,
}

/// Splits a written address into its parts, all normalized.
fn parse(text: &str) -> Address {
    let lowered = text.to_lowercase();
    let mut parts = lowered.split([',', ';', '\n']);
    let main = parts.next().unwrap_or_default();
    let hints: Vec<String> = parts.map(normalize).filter(|h| !h.is_empty()).collect();

    // Drop house numbers: "#123", "no. 123", "número 123".
    let main = HOUSE_NUMBER.iter().fold(main.to_owned(), |acc, marker| match acc.find(marker) {
        Some(at) => {
            let rest = &acc[at + marker.len()..];
            let end = rest.find(|ch: char| ch.is_alphabetic() && !ch.is_ascii_digit()).unwrap_or(rest.len());
            format!("{} {}", &acc[..at], &rest[end..])
        }
        None => acc,
    });
    let main = format!(" {} ", main.replace('/', "/ ").replace('.', ". "));

    for marker in [" e/ ", " entre ", " e ", " e. "] {
        if let Some((street, rest)) = main.split_once(marker)
            && let Some((a, b)) = rest.split_once(" y ")
        {
            return Address {
                kind: AddressKind::Between { street: normalize(street), a: normalize(a), b: normalize(b) },
                hints,
            };
        }
    }
    for marker in [" esquina a ", " esquina ", " esq. ", " esq ", " y "] {
        if let Some((street, cross)) = main.split_once(marker) {
            let (street, cross) = (normalize(street), normalize(cross));
            if !street.is_empty() && !cross.is_empty() {
                return Address { kind: AddressKind::Corner { street, cross }, hints };
            }
        }
    }
    Address { kind: AddressKind::Place(normalize(&main)), hints }
}

const HOUSE_NUMBER: [&str; 4] = ["#", "no. ", "número ", "numero "];

/// Words that do not tell streets apart.
const GENERIC: [&str; 18] = [
    "calle", "avenida", "ave", "av", "calzada", "calz", "de", "del", "la", "el", "los", "las", "frente", "al", "a",
    "cerca", "reparto", "rpto",
];

/// Lowercase, no accents or punctuation, without generic words, and with
/// ordinals as numbers: "Calle 23" → "23", "5ta Avenida" → "5",
/// "Calzada de Infanta" → "infanta", "Quinta" → "5".
pub fn normalize(text: &str) -> String {
    let plain: String = text
        .to_lowercase()
        .chars()
        .map(|ch| match ch {
            'á' | 'à' | 'ä' => 'a',
            'é' | 'è' | 'ë' => 'e',
            'í' | 'ì' | 'ï' => 'i',
            'ó' | 'ò' | 'ö' => 'o',
            'ú' | 'ù' | 'ü' => 'u',
            'ñ' => 'n',
            c if c.is_alphanumeric() => c,
            _ => ' ',
        })
        .collect();
    let words: Vec<String> = plain.split_whitespace().map(ordinal).collect();
    let meaningful: Vec<&String> = words.iter().filter(|w| !GENERIC.contains(&w.as_str())).collect();
    // "Paseo" or "La Rampa" alone are names: keep the words if all are generic.
    if meaningful.is_empty() {
        return words.join(" ");
    }
    meaningful.iter().map(|w| w.as_str()).collect::<Vec<_>>().join(" ")
}

/// "5ta" → "5", "1ra" → "1", "quinta" → "5".
fn ordinal(word: &str) -> String {
    const WORDS: [(&str, &str); 10] = [
        ("primera", "1"),
        ("segunda", "2"),
        ("tercera", "3"),
        ("cuarta", "4"),
        ("quinta", "5"),
        ("sexta", "6"),
        ("septima", "7"),
        ("octava", "8"),
        ("novena", "9"),
        ("decima", "10"),
    ];
    if let Some((_, n)) = WORDS.iter().find(|(w, _)| *w == word) {
        return (*n).to_owned();
    }
    let digits: String = word.chars().take_while(char::is_ascii_digit).collect();
    let suffix = &word[digits.len()..];
    if !digits.is_empty() && ["ra", "da", "ta", "na", "ma", "va", "o", "a", "er"].contains(&suffix) {
        return digits;
    }
    word.to_owned()
}

/// A written name matches an OSM name if they are equal; if all the written
/// words appear in it ("lazaro" ⊂ "san lazaro"); or, for a number, if the
/// OSM name starts with it ("23" ⊂ "23 rampa", but not "230").
fn name_matches(osm: &str, written: &str) -> bool {
    if written.is_empty() {
        return false;
    }
    if osm == written {
        return true;
    }
    let osm_words: Vec<&str> = osm.split(' ').collect();
    if written.chars().all(|c| c.is_ascii_digit()) {
        return osm_words.first() == Some(&written);
    }
    written.split(' ').all(|w| osm_words.contains(&w))
}

/// Two OSM names of the same street, as where a street changes name.
fn same_street(a: &str, b: &str) -> bool {
    name_matches(a, b) || name_matches(b, a)
}

/// Equirectangular approximation: accurate enough within a city.
pub fn km_between(a: Point, b: Point) -> f64 {
    const EARTH_KM: f64 = 6371.0;
    let mean_lat = ((a.lat + b.lat) / 2.0).to_radians();
    let x = (b.lon - a.lon).to_radians() * mean_lat.cos();
    let y = (b.lat - a.lat).to_radians();
    (x * x + y * y).sqrt() * EARTH_KM
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_street_names() {
        assert_eq!(normalize("Calle 23"), "23");
        assert_eq!(normalize("5ta Avenida"), "5");
        assert_eq!(normalize("Quinta Avenida"), "5");
        assert_eq!(normalize("Calzada de Infanta"), "infanta");
        assert_eq!(normalize("Calle San Lázaro"), "san lazaro");
        assert_eq!(normalize("Paseo"), "paseo");
        assert_eq!(normalize("Calle L"), "l");
        assert_eq!(normalize("1ra"), "1");
    }

    #[test]
    fn parses_cuban_address_forms() {
        let between = AddressKind::Between { street: "23".into(), a: "l".into(), b: "m".into() };
        assert_eq!(parse("23 e/ L y M").kind, between);
        assert_eq!(parse("Calle 23 entre L y M, Vedado").kind, between);
        assert_eq!(parse("Calle 23 #1052 e/ L y M").kind, between);
        assert_eq!(parse("Calle 23 entre L y M, Vedado").hints, vec!["vedado"]);
        let corner = AddressKind::Corner { street: "infanta".into(), cross: "san lazaro".into() };
        assert_eq!(parse("Infanta esq. San Lázaro").kind, corner);
        assert_eq!(parse("infanta y san lazaro").kind, corner);
        assert_eq!(parse("frente al Coppelia").kind, AddressKind::Place("coppelia".into()));
    }

    #[test]
    fn names_match_by_words() {
        assert!(name_matches("san lazaro", "lazaro"));
        assert!(name_matches("23", "23"));
        assert!(!name_matches("230", "23")); // numbers match whole
        assert!(name_matches("23 rampa", "23"));
        assert!(same_street("23", "23 rampa"));
        assert!(!same_street("23", "l"));
        assert!(!same_street("san lazaro", "san rafael"));
        assert!(!name_matches("infanta", ""));
    }

    async fn havana() -> Streets {
        Streets::load(concat!(env!("CARGO_MANIFEST_DIR"), "/data/streets.sqlite")).await.unwrap()
    }

    #[tokio::test]
    async fn finds_corners_blocks_and_places_in_havana() {
        let s = havana().await;

        let found = s.candidates("23 e/ L y M, Vedado");
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].municipality, "Plaza de la Revolución");

        let found = s.candidates("Infanta esq. San Lázaro");
        assert!(!found.is_empty());
        assert!(found.iter().all(|c| c.label.contains("Infanta")));

        // Several "Calle 4 entre 5 y 7" in Havana: the reparto picks Miramar's.
        assert!(s.candidates("4 entre 5 y 7").len() > 1);
        let miramar = s.candidates("4 entre 5 y 7, Miramar");
        assert_eq!(miramar.len(), 1, "{miramar:?}");
        assert_eq!(miramar[0].municipality, "Playa");
        // Alamar has no such street in OSM: nothing rather than another one.
        assert!(s.candidates("4 entre 5 y 7, Alamar").is_empty());

        let found = s.candidates("hospital ameijeiras");
        assert!(found.iter().any(|c| c.label.contains("Hermanos Ameijeiras") && c.municipality == "Centro Habana"));

        assert!(s.candidates("la casa azul detrás del policlínico").is_empty());
    }

    #[tokio::test]
    async fn nearest_corner_of_a_pin() {
        let s = havana().await;
        // About 20 m from the corner of 23 and L (by Coppelia).
        assert_eq!(s.nearest_corner(Point { lat: 23.1397, lon: -82.3832 }).as_deref(), Some("23 y L"));
        // In the sea: no corner.
        assert_eq!(s.nearest_corner(Point { lat: 23.2, lon: -82.4 }), None);
    }
}
