use std::collections::HashSet;

use anyhow::{anyhow, Result};
use serde::Deserialize;

use crate::rsc;

const HOMEPAGE: &str = "https://artificialanalysis.ai/";

#[derive(Debug, Clone, Deserialize)]
pub struct Model {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub model_creators: Option<Creator>,
    #[serde(default)]
    pub intelligence_index: Option<f64>,
    #[serde(default, rename = "timescaleData")]
    pub timescale: Option<Timescale>,
    #[serde(default)]
    pub price_1m_blended_3_to_1: Option<f64>,
    #[serde(default)]
    pub context_window_tokens: Option<u64>,
    #[serde(default)]
    pub release_date: Option<String>,
    #[serde(default)]
    pub is_open_weights: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Creator {
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Timescale {
    #[serde(default)]
    pub median_output_speed: Option<f64>,
}

impl Model {
    pub fn provider(&self) -> &str {
        self.model_creators
            .as_ref()
            .map(|c| c.name.as_str())
            .unwrap_or("?")
    }

    pub fn speed(&self) -> Option<f64> {
        self.timescale.as_ref().and_then(|t| t.median_output_speed)
    }
}

pub fn fetch() -> Result<Vec<Model>> {
    let html = rsc::fetch_html(HOMEPAGE)?;
    parse(&html)
}

pub fn parse(html: &str) -> Result<Vec<Model>> {
    let stream = rsc::extract_stream(html)?;
    let mut out = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for span in rsc::innermost_objects_with(&stream, "\"intelligence_index\":") {
        let Ok(model) = serde_json::from_str::<Model>(span) else {
            continue;
        };
        if model.id.is_empty() || !seen.insert(model.id.clone()) {
            continue;
        }
        out.push(model);
    }

    if out.is_empty() {
        return Err(anyhow!("no model records found in artificialanalysis.ai"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fixture_when_provided() {
        let Ok(path) = std::env::var("LLMPK_HOMEPAGE_FIXTURE") else {
            return;
        };
        let html = std::fs::read_to_string(&path).expect("fixture read");
        let models = parse(&html).expect("parse");
        assert!(models.len() >= 10, "expected >=10 models, got {}", models.len());
    }
}
