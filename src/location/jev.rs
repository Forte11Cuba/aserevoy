//! Jev (TypeSafe), optional: picks which candidate a written address refers
//! to. It does not generate text: it chooses among the options we give it
//! (always including "none"), with a probability per option and a confidence
//! for its choice. See `decide` for what the bot does with the answer; on any
//! error the bot asks for the pin instead.
//!
//! API: `POST https://api.typesafe.ai/v1/systemone`, a `choice` question whose
//! options go in `criteria`; the answer has `choice` and `confidence`.

use std::collections::BTreeMap;
use std::time::Duration;

use anyhow::{Context, bail};
use rand::seq::SliceRandom;
use serde::Deserialize;
use serde_json::json;

const URL: &str = "https://api.typesafe.ai/v1/systemone";
/// From this confidence Jev's choice is used directly (measured with Havana
/// addresses: right answers came at 0.76 and above, doubtful ones below 0.5).
pub const MIN_CONFIDENCE: f64 = 0.7;
/// Below that, options with at least this probability are offered as buttons.
pub const MIN_PROBABILITY: f64 = 0.3;
/// At most this many buttons for the likely options.
pub const MAX_OFFERED: usize = 4;
/// When none is likely, all candidates are offered, up to this many.
pub const MAX_ALL: usize = 8;
const NONE: &str = "none";

#[derive(Clone)]
pub struct Jev {
    http: reqwest::Client,
    api_key: String,
}

#[derive(Deserialize)]
struct Response {
    answers: BTreeMap<String, Answer>,
}

#[derive(Deserialize)]
struct Answer {
    choice: String,
    confidence: f64,
    #[serde(default)]
    probabilities: BTreeMap<String, f64>,
}

/// Jev's answer about our options.
#[derive(Debug, Clone, PartialEq)]
pub struct Verdict {
    /// The chosen option, `None` for "none of them".
    pub choice: Option<usize>,
    pub confidence: f64,
    /// Probability of each option, in the order given.
    pub probabilities: Vec<f64>,
}

/// What the bot does with a verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Confident choice: use it (and confirm with "Is it here?").
    Use(usize),
    /// Doubt: let the client pick among these, best first.
    Offer(Vec<usize>),
    /// Not an address we know: ask for the pin or a free description.
    Nothing,
}

/// ≥ 0.7 confidence: Jev's choice (or "nothing" if it chose "none").
/// Otherwise the options with probability ≥ 0.3, at most 4, best first; if
/// none is that likely, all of them (up to 8), best first: the rules found
/// real places and the text just doesn't say which ("12 entre 3ra y 5ta"
/// exists in six municipalities), so the client picks.
pub fn decide(v: &Verdict) -> Decision {
    if v.confidence >= MIN_CONFIDENCE {
        return v.choice.map_or(Decision::Nothing, Decision::Use);
    }
    let mut by_probability: Vec<usize> = (0..v.probabilities.len()).collect();
    by_probability.sort_by(|&a, &b| v.probabilities[b].total_cmp(&v.probabilities[a]));
    let likely: Vec<usize> =
        by_probability.iter().copied().filter(|&i| v.probabilities[i] >= MIN_PROBABILITY).take(MAX_OFFERED).collect();
    if !likely.is_empty() {
        return Decision::Offer(likely);
    }
    by_probability.truncate(MAX_ALL);
    if by_probability.is_empty() { Decision::Nothing } else { Decision::Offer(by_probability) }
}

impl Jev {
    pub fn new(api_key: &str) -> anyhow::Result<Jev> {
        let http = reqwest::Client::builder().timeout(Duration::from_secs(10)).build()?;
        Ok(Jev { http, api_key: api_key.to_owned() })
    }

    /// Which of `options` the client's `text` refers to. The text is the
    /// address as written (no names or phones).
    pub async fn ask(&self, text: &str, options: &[String]) -> anyhow::Result<Verdict> {
        // Jev tends to favour the first option, so the order is shuffled.
        let mut keys: Vec<String> = (0..options.len()).map(|i| format!("o{i}")).collect();
        keys.push(NONE.to_owned());
        keys.shuffle(&mut rand::rng());
        let criteria: serde_json::Map<String, serde_json::Value> = keys
            .iter()
            .map(|k| {
                let description = match k.strip_prefix('o').and_then(|i| i.parse::<usize>().ok()) {
                    Some(i) => options[i].clone(),
                    None => "Ninguna de las otras opciones corresponde a la dirección".to_owned(),
                };
                (k.clone(), json!(description))
            })
            .collect();
        let body = json!({
            "state": { "direccion_escrita_por_el_cliente": text },
            "model": "jev-latest",
            "questions": {
                "lugar": {
                    "type": "choice",
                    "instructions": "Un cliente en La Habana, Cuba, escribió la `direccion_escrita_por_el_cliente`. ¿Cuál de las opciones es el lugar al que se refiere?",
                    "criteria": criteria,
                }
            }
        });
        let response = self
            .http
            .post(URL)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| e.without_url())?;
        let status = response.status();
        if !status.is_success() {
            bail!("Jev answered {status}");
        }
        let response: Response = response.json().await.map_err(|e| e.without_url())?;
        let answer = response.answers.get("lugar").context("Jev gave no answer")?;
        let index = answer.choice.strip_prefix('o').and_then(|i| i.parse::<usize>().ok()).filter(|i| *i < options.len());
        let probabilities = (0..options.len()).map(|i| answer.probabilities.get(&format!("o{i}")).copied().unwrap_or(0.0)).collect();
        Ok(Verdict { choice: index, confidence: answer.confidence, probabilities })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn verdict(choice: Option<usize>, confidence: f64, probabilities: &[f64]) -> Verdict {
        Verdict { choice, confidence, probabilities: probabilities.to_vec() }
    }

    #[test]
    fn confident_choices_are_used() {
        assert_eq!(decide(&verdict(Some(1), 0.87, &[0.05, 0.9, 0.05])), Decision::Use(1));
        assert_eq!(decide(&verdict(None, 0.99, &[0.0, 0.01, 0.0])), Decision::Nothing);
    }

    #[test]
    fn doubts_offer_the_likely_options_best_first() {
        // Three Coppelias and no zone: the client picks.
        let v = verdict(None, 0.4, &[0.35, 0.31, 0.1]);
        assert_eq!(decide(&v), Decision::Offer(vec![0, 1]));
        let v = verdict(Some(2), 0.5, &[0.3, 0.05, 0.6]);
        assert_eq!(decide(&v), Decision::Offer(vec![2, 0]));
        // Spread thin: all of them, best first (found in the live test with
        // "12 entre 3era y 5ta": six real places, Jev unsure, "none" at 0.5).
        let v = verdict(None, 0.44, &[0.08, 0.17, 0.02, 0.02, 0.15, 0.04]);
        assert_eq!(decide(&v), Decision::Offer(vec![1, 4, 0, 5, 2, 3]));
        // But a confident "none" is nothing.
        assert_eq!(decide(&verdict(None, 0.9, &[0.02, 0.03])), Decision::Nothing);
    }

    /// Calls the real API: `TYPESAFE_API_KEY=… cargo test jev -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn picks_the_right_street_among_namesakes() {
        let jev = Jev::new(&std::env::var("TYPESAFE_API_KEY").expect("TYPESAFE_API_KEY")).unwrap();
        let options = vec![
            "Calle 23 entre L y M en Plaza de la Revolución".to_owned(),
            "Calle 23 en Playa".to_owned(),
            "Avenida 23 en Playa".to_owned(),
        ];
        let v = jev.ask("23 e/ L y M, Vedado", &options).await.unwrap();
        assert_eq!(decide(&v), Decision::Use(0), "{v:?}");
        let v = jev.ask("la casa azul detrás del policlínico", &options).await.unwrap();
        assert_eq!(decide(&v), Decision::Nothing, "{v:?}");
    }
}
