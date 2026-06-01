use crate::aa;
use crate::coding_agents;
use crate::deepswe;

pub struct FilterCache {
    pub(super) indices: Vec<usize>,
    pub(super) query: String,
}

pub fn filter_tokens(query: &str) -> Vec<String> {
    query.split_whitespace().map(str::to_lowercase).collect()
}

/// Check if a lowercase token appears in any of the provided field values.
fn token_in_fields(token: &str, fields: &[&str]) -> bool {
    fields.iter().any(|f| f.to_lowercase().contains(token))
}

pub fn aa_matches_filter(model: &aa::Model, tokens: &[String]) -> bool {
    let weights = match model.is_open_weights {
        Some(true) => "open open-weights",
        Some(false) => "closed proprietary",
        None => "",
    };
    let fields = [
        model.id.as_str(),
        model.name.as_str(),
        model.provider(),
        model.release_date.as_deref().unwrap_or(""),
        weights,
    ];
    tokens.iter().all(|token| token_in_fields(token, &fields))
}

pub fn agent_matches_filter(row: &coding_agents::AgentRow, tokens: &[String]) -> bool {
    let fields = [
        row.id.as_str(),
        row.agent_name.as_str(),
        row.agent(),
        row.model(),
        row.provider(),
        row.display_label.as_deref().unwrap_or(""),
        row.model_name.as_deref().unwrap_or(""),
        row.host_model_slug.as_deref().unwrap_or(""),
        row.release_date.as_deref().unwrap_or(""),
    ];
    tokens.iter().all(|token| token_in_fields(token, &fields))
}

pub(super) fn count_matching_aa(models: &[aa::Model], query: &str) -> usize {
    let tokens = filter_tokens(query);
    if tokens.is_empty() {
        return models.len();
    }
    models
        .iter()
        .filter(|model| aa_matches_filter(model, &tokens))
        .count()
}

pub(super) fn count_matching_agents(rows: &[coding_agents::AgentRow], query: &str) -> usize {
    let tokens = filter_tokens(query);
    if tokens.is_empty() {
        return rows.len();
    }
    rows.iter()
        .filter(|row| agent_matches_filter(row, &tokens))
        .count()
}

pub fn deepswe_matches_filter(row: &deepswe::Row, tokens: &[String]) -> bool {
    let display = row.display_model();
    let fields = [
        row.model.as_str(),
        display.as_str(),
        row.reasoning_effort.as_deref().unwrap_or(""),
    ];
    tokens.iter().all(|token| token_in_fields(token, &fields))
}

pub(super) fn count_matching_deepswe(rows: &[deepswe::Row], query: &str) -> usize {
    let tokens = filter_tokens(query);
    if tokens.is_empty() {
        return rows.len();
    }
    rows.iter()
        .filter(|row| deepswe_matches_filter(row, &tokens))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_tokens_splits_and_lowercases() {
        assert_eq!(filter_tokens("Hello World"), vec!["hello", "world"]);
    }

    #[test]
    fn filter_tokens_empty_query() {
        assert!(filter_tokens("").is_empty());
    }

    #[test]
    fn filter_tokens_whitespace_only() {
        assert!(filter_tokens("   ").is_empty());
    }

    #[test]
    fn aa_matches_filter_by_id() {
        let model =
            crate::ui::test_helpers::aa_model("claude-sonnet-4", "Claude Sonnet 4", "Anthropic");
        assert!(aa_matches_filter(&model, &["claude".into()]));
        assert!(!aa_matches_filter(&model, &["gpt".into()]));
    }

    #[test]
    fn aa_matches_filter_by_provider() {
        let model = crate::ui::test_helpers::aa_model("gpt-4o", "GPT-4o", "OpenAI");
        assert!(aa_matches_filter(&model, &["openai".into()]));
    }

    #[test]
    fn aa_matches_filter_multi_token_and() {
        let model =
            crate::ui::test_helpers::aa_model("claude-sonnet-4", "Claude Sonnet 4", "Anthropic");
        assert!(aa_matches_filter(
            &model,
            &["claude".into(), "sonnet".into()]
        ));
        assert!(!aa_matches_filter(&model, &["claude".into(), "gpt".into()]));
    }

    #[test]
    fn aa_matches_filter_open_weights() {
        let mut model = crate::ui::test_helpers::aa_model("llama-3", "Llama 3", "Meta");
        model.is_open_weights = Some(true);
        assert!(aa_matches_filter(&model, &["open".into()]));
        assert!(aa_matches_filter(&model, &["open-weights".into()]));
        assert!(!aa_matches_filter(&model, &["proprietary".into()]));
    }

    #[test]
    fn aa_matches_filter_proprietary() {
        let mut model = crate::ui::test_helpers::aa_model("gpt-4o", "GPT-4o", "OpenAI");
        model.is_open_weights = Some(false);
        assert!(aa_matches_filter(&model, &["proprietary".into()]));
        assert!(aa_matches_filter(&model, &["closed".into()]));
        assert!(!aa_matches_filter(&model, &["open-weights".into()]));
    }

    #[test]
    fn count_matching_aa_returns_total_on_empty_query() {
        let models = vec![
            crate::ui::test_helpers::aa_model("a", "A", "P"),
            crate::ui::test_helpers::aa_model("b", "B", "P"),
        ];
        assert_eq!(count_matching_aa(&models, ""), 2);
    }

    #[test]
    fn count_matching_aa_filters() {
        let models = vec![
            crate::ui::test_helpers::aa_model("claude-sonnet", "Claude", "Anthropic"),
            crate::ui::test_helpers::aa_model("gpt-4o", "GPT", "OpenAI"),
        ];
        assert_eq!(count_matching_aa(&models, "claude"), 1);
        assert_eq!(count_matching_aa(&models, "gpt"), 1);
        assert_eq!(count_matching_aa(&models, "xyz"), 0);
    }

    #[test]
    fn agent_matches_filter_by_id() {
        let row = crate::ui::test_helpers::agent_row("claude-code", "Claude Code", "Anthropic");
        assert!(agent_matches_filter(&row, &["claude".into()]));
        assert!(!agent_matches_filter(&row, &["copilot".into()]));
    }

    #[test]
    fn agent_matches_filter_by_provider() {
        let row = crate::ui::test_helpers::agent_row("copilot", "Copilot", "GitHub");
        assert!(agent_matches_filter(&row, &["github".into()]));
    }
}
