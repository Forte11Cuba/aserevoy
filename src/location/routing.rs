//! Route distance from our own OSRM instance (never the public demo server).

use std::time::Duration;

use anyhow::{Context, bail};
use serde::Deserialize;

use crate::domain::request::Point;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Route {
    pub km: f64,
    pub minutes: f64,
}

#[derive(Clone)]
pub struct Osrm {
    http: reqwest::Client,
    base_url: String,
}

impl Osrm {
    pub fn new(base_url: &str) -> anyhow::Result<Osrm> {
        let http = reqwest::Client::builder().timeout(Duration::from_secs(10)).build()?;
        Ok(Osrm { http, base_url: base_url.trim_end_matches('/').to_owned() })
    }

    pub async fn route(&self, from: Point, to: Point) -> anyhow::Result<Route> {
        // OSRM takes lon,lat.
        let url = format!(
            "{}/route/v1/driving/{},{};{},{}?overview=false",
            self.base_url, from.lon, from.lat, to.lon, to.lat
        );
        let body = self.http.get(url).send().await?.text().await?;
        parse(&body)
    }
}

#[derive(Deserialize)]
struct Response {
    code: String,
    #[serde(default)]
    routes: Vec<RouteJson>,
}

#[derive(Deserialize)]
struct RouteJson {
    /// Meters.
    distance: f64,
    /// Seconds.
    duration: f64,
}

fn parse(body: &str) -> anyhow::Result<Route> {
    let response: Response = serde_json::from_str(body).context("unexpected OSRM response")?;
    if response.code != "Ok" {
        bail!("OSRM answered {}", response.code);
    }
    let route = response.routes.first().context("OSRM returned no route")?;
    Ok(Route { km: route.distance / 1000.0, minutes: route.duration / 60.0 })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_first_route() {
        let body = r#"{"code":"Ok","routes":[{"distance":3210.4,"duration":420.0,"legs":[]}],"waypoints":[]}"#;
        assert_eq!(parse(body).unwrap(), Route { km: 3.2104, minutes: 7.0 });
    }

    #[test]
    fn reports_osrm_errors() {
        assert!(parse(r#"{"code":"NoRoute","message":"Impossible route"}"#).is_err());
        assert!(parse("<html>").is_err());
    }
}
