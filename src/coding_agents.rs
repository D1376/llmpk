use anyhow::{anyhow, Result};
use serde::Deserialize;

use crate::rsc;

const CODING_AGENTS_URL: &str = "https://artificialanalysis.ai/agents/coding-agents";

#[derive(Debug, Clone, Deserialize)]
pub struct AgentRow {
    #[serde(default)]
    pub id: String,
    #[serde(default, rename = "agentName")]
    pub agent_name: String,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default, rename = "hostName")]
    pub host_name: Option<String>,
    #[serde(default, rename = "hostShortName")]
    pub host_short_name: Option<String>,
    #[serde(default, rename = "modelName")]
    pub model_name: Option<String>,
    #[serde(default, rename = "hostModelSlug")]
    pub host_model_slug: Option<String>,
    #[serde(default, rename = "displayLabel")]
    pub display_label: Option<String>,
    #[serde(default, rename = "releaseDate")]
    pub release_date: Option<String>,
    #[serde(default, rename = "indexScore")]
    pub index_score: Option<f64>,
    #[serde(default)]
    pub display: AgentDisplay,
    #[serde(default)]
    pub mean: AgentMean,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AgentDisplay {
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub creator: Option<AgentCreator>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AgentCreator {
    #[serde(default)]
    pub model: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AgentMean {
    #[serde(default)]
    pub reward: Option<f64>,
    #[serde(default, rename = "costUsd")]
    pub cost_usd: Option<f64>,
    #[serde(default, rename = "agentWallTimeSec")]
    pub agent_wall_time_sec: Option<f64>,
    #[serde(default)]
    pub steps: Option<f64>,
    #[serde(default, rename = "totalTokens")]
    pub total_tokens: Option<f64>,
}

impl AgentRow {
    pub fn agent(&self) -> &str {
        first_nonempty([
            self.display.agent.as_deref(),
            Some(self.agent_name.as_str()),
        ])
        .unwrap_or("?")
    }

    pub fn model(&self) -> &str {
        first_nonempty([
            self.display.model.as_deref(),
            self.model_name.as_deref(),
            self.host_model_slug.as_deref(),
        ])
        .unwrap_or("?")
    }

    pub fn provider(&self) -> &str {
        first_nonempty([
            self.display
                .creator
                .as_ref()
                .and_then(|creator| creator.model.as_deref()),
            self.host_short_name.as_deref(),
            self.host_name.as_deref(),
            self.provider.as_deref(),
        ])
        .unwrap_or("?")
    }

    pub fn label(&self) -> String {
        first_nonempty([self.display_label.as_deref()])
            .map(str::to_string)
            .unwrap_or_else(|| format!("{} - {}", self.agent(), self.model()))
    }
}

fn first_nonempty<const N: usize>(values: [Option<&str>; N]) -> Option<&str> {
    values
        .into_iter()
        .flatten()
        .find(|value| !value.trim().is_empty())
}

pub fn fetch() -> Result<Vec<AgentRow>> {
    let html = rsc::fetch_text_retry(CODING_AGENTS_URL)?;
    parse(&html)
}

pub fn parse(html: &str) -> Result<Vec<AgentRow>> {
    let stream = rsc::extract_stream(html)?;
    let arr = rsc::first_array_after(&stream, "rows")
        .ok_or_else(|| anyhow!("no rows array in artificialanalysis.ai coding agents page"))?;
    let rows: Vec<AgentRow> = serde_json::from_str(arr)?;
    if rows.is_empty() {
        return Err(anyhow!("coding agents rows array empty"));
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_fetch_when_enabled() {
        if std::env::var("LLMPK_LIVE").is_err() {
            return;
        }
        let rows = fetch().expect("live coding agents fetch");
        assert!(rows.len() > 5);
        assert!(rows.iter().any(|row| row.index_score.is_some()));
    }

    #[test]
    fn parses_committed_fixture() {
        let html = include_str!("../tests/fixtures/coding_agents.html");
        let rows = parse(html).expect("parse committed fixture");
        assert!(rows.len() >= 2, "expected >=2 rows, got {}", rows.len());
        assert!(rows.iter().any(|r| r.id == "claude-code"));
        let cursor = rows
            .iter()
            .find(|r| r.id == "cursor-cli")
            .expect("fixture should include Cursor CLI");
        assert_eq!(cursor.provider(), "Anthropic");
    }

    #[test]
    fn parses_fixture_when_provided() {
        let Ok(path) = std::env::var("LLMPK_CODING_AGENTS_FIXTURE") else {
            return;
        };
        let html = std::fs::read_to_string(&path).expect("fixture read");
        let rows = parse(&html).expect("parse");
        assert!(
            rows.len() >= 5,
            "expected >=5 coding agent rows, got {}",
            rows.len()
        );
    }
}
