use std::collections::HashSet;

use anyhow::{anyhow, Result};
use serde::Deserialize;

use crate::rsc;

const HOMEPAGE: &str = "https://artificialanalysis.ai/";

#[derive(Debug, Clone, Deserialize)]
pub struct Model {
    pub id: String,
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default)]
    pub name: String,
    #[serde(default, alias = "modelCreators", alias = "creator")]
    pub model_creators: Option<Creator>,
    #[serde(default, alias = "intelligenceIndex")]
    pub intelligence_index: Option<f64>,
    #[serde(default, rename = "timescaleData")]
    pub timescale: Option<Timescale>,
    #[serde(default, alias = "price1mBlended3To1", alias = "price1mBlended0To3To1")]
    pub price_1m_blended_3_to_1: Option<f64>,
    #[serde(default, alias = "contextWindowTokens")]
    pub context_window_tokens: Option<u64>,
    #[serde(default, alias = "releaseDate")]
    pub release_date: Option<String>,
    #[serde(default, alias = "isOpenWeights")]
    pub is_open_weights: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Creator {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub color: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Timescale {
    #[serde(default, alias = "medianOutputSpeed")]
    pub median_output_speed: Option<f64>,
}

impl Model {
    pub fn provider(&self) -> &str {
        self.model_creators
            .as_ref()
            .map(|c| c.name.as_str())
            .unwrap_or("?")
    }

    pub fn provider_color(&self) -> Option<&str> {
        self.model_creators
            .as_ref()
            .and_then(|c| c.color.as_deref())
    }

    pub fn speed(&self) -> Option<f64> {
        self.timescale.as_ref().and_then(|t| t.median_output_speed)
    }

    /// Returns the human-readable identifier (slug) or falls back to id.
    pub fn display_id(&self) -> &str {
        self.slug.as_deref().unwrap_or(&self.id)
    }
}

pub fn fetch() -> Result<Vec<Model>> {
    let html = rsc::fetch_text_retry(HOMEPAGE)?;
    parse(&html)
}

pub fn parse(html: &str) -> Result<Vec<Model>> {
    let stream = rsc::extract_stream(html)?;
    let mut out = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut parse_failures = 0u32;

    for span in rsc::innermost_objects_with(&stream, "\"intelligenceIndex\":") {
        let Ok(model) = serde_json::from_str::<Model>(span) else {
            parse_failures += 1;
            continue;
        };
        if model.id.is_empty() || !seen.insert(model.id.clone()) {
            continue;
        }
        out.push(model);
    }

    if parse_failures > 0 {
        eprintln!("warning: {parse_failures} model(s) failed to parse from artificialanalysis.ai");
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
    fn live_fetch_when_enabled() {
        if std::env::var("LLMPK_LIVE").is_err() {
            return;
        }
        let models = fetch().expect("live artificialanalysis fetch");
        assert!(models.len() > 10);
    }

    #[test]
    fn parses_committed_fixture() {
        let html = include_str!("../tests/fixtures/aa_homepage.html");
        let models = parse(html).expect("parse committed fixture");
        assert!(
            models.len() >= 2,
            "expected >=2 models, got {}",
            models.len()
        );
        assert!(models.iter().any(|m| m.display_id() == "claude-sonnet-4"));
        let claude = models
            .iter()
            .find(|m| m.display_id() == "claude-sonnet-4")
            .expect("fixture should include Claude");
        assert_eq!(claude.provider_color(), Some("#cc785c"));
    }

    #[test]
    fn parses_fixture_when_provided() {
        let Ok(path) = std::env::var("LLMPK_HOMEPAGE_FIXTURE") else {
            return;
        };
        let html = std::fs::read_to_string(&path).expect("fixture read");
        let models = parse(&html).expect("parse");
        assert!(
            models.len() >= 10,
            "expected >=10 models, got {}",
            models.len()
        );
    }
}
