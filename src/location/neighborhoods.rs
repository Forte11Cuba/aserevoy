//! Approximate zone of a pin: the nearest neighborhood from a static list
//! (`data/neighborhoods.csv`), so the channel never shows exact locations.

use anyhow::{Context, bail};

use crate::domain::request::Point;

/// Pins farther than this from every neighborhood get no name, rather than a
/// misleading one.
const MAX_KM: f64 = 5.0;

#[derive(Debug, Clone, PartialEq)]
struct Neighborhood {
    name: String,
    point: Point,
}

#[derive(Debug, Clone, Default)]
pub struct Neighborhoods(Vec<Neighborhood>);

impl Neighborhoods {
    /// The list compiled into the binary.
    pub fn bundled() -> anyhow::Result<Neighborhoods> {
        Neighborhoods::parse(include_str!("../../data/neighborhoods.csv")).context("data/neighborhoods.csv")
    }

    /// `name,lat,lon` per line; `#` comments and the header are skipped.
    fn parse(csv: &str) -> anyhow::Result<Neighborhoods> {
        let mut list = Vec::new();
        for (n, line) in csv.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line == "name,lat,lon" {
                continue;
            }
            let fields: Vec<&str> = line.split(',').map(str::trim).collect();
            let [name, lat, lon] = fields[..] else {
                bail!("line {}: expected name,lat,lon", n + 1);
            };
            let point = Point {
                lat: lat.parse().with_context(|| format!("line {}: bad latitude", n + 1))?,
                lon: lon.parse().with_context(|| format!("line {}: bad longitude", n + 1))?,
            };
            list.push(Neighborhood { name: name.to_owned(), point });
        }
        Ok(Neighborhoods(list))
    }

    pub fn nearest(&self, p: Point) -> Option<&str> {
        self.0
            .iter()
            .map(|n| (km_between(p, n.point), n))
            .filter(|(km, _)| *km <= MAX_KM)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, n)| n.name.as_str())
    }
}

/// Equirectangular approximation: accurate enough within a city.
fn km_between(a: Point, b: Point) -> f64 {
    const EARTH_KM: f64 = 6371.0;
    let mean_lat = ((a.lat + b.lat) / 2.0).to_radians();
    let x = (b.lon - a.lon).to_radians() * mean_lat.cos();
    let y = (b.lat - a.lat).to_radians();
    (x * x + y * y).sqrt() * EARTH_KM
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "# test\nname,lat,lon\nNorth, 23.14, -82.38\nSouth,23.10,-82.38\n";

    #[test]
    fn picks_the_nearest_within_range() {
        let list = Neighborhoods::parse(SAMPLE).unwrap();
        assert_eq!(list.nearest(Point { lat: 23.135, lon: -82.381 }), Some("North"));
        assert_eq!(list.nearest(Point { lat: 23.105, lon: -82.379 }), Some("South"));
        assert_eq!(list.nearest(Point { lat: 22.0, lon: -80.0 }), None);
    }

    #[test]
    fn rejects_malformed_lines_and_bundled_list_parses() {
        assert!(Neighborhoods::parse("Vedado,23.1").is_err());
        assert!(Neighborhoods::parse("Vedado,x,-82.4").is_err());
        Neighborhoods::bundled().unwrap();
    }

    #[test]
    fn distance_is_about_right() {
        // 0.01° of latitude ≈ 1.11 km.
        let km = km_between(Point { lat: 23.13, lon: -82.38 }, Point { lat: 23.14, lon: -82.38 });
        assert!((km - 1.11).abs() < 0.01);
    }
}
