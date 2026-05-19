use std::cmp::Ordering;

use crate::aa;
use crate::coding_agents;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDir {
    Desc,
    Asc,
}

impl SortDir {
    pub fn arrow(self) -> &'static str {
        match self {
            SortDir::Desc => "v",
            SortDir::Asc => "^",
        }
    }
    pub fn toggle(self) -> Self {
        match self {
            SortDir::Desc => SortDir::Asc,
            SortDir::Asc => SortDir::Desc,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AaKey {
    Intelligence,
    Speed,
    Price,
    Context,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentKey {
    Index,
    Pass,
    Cost,
    Time,
    Tokens,
    Turns,
}

#[derive(Debug, Clone)]
pub struct AaSort {
    pub key: AaKey,
    pub dir: SortDir,
}

#[derive(Debug, Clone)]
pub struct AgentSort {
    pub key: AgentKey,
    pub dir: SortDir,
}

// ── Sort functions ───────────────────────────────────────────────────

pub fn sort_with(data: &mut crate::board::Data, aa_sort: &AaSort, agent_sort: &AgentSort) {
    match data {
        crate::board::Data::Aa(models) => sort_aa(models, aa_sort),
        crate::board::Data::AaAgents(rows) => sort_agents(rows, agent_sort),
    }
}

fn sort_aa(models: &mut [aa::Model], sort: &AaSort) {
    let key = sort.key;
    let dir = sort.dir;
    models.sort_by(|a, b| {
        let av = aa_metric(a, key);
        let bv = aa_metric(b, key);
        cmp_opt_for_dir(av, bv, dir)
    });
}

pub fn aa_metric(m: &aa::Model, key: AaKey) -> Option<f64> {
    match key {
        AaKey::Intelligence => m.intelligence_index,
        AaKey::Speed => m.speed(),
        AaKey::Price => m.price_1m_blended_3_to_1,
        AaKey::Context => m.context_window_tokens.map(|x| x as f64),
    }
}

fn sort_agents(rows: &mut [coding_agents::AgentRow], sort: &AgentSort) {
    let key = sort.key;
    let dir = sort.dir;
    rows.sort_by(|a, b| {
        let av = agent_metric(a, key);
        let bv = agent_metric(b, key);
        cmp_opt_for_dir(av, bv, dir)
    });
}

pub fn agent_metric(row: &coding_agents::AgentRow, key: AgentKey) -> Option<f64> {
    match key {
        AgentKey::Index => row.index_score,
        AgentKey::Pass => row.mean.reward,
        AgentKey::Cost => row.mean.cost_usd,
        AgentKey::Time => row.mean.agent_wall_time_sec,
        AgentKey::Tokens => row.mean.total_tokens,
        AgentKey::Turns => row.mean.steps,
    }
}

fn cmp_opt_for_dir(a: Option<f64>, b: Option<f64>, dir: SortDir) -> Ordering {
    match (a, b) {
        (Some(x), Some(y)) => {
            let ord = x.partial_cmp(&y).unwrap_or(Ordering::Equal);
            match dir {
                SortDir::Desc => ord.reverse(),
                SortDir::Asc => ord,
            }
        }
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::*;
    use crate::board::{Board, Data, Status};
    use super::super::AppState;

    #[test]
    fn descending_sorts_keep_missing_metrics_last() {
        let mut app = AppState::new();
        let mut missing = aa_model("missing", "Missing Intel", "Unknown");
        missing.intelligence_index = None;
        app.set_status(
            Board::Aa,
            Status::Loaded(Data::Aa(vec![
                missing,
                aa_model("scored", "Scored Intel", "OpenAI"),
            ])),
        );

        let Some(Status::Loaded(Data::Aa(models))) = app.status.get(&Board::Aa) else {
            panic!("AA data should be loaded");
        };
        assert_eq!(models[0].name, "Scored Intel");
        assert_eq!(models[1].name, "Missing Intel");
    }

    #[test]
    fn descending_agent_sorts_keep_missing_metrics_last() {
        let mut app = AppState::new();
        let mut missing = agent_row("missing", "Missing Agent", "Unknown");
        missing.index_score = None;
        app.set_status(
            Board::AaAgents,
            Status::Loaded(Data::AaAgents(vec![
                missing,
                agent_row("scored", "Scored Agent", "Known"),
            ])),
        );

        let Some(Status::Loaded(Data::AaAgents(rows))) = app.status.get(&Board::AaAgents) else {
            panic!("AA Agents data should be loaded");
        };
        assert_eq!(rows[0].agent(), "Scored Agent");
        assert_eq!(rows[1].agent(), "Missing Agent");
    }
}
