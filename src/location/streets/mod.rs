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

mod labels;
mod parse;

use std::str::FromStr;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

use crate::domain::request::Point;

use labels::{PLACE_KINDS, button_label, place_candidate, short};
use parse::{AddressKind, name_matches, normalize, parse, same_street};

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
    /// "suburb" (a big reparto), "neighborhood" (a smaller one) or a landmark type.
    kind: String,
    point: Point,
    municipality: String,
}

impl Place {
    fn is_reparto(&self) -> bool {
        self.kind == "suburb" || self.kind == "neighborhood"
    }
}

/// A point the text may refer to, with a description for Jev and the client.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub label: String,
    pub point: Point,
    pub municipality: String,
    /// Deduced from the cross streets: the written street name is unknown
    /// ("doble vía e/ 160c y 160d"), so the client must pick it.
    pub inferred: bool,
    /// For the button: municipality, reparto, then the rest ("Playa ·
    /// Miramar · 12 entre …"). A button cut at the end still shows what
    /// tells the options apart. See `button_label`.
    pub short: String,
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
        let mut area = self.area_hint(&address.hints);
        let mut found = match &address.kind {
            // "entre 5ta B y 5ta C Avenida de los Cocos": no street before.
            // Or after a comma: "entre 5ta B y 5ta C, Avenida de los Cocos".
            AddressKind::Between { street, a, b } if street.is_empty() => {
                let found = self.street_last(a, b);
                if found.is_empty() {
                    let blocks: Vec<Candidate> =
                        address.hints.iter().flat_map(|street| self.blocks(street, a, b)).collect();
                    // The hint was the street ("Los Cocos" is a reparto too).
                    if !blocks.is_empty() {
                        area = None;
                    }
                    blocks
                } else {
                    found
                }
            }
            AddressKind::Between { street, a, b } => self.between(street, a, b),
            AddressKind::Corner { street, cross } => {
                let corners: Vec<Candidate> =
                    self.corners_of(street, cross).into_iter().map(|c| self.corner_candidate(c)).collect();
                if !corners.is_empty() {
                    corners
                } else {
                    // The street without "entre" ("Calle L 23 y 21"), or the
                    // cross streets first ("5ta B y 5ta C Avenida de los
                    // Cocos"); else two parallel streets ("21 D y 21 C")
                    // mean the block of one crossing both.
                    let mut blocks = self.street_first(street, cross);
                    blocks.extend(self.street_last(street, cross));
                    if blocks.is_empty() { self.crossing_both(street, cross) } else { blocks }
                }
            }
            AddressKind::Place(name) => self.places_in_text(name),
        };
        // A named municipality or reparto rules out places elsewhere, even if
        // none is left: better to ask for the pin than to suggest the wrong one.
        if let Some(area) = &area {
            found.retain(|c| area.contains(c));
        }
        // A hint that is no area may be the street itself ("5 y Calzada,
        // Avenida de los Presidentes"): if some places name it, keep those.
        if area.is_none() {
            for hint in &address.hints {
                let names_it = |c: &Candidate| {
                    let label = normalize(&c.label);
                    let words: Vec<&str> = label.split(' ').collect();
                    hint.split(' ').all(|w| words.contains(&w))
                };
                if found.iter().any(names_it) {
                    found.retain(names_it);
                }
            }
        }
        // Parallel streets deduced from cross streets are close but distinct.
        if !found.iter().any(|c| c.inferred) {
            dedup_nearby(&mut found);
        }
        merge_same_label(&mut found);
        found.truncate(MAX_CANDIDATES);
        // When all are in one municipality, it tells nothing apart.
        let one_municipality = found.windows(2).all(|w| w[0].municipality == w[1].municipality);
        for c in &mut found {
            let reparto = self.nearest_reparto(c.point, &c.municipality);
            c.short = button_label(c, reparto.as_deref(), !one_municipality);
        }
        found
    }

    /// The reparto people would name: the nearest big one (Vedado, Miramar,
    /// Alamar) in the same municipality within 3.5 km, else the nearest small
    /// one. OSM has a point per reparto, not its limits.
    fn nearest_reparto(&self, p: Point, municipality: &str) -> Option<String> {
        let nearest = |big: bool| {
            self.places
                .iter()
                .filter(|r| r.is_reparto() && (r.kind == "suburb") == big && r.municipality == municipality)
                .map(|r| (km_between(p, r.point), r))
                .filter(|(km, _)| *km <= NEAR_REPARTO_KM)
                .min_by(|x, y| x.0.total_cmp(&y.0))
                .map(|(_, r)| r.name.clone())
        };
        nearest(true).or_else(|| nearest(false))
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
            inferred: false,
            short: String::new(),
        }
    }

    /// "X A y B": the street, then the cross streets without "entre". Every
    /// cut of "X A" that names a real block counts ("l 23" → "l" and "23").
    fn street_first(&self, rest: &str, b: &str) -> Vec<Candidate> {
        let words: Vec<&str> = rest.split(' ').collect();
        (1..words.len())
            .flat_map(|cut| {
                let (street, a) = (words[..cut].join(" "), words[cut..].join(" "));
                self.blocks(&street, &a, b)
            })
            .collect()
    }

    /// "A y B X": the street comes after the cross streets, glued to the
    /// second one. Every way of cutting "B X" that names a real block counts
    /// ("5 c cocos" → "5 c" and "cocos"); several are offered as choices.
    fn street_last(&self, a: &str, rest: &str) -> Vec<Candidate> {
        let words: Vec<&str> = rest.split(' ').collect();
        (1..words.len())
            .flat_map(|cut| {
                let (b, street) = (words[..cut].join(" "), words[cut..].join(" "));
                self.blocks(&street, a, &b)
            })
            .collect()
    }

    /// The middle of each block of `street` between its corners with `a` and `b`.
    fn blocks(&self, street: &str, a: &str, b: &str) -> Vec<Candidate> {
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
                        inferred: false,
                        short: String::new(),
                    });
                }
            }
        }
        found
    }

    /// The middle of the block between the two corners, or one of the corners
    /// if only one is known.
    fn between(&self, street: &str, a: &str, b: &str) -> Vec<Candidate> {
        let (first, second) = (self.corners_of(street, a), self.corners_of(street, b));
        let mut found = self.blocks(street, a, b);
        if found.is_empty() && first.is_empty() && second.is_empty() {
            // The street's name is unknown (a popular one, like "Doble Vía"):
            // it must be one that crosses both cross streets on one block.
            return self.crossing_both(a, b);
        }
        if found.is_empty() {
            found = first.into_iter().chain(second).map(|c| self.corner_candidate(c)).collect();
        }
        found
    }

    /// The blocks of every street that crosses both `a` and `b` close together.
    fn crossing_both(&self, a: &str, b: &str) -> Vec<Candidate> {
        let side = |c: &Corner, name: &str| -> Option<(String, String)> {
            if name_matches(&c.a_norm, name) {
                Some((c.b.clone(), c.b_norm.clone()))
            } else if name_matches(&c.b_norm, name) {
                Some((c.a.clone(), c.a_norm.clone()))
            } else {
                None
            }
        };
        let mut found: Vec<Candidate> = Vec::new();
        let mut seen: Vec<(String, String)> = Vec::new();
        for x in &self.corners {
            let Some((street, street_norm)) = side(x, a) else { continue };
            if name_matches(&street_norm, b) || seen.contains(&(street_norm.clone(), x.municipality.clone())) {
                continue;
            }
            let other_end = self.corners.iter().find(|y| {
                y.municipality == x.municipality
                    && km_between(x.point, y.point) <= BLOCK_MAX_KM
                    && side(y, b).is_some_and(|(_, norm)| norm == street_norm)
            });
            if let Some(y) = other_end {
                let label = format!("{street} entre {} y {} en {}", other(x, &street_norm), other(y, &street_norm), x.municipality);
                seen.push((street_norm, x.municipality.clone()));
                found.push(Candidate {
                    label,
                    point: Point { lat: (x.point.lat + y.point.lat) / 2.0, lon: (x.point.lon + y.point.lon) / 2.0 },
                    municipality: x.municipality.clone(),
                    inferred: true,
                    short: String::new(),
                });
            }
        }
        found
    }

    /// A place by name; if none is called that, the name may end with an
    /// area ("coppelia vedado") or carry extra words ("heladería coppelia").
    fn places_in_text(&self, name: &str) -> Vec<Candidate> {
        let found = self.places_named(name);
        if !found.is_empty() {
            return found;
        }
        let words: Vec<&str> = name.split(' ').collect();
        for cut in (1..words.len()).rev() {
            if let Some(area) = self.area_hint(&[words[cut..].join(" ")]) {
                let found: Vec<Candidate> =
                    self.places_named(&words[..cut].join(" ")).into_iter().filter(|c| area.contains(c)).collect();
                if !found.is_empty() {
                    return found;
                }
            }
        }
        // Without the kind of place, which OSM may leave out of the name.
        let bare: Vec<&str> = words.iter().copied().filter(|w| !PLACE_KINDS.contains(w)).collect();
        if bare.is_empty() || bare.len() == words.len() {
            return Vec::new();
        }
        self.places_in_text(&bare.join(" "))
    }

    fn places_named(&self, name: &str) -> Vec<Candidate> {
        if name.is_empty() {
            return Vec::new();
        }
        let mut found: Vec<&Place> =
            self.places.iter().filter(|p| !p.is_reparto() && name_matches(&p.norm, name)).collect();
        // Exact names first, then shorter ones (closer to what was written).
        found.sort_by_key(|p| (p.norm != name, p.norm.len()));
        found.into_iter().map(place_candidate).collect()
    }

    /// The repartos of a municipality, by name (one point per name).
    pub fn repartos_in(&self, municipality: &str) -> Vec<Reparto> {
        let mut found: Vec<Reparto> = Vec::new();
        for p in self.places.iter().filter(|p| p.is_reparto() && p.municipality == municipality) {
            if !found.iter().any(|r| r.name == p.name) {
                found.push(Reparto { name: p.name.clone(), point: p.point });
            }
        }
        found.sort_by(|a, b| a.name.cmp(&b.name));
        found
    }

    /// The repartos of `municipality` the client's words name, best first:
    /// "alamar zona 6" names both "Alamar" and "Zona 6", and the longer name
    /// is more precise, so only "Zona 6" is kept.
    pub fn find_repartos(&self, municipality: &str, text: &str) -> Vec<Reparto> {
        let repartos = self.repartos_in(municipality);
        best_named(&repartos, &normalize(text), |r| normalize(&r.name))
    }

    /// A reparto named in a free description, anywhere in Havana, if exactly
    /// one name stands out ("doble vía e/ 160c y 160d, Alamar").
    pub fn reparto_in_text(&self, text: &str) -> Option<(Reparto, String)> {
        let written = normalize(text);
        let repartos: Vec<(Reparto, String)> = self
            .places
            .iter()
            .filter(|p| p.is_reparto())
            .map(|p| (Reparto { name: p.name.clone(), point: p.point }, p.municipality.clone()))
            .collect();
        let best = best_named(&repartos, &written, |(r, _)| normalize(&r.name));
        let mut names: Vec<&str> = best.iter().map(|(r, _)| r.name.as_str()).collect();
        names.dedup();
        match (best.first(), names.len()) {
            (Some(found), 1) => Some(found.clone()),
            _ => None,
        }
    }

    /// A municipality or reparto named in the text.
    fn area_hint(&self, hints: &[String]) -> Option<Area> {
        for hint in hints {
            if let Some((_, m)) = self.municipalities.iter().find(|(norm, _)| norm == hint) {
                return Some(Area::Municipality(m.clone()));
            }
            if let Some(p) = self.places.iter().find(|p| p.is_reparto() && &p.norm == hint) {
                return Some(Area::Near(p.point, p.municipality.clone()));
            }
        }
        None
    }
}

/// A neighborhood: where a free description is placed when there is no pin.
#[derive(Debug, Clone, PartialEq)]
pub struct Reparto {
    pub name: String,
    pub point: Point,
}

/// The items whose whole name appears among the written words, keeping only
/// those with the most words (the most precise). Names without a word of
/// three letters or more are ignored, so "6" alone never picks "Zona 6".
fn best_named<T: Clone>(items: &[T], written: &str, name: impl Fn(&T) -> String) -> Vec<T> {
    let words: Vec<&str> = written.split(' ').filter(|w| !w.is_empty()).collect();
    let mut scored: Vec<(usize, T)> = items
        .iter()
        .filter_map(|item| {
            let n = name(item);
            let name_words: Vec<&str> = n.split(' ').filter(|w| !w.is_empty()).collect();
            let meaningful = name_words.iter().any(|w| w.len() > 2 && w.chars().any(char::is_alphabetic));
            (meaningful && name_words.iter().all(|w| words.contains(w))).then(|| (name_words.len(), item.clone()))
        })
        .collect();
    let most = scored.iter().map(|(n, _)| *n).max().unwrap_or(0);
    scored.retain(|(n, _)| *n == most);
    scored.into_iter().map(|(_, item)| item).collect()
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

/// Two blocks with the same name close together are one for the client
/// (a cross street that meets the street twice, as 24 de Febrero does
/// Circunvalación around 1ro de Mayo): one option, at their middle.
fn merge_same_label(found: &mut Vec<Candidate>) {
    let mut kept: Vec<(Candidate, usize)> = Vec::new();
    for c in found.drain(..) {
        match kept.iter_mut().find(|(k, _)| k.label == c.label && km_between(k.point, c.point) <= BLOCK_MAX_KM) {
            Some((k, n)) => {
                // Running mean of the merged points.
                *n += 1;
                k.point.lat += (c.point.lat - k.point.lat) / *n as f64;
                k.point.lon += (c.point.lon - k.point.lon) / *n as f64;
            }
            None => kept.push((c, 1)),
        }
    }
    *found = kept.into_iter().map(|(c, _)| c).collect();
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

        // Coppelia as people write it: one "p", with the reparto, or with
        // the kind of place first.
        for text in ["copelia, vedado", "coppelia vedado", "heladeria copelia vedado"] {
            let found = s.candidates(text);
            assert!(found.len() == 1 && found[0].label.starts_with("Coppelia"), "{text}: {found:?}");
        }
        assert!(s.candidates("copelia").len() > 1);

        let found = s.candidates("hospital ameijeiras");
        assert!(found.iter().any(|c| c.label.contains("Hermanos Ameijeiras") && c.municipality == "Centro Habana"));

        assert!(s.candidates("la casa azul detrás del policlínico").is_empty());

        // Found in a live test: 21 D and 21 C are parallel (no corner), and a
        // landmark followed them. The streets crossing both are offered.
        let found = s.candidates("21 d y 21 c por la cafeteria el riki");
        assert!(!found.is_empty() && found.iter().all(|c| c.inferred), "{found:?}");
        assert!(found.iter().any(|c| c.label.contains("Avenida Cuarta")), "{found:?}");

        // Found in a live test: "Doble Vía" is not in OSM, but the streets
        // crossing both 160C and 160D are, Avenida Séptima among them.
        let found = s.candidates("doble via entre 160c y 160d");
        assert!(found.iter().all(|c| c.inferred && c.municipality == "Habana del Este"), "{found:?}");
        assert!(found.iter().any(|c| c.label.contains("Séptima")), "{found:?}");

        // Buttons say municipality, reparto, then the rest: the "12 e/ 3ra
        // y 5ta" of Miramar and of the Vedado are told apart at a glance.
        let found = s.candidates("12 entre 3era y 5ta");
        let short = |m: &str| found.iter().find(|c| c.municipality == m).map(|c| c.short.clone()).unwrap_or_default();
        assert!(short("Playa").starts_with("Playa · Miramar · 12 entre"), "{found:?}");
        assert!(short("Plaza de la Revolución").starts_with("Plaza · Vedado · 12 entre"), "{found:?}");
        assert!(!short("Playa").ends_with("en Playa"), "{found:?}");
        // All in one municipality: it is left out.
        let found = s.candidates("7ma entre 160c y 160d, Alamar");
        assert!(found.iter().all(|c| !c.short.starts_with("Habana del Este")), "{found:?}");

        // Found in a live test: "3era" and "a y" typed as "ay".
        let found = s.candidates("160e entre 3era ay 3b");
        assert!(!found.is_empty(), "{found:?}");
        assert!(found.iter().all(|c| c.municipality == "Habana del Este"));

        // Found in a live test: 24 de Febrero meets Circunvalación on both
        // sides of 1ro de Mayo; two identical options are one.
        let found = s.candidates("circunvalacion entre 1ero de mayo y 24 de febrero");
        assert_eq!(found.len(), 1, "{found:?}");

        // Found in a live test: "Calzada (7)" is Calzada, and Calle 5ta runs
        // parallel to it, so the street is a third one, maybe after a comma.
        let found = s.candidates("calle 5 y calzada, avenida los presidentes");
        assert!(found.len() == 1 && found[0].label.starts_with("Avenida de los Presidentes (Calle G) entre Calle 5ta y Calzada"), "{found:?}");
        assert!(s.candidates("calle 5 y calzada").len() > 1);
        assert_eq!(s.candidates("calzada y L").len(), 1);

        // Found in a live test: the cross streets first, then the street.
        for text in [
            "5ta b y 5ta c avenida de los cocos",
            "entre 5ta b y 5ta c avenida de los cocos",
            "entre 5ta b y 5ta c, avenida de los cocos",
        ] {
            let found = s.candidates(text);
            assert!(found.len() == 1 && found[0].label.starts_with("Avenida de Los Cocos (164) entre 5ta B y 5ta C"), "{found:?}");
            assert!(found[0].short.starts_with("Alamar · "), "{found:?}");
        }
        // Found in a live test: no "entre" between the street and the cross streets.
        let found = s.candidates("calle l 23 y 21");
        assert!(found.len() == 1 && found[0].label.starts_with("Calle L entre"), "{found:?}");
        let found = s.candidates("hotel nacional");
        assert!(found.len() == 1 && found[0].label == "Hotel Nacional en Plaza de la Revolución", "{found:?}");
        assert!(s.candidates("hospital ameijeiras").iter().any(|c| c.label.starts_with("Hospital Hermanos Ameijeiras en")));
        let found = s.candidates("L y M 23");
        assert!(found.len() == 1 && found[0].municipality == "Plaza de la Revolución", "{found:?}");

        // Found in a live test: Alamar's streets are written "160 C" or "160C".
        let found = s.candidates("7ma entre 160c y 160d, Alamar");
        assert!(!found.is_empty(), "{found:?}");
        assert!(found.iter().all(|c| c.municipality == "Habana del Este"));
    }

    #[tokio::test]
    async fn repartos_for_free_descriptions() {
        let s = havana().await;
        let names = |found: Vec<Reparto>| found.into_iter().map(|r| r.name).collect::<Vec<_>>();
        // The more precise name wins: Alamar's zone 6, not all of Alamar.
        assert_eq!(names(s.find_repartos("Habana del Este", "alamar zona 6")), ["Zona 6"]);
        assert_eq!(names(s.find_repartos("Habana del Este", "en cojimar")), ["Cojímar"]);
        assert!(s.find_repartos("Habana del Este", "vedado").is_empty());
        // Small municipalities have few repartos (shown as buttons).
        assert!(s.repartos_in("Centro Habana").len() <= 10);
        // Named in the description itself.
        let (reparto, municipality) = s.reparto_in_text("doble via e/ 160c y 160d, Alamar").unwrap();
        assert_eq!((reparto.name.as_str(), municipality.as_str()), ("Alamar", "Habana del Este"));
        assert!(s.reparto_in_text("la casa azul detrás del policlínico").is_none());
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
