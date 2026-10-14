//! A point given as coordinates or as a map link instead of a pin:
//! - written coordinates: "23.168, -82.277" or `23°10'04.9"N 82°16'36.6"W`;
//! - full map links that carry them (Google Maps, OpenStreetMap, Apple Maps);
//! - short Google links (`maps.app.goo.gl/…`), which carry nothing: the server
//!   asks Google where they redirect (one request, 5 s) and reads that.
//!
//! These texts are locations: they are never logged.

use std::time::Duration;

use reqwest::redirect::Policy;

use crate::domain::request::Point;

/// Redirects followed for a short link (short link → map URL, maybe a consent page).
const MAX_HOPS: usize = 3;

#[derive(Clone)]
pub struct MapLinks {
    http: reqwest::Client,
}

impl MapLinks {
    pub fn new() -> anyhow::Result<MapLinks> {
        let http = reqwest::Client::builder().redirect(Policy::none()).timeout(Duration::from_secs(5)).build()?;
        Ok(MapLinks { http })
    }

    /// The point in the text, if it is coordinates or a map link. `Ok(None)`
    /// when the text is neither (then it may be a written address).
    pub async fn point(&self, text: &str) -> anyhow::Result<Option<Point>> {
        if let Some(url) = first_url(text) {
            if let Some(p) = point_in_url(url) {
                return Ok(Some(p));
            }
            if is_short_google_link(url) {
                return self.follow(url).await;
            }
            return Ok(None);
        }
        Ok(coordinates(text))
    }

    /// Follows the short link's redirects, reading coordinates at each hop.
    async fn follow(&self, url: &str) -> anyhow::Result<Option<Point>> {
        let mut url = url.to_owned();
        for _ in 0..MAX_HOPS {
            let response = self.http.get(&url).send().await.map_err(|e| e.without_url())?;
            let Some(next) = response.headers().get(reqwest::header::LOCATION).and_then(|v| v.to_str().ok()) else {
                return Ok(None);
            };
            let next = consent_target(next).unwrap_or_else(|| next.to_owned());
            if let Some(p) = point_in_url(&next) {
                return Ok(Some(p));
            }
            url = next;
        }
        Ok(None)
    }
}

fn first_url(text: &str) -> Option<&str> {
    text.split_whitespace().find(|w| w.starts_with("http://") || w.starts_with("https://"))
}

fn is_short_google_link(url: &str) -> bool {
    let host = url.split("://").nth(1).unwrap_or_default().split('/').next().unwrap_or_default();
    host == "maps.app.goo.gl" || host == "goo.gl"
}

/// Google's consent page wraps the real address in `continue=`.
fn consent_target(url: &str) -> Option<String> {
    if !url.contains("consent.google.") {
        return None;
    }
    let parsed = reqwest::Url::parse(url).ok()?;
    parsed.query_pairs().find(|(k, _)| k == "continue").map(|(_, v)| v.into_owned())
}

/// Coordinates inside a map URL, in the forms the big map sites use.
pub fn point_in_url(url: &str) -> Option<Point> {
    let decoded = percent_decode(url);
    // Google's data parameters are the place itself; "@" is the camera.
    if let (Some(lat), Some(lon)) = (after(&decoded, "!3d"), after(&decoded, "!4d")) {
        return valid(lat, lon);
    }
    for marker in ["q=", "query=", "ll=", "destination=", "/place/", "/search/", "@"] {
        if let Some(rest) = decoded.split(marker).nth(1)
            && let Some(p) = pair(rest, ',')
        {
            return Some(p);
        }
    }
    // OpenStreetMap: #map=17/23.168/-82.277, or mlat=..&mlon=..
    if let Some(rest) = decoded.split("#map=").nth(1) {
        let mut parts = rest.split('/').skip(1);
        if let (Some(lat), Some(lon)) = (parts.next().and_then(number), parts.next().and_then(number)) {
            return valid(lat, lon);
        }
    }
    if let (Some(lat), Some(lon)) = (after(&decoded, "mlat="), after(&decoded, "mlon=")) {
        return valid(lat, lon);
    }
    None
}

/// Written coordinates: decimal ("23.168, -82.277") or degrees, minutes and
/// seconds (`23°10'04.9"N 82°16'36.6"W`).
pub fn coordinates(text: &str) -> Option<Point> {
    let text = text.trim();
    if text.contains('°') {
        return dms(text);
    }
    pair(text, ',').or_else(|| pair(text, ' '))
}

fn dms(text: &str) -> Option<Point> {
    let upper = text.to_uppercase();
    let split_at = upper.find(['N', 'S'])? + 1;
    let (lat_part, lon_part) = upper.split_at(split_at);
    let lat = dms_value(lat_part)?;
    let lon = dms_value(lon_part)?;
    valid(lat, lon)
}

/// "23°10'04.9"N" → 23.168; south and west are negative.
fn dms_value(part: &str) -> Option<f64> {
    let numbers: Vec<f64> = part
        .split(|c: char| !(c.is_ascii_digit() || c == '.'))
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect();
    let value = match numbers[..] {
        [d, m, s, ..] => d + m / 60.0 + s / 3600.0,
        [d, m] => d + m / 60.0,
        [d] => d,
        _ => return None,
    };
    let negative = part.contains('S') || part.contains('W') || part.contains('O');
    Some(if negative { -value } else { value })
}

/// "23.168,-82.277…" at the start of `text`.
fn pair(text: &str, separator: char) -> Option<Point> {
    let mut parts = text.trim().splitn(2, separator);
    let lat = number(parts.next()?.trim())?;
    let rest = parts.next()?.trim_start();
    let end = rest.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+')).unwrap_or(rest.len());
    let lon = number(&rest[..end])?;
    valid(lat, lon)
}

fn after(text: &str, marker: &str) -> Option<f64> {
    let rest = text.split(marker).nth(1)?;
    let end = rest.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-')).unwrap_or(rest.len());
    number(&rest[..end])
}

fn number(text: &str) -> Option<f64> {
    let text = text.trim_start_matches('+');
    // A coordinate has a decimal point; "23" alone is more likely a street.
    if !text.contains('.') {
        return None;
    }
    text.parse().ok()
}

fn valid(lat: f64, lon: f64) -> Option<Point> {
    ((-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon)).then_some(Point { lat, lon })
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(byte) = u8::from_str_radix(&text[i + 1..i + 3], 16)
        {
            out.push(byte);
            i += 3;
            continue;
        }
        out.push(if bytes[i] == b'+' { b' ' } else { bytes[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Coppelia, Vedado: 23.1396, -82.3834 (a public landmark, not a person's place).
    fn near_coppelia(p: Option<Point>) -> bool {
        p.is_some_and(|p| (p.lat - 23.1396).abs() < 0.001 && (p.lon + 82.3834).abs() < 0.001)
    }

    #[test]
    fn written_coordinates() {
        assert!(near_coppelia(coordinates("23.1396, -82.3834")));
        assert!(near_coppelia(coordinates("23.1396 -82.3834")));
        assert!(near_coppelia(coordinates("23°08'22.6\"N 82°23'00.2\"W")));
        assert!(near_coppelia(coordinates("23°8'22.6\" N, 82°23'0.2\" O")));
        // A street, not coordinates.
        assert_eq!(coordinates("23 y L"), None);
        assert_eq!(coordinates("200.5, 10.2"), None);
    }

    #[test]
    fn full_map_links() {
        assert!(near_coppelia(point_in_url("https://www.google.com/maps/@23.1396,-82.3834,17z")));
        assert!(near_coppelia(point_in_url("https://www.google.com/maps?q=23.1396,-82.3834")));
        assert!(near_coppelia(point_in_url("https://www.google.com/maps/search/?api=1&query=23.1396%2C-82.3834")));
        assert!(near_coppelia(point_in_url(
            "https://www.google.com/maps/place/Coppelia/@23.13,-82.38,17z/data=!3m1!4b1!4m6!3m5!8m2!3d23.1396!4d-82.3834"
        )));
        assert!(near_coppelia(point_in_url("https://www.openstreetmap.org/#map=18/23.1396/-82.3834")));
        assert!(near_coppelia(point_in_url("https://www.openstreetmap.org/?mlat=23.1396&mlon=-82.3834")));
        assert!(near_coppelia(point_in_url("https://maps.apple.com/?ll=23.1396,-82.3834&q=Coppelia")));
        assert_eq!(point_in_url("https://maps.app.goo.gl/abc123"), None);
    }

    #[test]
    fn short_links_and_consent_pages() {
        assert!(is_short_google_link("https://maps.app.goo.gl/abc123"));
        assert!(!is_short_google_link("https://www.google.com/maps/@23.1,-82.3,17z"));
        let wrapped = "https://consent.google.com/m?continue=https://www.google.com/maps/%4023.1396,-82.3834,17z&gl=ES";
        assert!(near_coppelia(consent_target(wrapped).as_deref().and_then(point_in_url)));
    }

    /// Calls Google: `MAPLINK_URL=https://maps.app.goo.gl/… cargo test maplink -- --ignored`.
    /// Prints nothing about the point: only whether it fell in Havana.
    #[tokio::test]
    #[ignore]
    async fn follows_a_real_short_link() {
        let url = std::env::var("MAPLINK_URL").expect("MAPLINK_URL");
        let point = MapLinks::new().unwrap().point(&url).await.unwrap().expect("no point in the link");
        let zones = crate::location::zones::Zones::bundled().unwrap();
        assert!(zones.municipality(point).is_some(), "the point is outside Havana");
    }
}
