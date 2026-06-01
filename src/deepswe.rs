use anyhow::{anyhow, Result};
use serde::Deserialize;

use crate::rsc;

const DEEPSWE_URL: &str = "https://deepswe.datacurve.ai/artifacts/leaderboard-live.json";

#[derive(Debug, Clone, Deserialize)]
pub struct Row {
    pub model: String,
    #[serde(default)]
    pub reasoning_effort: Option<String>,
    #[serde(default)]
    pub pass_rate: Option<f64>,
    #[allow(dead_code)]
    #[serde(default)]
    pub pass_at_1: Option<f64>,
    #[serde(default)]
    pub mean_cost_usd: Option<f64>,
    #[serde(default)]
    pub mean_duration_seconds: Option<f64>,
    #[serde(default)]
    pub mean_agent_steps: Option<f64>,
    #[serde(default)]
    pub mean_input_tokens: Option<f64>,
    #[serde(default)]
    pub mean_output_tokens: Option<f64>,
    #[serde(default)]
    pub n_passed: Option<u64>,
    #[serde(default)]
    pub n_attempted: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
struct Response {
    rows: Vec<Row>,
}

impl Row {
    pub fn display_model(&self) -> String {
        match &self.reasoning_effort {
            Some(e) if !e.is_empty() => format!("{} [{}]", self.model, e),
            _ => self.model.clone(),
        }
    }

    pub fn total_tokens(&self) -> Option<f64> {
        match (self.mean_input_tokens, self.mean_output_tokens) {
            (Some(i), Some(o)) => Some(i + o),
            (Some(i), None) => Some(i),
            (None, Some(o)) => Some(o),
            _ => None,
        }
    }
}

pub fn fetch() -> Result<Vec<Row>> {
    let json = rsc::fetch_html(DEEPSWE_URL)?;
    parse(&json)
}

pub fn parse(json: &str) -> Result<Vec<Row>> {
    let resp: Response = serde_json::from_str(json)
        .map_err(|e| anyhow!("failed to parse DeepSWE leaderboard JSON: {e}"))?;
    if resp.rows.is_empty() {
        return Err(anyhow!("DeepSWE leaderboard rows array empty"));
    }
    Ok(best_per_model(resp.rows))
}

/// Keep only the best (highest pass_rate) config per model.
fn best_per_model(rows: Vec<Row>) -> Vec<Row> {
    use std::collections::HashMap;
    let mut best: HashMap<String, Row> = HashMap::new();
    for row in rows {
        let key = row.model.clone();
        match best.get(&key) {
            Some(existing) if existing.pass_rate >= row.pass_rate => {}
            _ => {
                best.insert(key, row);
            }
        }
    }
    let mut out: Vec<Row> = best.into_values().collect();
    out.sort_by(|a, b| {
        let av = a.pass_rate.unwrap_or(0.0);
        let bv = b.pass_rate.unwrap_or(0.0);
        bv.partial_cmp(&av).unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_fetch_when_enabled() {
        if std::env::var("LLMPK_LIVE").is_err() {
            return;
        }
        let rows = fetch().expect("live DeepSWE fetch");
        assert!(rows.len() > 5);
        assert!(rows.iter().any(|r| r.pass_rate.is_some()));
    }

    #[test]
    fn parses_fixture_when_provided() {
        let Ok(path) = std::env::var("LLMPK_DEEPSWE_FIXTURE") else {
            return;
        };
        let json = std::fs::read_to_string(&path).expect("fixture read");
        let rows = parse(&json).expect("parse");
        assert!(rows.len() >= 5, "expected >=5 rows, got {}", rows.len());
    }

    #[test]
    fn best_per_model_keeps_highest_pass_rate() {
        let rows = vec![
            Row {
                model: "gpt-5".into(),
                reasoning_effort: Some("high".into()),
                pass_rate: Some(0.5),
                pass_at_1: Some(0.5),
                mean_cost_usd: Some(1.0),
                mean_duration_seconds: Some(100.0),
                mean_agent_steps: Some(10.0),
                mean_input_tokens: Some(1000.0),
                mean_output_tokens: Some(500.0),
                n_passed: Some(50),
                n_attempted: Some(100),
            },
            Row {
                model: "gpt-5".into(),
                reasoning_effort: Some("xhigh".into()),
                pass_rate: Some(0.7),
                pass_at_1: Some(0.7),
                mean_cost_usd: Some(2.0),
                mean_duration_seconds: Some(200.0),
                mean_agent_steps: Some(20.0),
                mean_input_tokens: Some(2000.0),
                mean_output_tokens: Some(1000.0),
                n_passed: Some(70),
                n_attempted: Some(100),
            },
        ];
        let result = best_per_model(rows);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].reasoning_effort.as_deref(), Some("xhigh"));
    }

    #[test]
    fn display_model_with_effort() {
        let row = Row {
            model: "gpt-5-5".into(),
            reasoning_effort: Some("xhigh".into()),
            pass_rate: None,
            pass_at_1: None,
            mean_cost_usd: None,
            mean_duration_seconds: None,
            mean_agent_steps: None,
            mean_input_tokens: None,
            mean_output_tokens: None,
            n_passed: None,
            n_attempted: None,
        };
        assert_eq!(row.display_model(), "gpt-5-5 [xhigh]");
    }
}
