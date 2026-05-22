use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Style, Stylize},
    text::Line,
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap},
    Frame,
};

use super::chart::{chart_capacity, render_metric_chart, ChartPreference, ChartRow};
use super::{
    accent_color_for_provider, agent_metric, color_for_seed, detail_line, fmt_f, fmt_price,
    header_cell, highlight_matches, price_color, push_unique, score_color, selected_row_style,
    truncate, AgentKey, AppState,
};
use crate::board::Board;
use crate::coding_agents;
use ratatui::widgets::TableState;

const CLAUDE_CODE_COLOR: Color = Color::Rgb(255, 165, 0);
const CODEX_COLOR: Color = Color::Rgb(0xbe, 0xa5, 0xff);
const GEMINI_COLOR: Color = Color::Rgb(0x42, 0x85, 0xf4);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AgentColumn {
    Index,
    Agent,
    Model,
    Provider,
    Score,
    Pass,
    Cost,
    Time,
    Tokens,
    Turns,
    Released,
}

impl AgentColumn {
    fn header(self) -> &'static str {
        match self {
            AgentColumn::Index => "#",
            AgentColumn::Agent => "Agent",
            AgentColumn::Model => "Model",
            AgentColumn::Provider => "Provider",
            AgentColumn::Score => "Index",
            AgentColumn::Pass => "Pass@1",
            AgentColumn::Cost => "Cost",
            AgentColumn::Time => "Time",
            AgentColumn::Tokens => "Tokens",
            AgentColumn::Turns => "Turns",
            AgentColumn::Released => "Release",
        }
    }

    fn width(self) -> Constraint {
        match self {
            AgentColumn::Index => Constraint::Length(3),
            AgentColumn::Agent => Constraint::Min(12),
            AgentColumn::Model => Constraint::Length(24),
            AgentColumn::Provider => Constraint::Length(12),
            AgentColumn::Score => Constraint::Length(7),
            AgentColumn::Pass => Constraint::Length(7),
            AgentColumn::Cost => Constraint::Length(8),
            AgentColumn::Time => Constraint::Length(8),
            AgentColumn::Tokens => Constraint::Length(8),
            AgentColumn::Turns => Constraint::Length(6),
            AgentColumn::Released => Constraint::Length(10),
        }
    }

    fn order(self) -> u8 {
        match self {
            AgentColumn::Index => 0,
            AgentColumn::Agent => 1,
            AgentColumn::Model => 2,
            AgentColumn::Provider => 3,
            AgentColumn::Score => 4,
            AgentColumn::Pass => 5,
            AgentColumn::Cost => 6,
            AgentColumn::Time => 7,
            AgentColumn::Tokens => 8,
            AgentColumn::Turns => 9,
            AgentColumn::Released => 10,
        }
    }
}

pub(super) fn agent_columns(width: u16, sort_key: AgentKey) -> Vec<AgentColumn> {
    let mut columns = vec![
        AgentColumn::Index,
        AgentColumn::Agent,
        AgentColumn::Score,
        AgentColumn::Pass,
        AgentColumn::Cost,
    ];
    push_unique(&mut columns, agent_column_for_key(sort_key));
    if width >= 62 {
        push_unique(&mut columns, AgentColumn::Model);
    }
    if width >= 76 {
        push_unique(&mut columns, AgentColumn::Provider);
    }
    if width >= 88 {
        push_unique(&mut columns, AgentColumn::Time);
    }
    if width >= 100 {
        push_unique(&mut columns, AgentColumn::Tokens);
    }
    if width >= 110 {
        push_unique(&mut columns, AgentColumn::Turns);
    }
    if width >= 122 {
        push_unique(&mut columns, AgentColumn::Released);
    }
    columns.sort_by_key(|column| column.order());
    columns
}

fn agent_column_for_key(key: AgentKey) -> AgentColumn {
    match key {
        AgentKey::Index => AgentColumn::Score,
        AgentKey::Pass => AgentColumn::Pass,
        AgentKey::Cost => AgentColumn::Cost,
        AgentKey::Time => AgentColumn::Time,
        AgentKey::Tokens => AgentColumn::Tokens,
        AgentKey::Turns => AgentColumn::Turns,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn render_agents_table(
    frame: &mut Frame,
    area: Rect,
    all_rows: &[coding_agents::AgentRow],
    indices: &[usize],
    table_state: &mut std::collections::HashMap<Board, TableState>,
    board: Board,
    sort_key: AgentKey,
    filter_tokens: &[String],
    compact: bool,
) {
    let columns = agent_columns(area.width, sort_key);
    let header = Row::new(columns.iter().map(|column| header_cell(column.header())));

    let table_rows = indices.iter().enumerate().filter_map(|(i, &idx)| {
        all_rows.get(idx).map(|row| {
            Row::new(
                columns
                    .iter()
                    .map(|column| agent_cell(*column, i, row, filter_tokens)),
            )
        })
    });

    let widths: Vec<Constraint> = columns.iter().map(|column| column.width()).collect();
    let title = format!("AA Agents ({})", indices.len());

    let spacing = if compact { 0 } else { 1 };
    let table = Table::new(table_rows, widths)
        .header(header.height(1))
        .row_highlight_style(selected_row_style())
        .highlight_symbol("> ")
        .block(Block::default().borders(Borders::ALL).title(title))
        .column_spacing(spacing);

    let st = table_state.entry(board).or_default();
    frame.render_stateful_widget(table, area, st);
}

fn agent_cell(
    column: AgentColumn,
    index: usize,
    row: &coding_agents::AgentRow,
    filter_tokens: &[String],
) -> Cell<'static> {
    match column {
        AgentColumn::Index => {
            Cell::from(format!("{:>2}", index + 1)).style(Style::default().fg(Color::DarkGray))
        }
        AgentColumn::Agent => Cell::from(highlight_matches(
            row.agent(),
            filter_tokens,
            agent_name_style(row),
        )),
        AgentColumn::Model => Cell::from(row.model().to_string()),
        AgentColumn::Provider => Cell::from(row.provider().to_string())
            .style(Style::default().fg(agent_provider_color(row).unwrap_or(Color::Magenta))),
        AgentColumn::Score => {
            Cell::from(fmt_pct(row.index_score, 1)).style(score_color_pct(row.index_score))
        }
        AgentColumn::Pass => {
            Cell::from(fmt_pct(row.mean.reward, 1)).style(score_color_pct(row.mean.reward))
        }
        AgentColumn::Cost => Cell::from(fmt_price(row.mean.cost_usd, 2, "")).style(price_color(
            row.mean.cost_usd,
            1.0,
            5.0,
        )),
        AgentColumn::Time => Cell::from(fmt_duration(row.mean.agent_wall_time_sec))
            .style(Style::default().fg(Color::Blue)),
        AgentColumn::Tokens => Cell::from(fmt_compact_f(row.mean.total_tokens)),
        AgentColumn::Turns => Cell::from(fmt_f(row.mean.steps, 1)),
        AgentColumn::Released => Cell::from(row.release_date.clone().unwrap_or_else(|| "-".into()))
            .style(Style::default().fg(Color::DarkGray)),
    }
}

pub(super) fn render_agents_chart(
    frame: &mut Frame,
    area: Rect,
    all_rows: &[coding_agents::AgentRow],
    indices: &[usize],
    app: &AppState,
) {
    let key = app.agent_sort.key;
    let max_bars = chart_capacity(area);
    let chart_rows: Vec<ChartRow> = indices
        .iter()
        .filter_map(|&idx| {
            let row = all_rows.get(idx)?;
            let value = agent_metric(row, key)?;
            if !value.is_finite() {
                return None;
            }
            Some(ChartRow {
                name: row.label(),
                meta: row.provider().to_string(),
                value,
                value_label: agent_chart_text_value(row, key),
                color: Some(agent_series_color(row)),
                color_seed: row.id.clone(),
            })
        })
        .take(max_bars)
        .collect();

    let title = format!(
        "AA Agents chart - {} {} (top {}/{})",
        agent_key_label(key),
        app.agent_sort.dir.arrow(),
        chart_rows.len(),
        indices.len(),
    );
    let empty = format!(
        "no data for {} - switch sort key (i/a/p/t/u/s) or press m for table",
        agent_key_label(key)
    );
    let preference = agent_chart_preference(key);
    render_metric_chart(frame, area, &title, &empty, &chart_rows, preference);
}

pub(super) fn render_agent_detail(
    frame: &mut Frame,
    area: Rect,
    row: Option<&coding_agents::AgentRow>,
) {
    let max = area.width.saturating_sub(4) as usize;
    let lines = match row {
        Some(row) => vec![
            Line::styled(truncate(row.agent(), max.max(8)), agent_name_style(row)),
            detail_line("ID", &row.id, Style::default().fg(Color::DarkGray)),
            Line::from(""),
            detail_line("Model", row.model(), Style::default()),
            detail_line(
                "Provider",
                row.provider(),
                Style::default().fg(agent_provider_color(row).unwrap_or(Color::Magenta)),
            ),
            detail_line(
                "Index",
                fmt_pct(row.index_score, 1),
                score_color_pct(row.index_score),
            ),
            detail_line(
                "Pass@1",
                fmt_pct(row.mean.reward, 1),
                score_color_pct(row.mean.reward),
            ),
            detail_line(
                "Cost",
                fmt_price(row.mean.cost_usd, 2, "/task"),
                price_color(row.mean.cost_usd, 1.0, 5.0),
            ),
            detail_line(
                "Time",
                fmt_duration(row.mean.agent_wall_time_sec),
                Style::default().fg(Color::Blue),
            ),
            detail_line(
                "Tokens",
                fmt_compact_f(row.mean.total_tokens),
                Style::default(),
            ),
            detail_line("Turns", fmt_f(row.mean.steps, 1), Style::default()),
            detail_line(
                "Release",
                row.release_date.clone().unwrap_or_else(|| "-".into()),
                Style::default().fg(Color::Gray),
            ),
        ],
        None => vec![Line::styled(
            "No row selected",
            Style::default().fg(Color::DarkGray),
        )],
    };

    let p = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("Selected"))
        .wrap(Wrap { trim: true });
    frame.render_widget(p, area);
}

fn agent_chart_preference(key: AgentKey) -> ChartPreference {
    match key {
        AgentKey::Cost | AgentKey::Time | AgentKey::Tokens | AgentKey::Turns => {
            ChartPreference::Lower
        }
        AgentKey::Index | AgentKey::Pass => ChartPreference::Higher,
    }
}

fn agent_chart_text_value(row: &coding_agents::AgentRow, key: AgentKey) -> String {
    match key {
        AgentKey::Index => fmt_pct(row.index_score, 1),
        AgentKey::Pass => fmt_pct(row.mean.reward, 1),
        AgentKey::Cost => fmt_price(row.mean.cost_usd, 2, "/task"),
        AgentKey::Time => fmt_duration(row.mean.agent_wall_time_sec),
        AgentKey::Tokens => fmt_compact_f(row.mean.total_tokens),
        AgentKey::Turns => fmt_f(row.mean.steps, 1),
    }
}

fn agent_name_style(row: &coding_agents::AgentRow) -> Style {
    let style = Style::default().bold();
    if let Some(color) = agent_brand_color(row) {
        style.fg(color)
    } else {
        style
    }
}

fn agent_provider_color(row: &coding_agents::AgentRow) -> Option<Color> {
    accent_color_for_provider(row.provider())
}

fn agent_series_color(row: &coding_agents::AgentRow) -> Color {
    agent_brand_color(row)
        .unwrap_or_else(|| agent_provider_color(row).unwrap_or_else(|| color_for_seed(&row.id)))
}

fn agent_brand_color(row: &coding_agents::AgentRow) -> Option<Color> {
    if is_claude_code(row) {
        Some(CLAUDE_CODE_COLOR)
    } else if is_codex(row) {
        Some(CODEX_COLOR)
    } else if is_gemini(row) {
        Some(GEMINI_COLOR)
    } else {
        None
    }
}

const BRAND_CLAUDE_CODE: &str = "claude code";
const BRAND_CODEX: &str = "codex";
const BRAND_GEMINI: &str = "gemini";
const BRAND_GEMINI_CLI: &str = "gemini cli";

fn is_claude_code(row: &coding_agents::AgentRow) -> bool {
    row.agent().eq_ignore_ascii_case(BRAND_CLAUDE_CODE)
}

fn is_codex(row: &coding_agents::AgentRow) -> bool {
    row.agent().eq_ignore_ascii_case(BRAND_CODEX)
}

fn is_gemini(row: &coding_agents::AgentRow) -> bool {
    row.agent().eq_ignore_ascii_case(BRAND_GEMINI)
        || row.agent().eq_ignore_ascii_case(BRAND_GEMINI_CLI)
}

pub(super) fn agent_key_label(k: AgentKey) -> &'static str {
    match k {
        AgentKey::Index => "Index",
        AgentKey::Pass => "Pass@1",
        AgentKey::Cost => "Cost",
        AgentKey::Time => "Time",
        AgentKey::Tokens => "Tokens",
        AgentKey::Turns => "Turns",
    }
}

fn score_color_pct(v: Option<f64>) -> ratatui::style::Style {
    score_color(v.map(|x| x * 100.0), 30.0, 60.0)
}

fn fmt_pct(v: Option<f64>, decimals: usize) -> String {
    match v {
        Some(x) => format!("{:.*}", decimals, x * 100.0),
        None => "-".into(),
    }
}

fn fmt_duration(v: Option<f64>) -> String {
    let Some(seconds) = v else {
        return "-".into();
    };
    if seconds >= 3600.0 {
        format!("{:.1}h", seconds / 3600.0)
    } else if seconds >= 60.0 {
        format!("{:.1}m", seconds / 60.0)
    } else {
        format!("{seconds:.0}s")
    }
}

fn fmt_compact_f(v: Option<f64>) -> String {
    match v {
        Some(x) if x.is_finite() && x >= 0.0 => super::format_compact(x.round() as u64),
        Some(_) | None => "-".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coding_agents;

    fn make_agent_row(id: &str, agent: &str, provider: &str) -> coding_agents::AgentRow {
        coding_agents::AgentRow {
            id: id.to_string(),
            agent_name: agent.to_string(),
            provider: Some(provider.to_string()),
            host_name: Some(provider.to_string()),
            host_short_name: Some(provider.to_string()),
            model_name: Some(format!("{provider} Model")),
            host_model_slug: Some(format!("{}_model", provider.to_lowercase())),
            display_label: Some(format!("{agent} - {provider} Model")),
            release_date: Some("2026-01-01".to_string()),
            index_score: Some(0.6),
            display: coding_agents::AgentDisplay {
                agent: Some(agent.to_string()),
                model: Some(format!("{provider} Model")),
                creator: Some(coding_agents::AgentCreator {
                    model: Some(provider.to_string()),
                }),
            },
            mean: coding_agents::AgentMean {
                reward: Some(0.55),
                cost_usd: Some(1.25),
                agent_wall_time_sec: Some(420.0),
                steps: Some(42.0),
                total_tokens: Some(1_500_000.0),
            },
        }
    }

    #[test]
    fn claude_code_agent_uses_configured_color() {
        let claude = make_agent_row("opaque-id", "Claude Code", "Anthropic");
        let codex = make_agent_row("codex", "Codex", "OpenAI");

        assert_eq!(agent_brand_color(&claude), Some(CLAUDE_CODE_COLOR));
        assert_ne!(agent_brand_color(&codex), Some(CLAUDE_CODE_COLOR));
    }

    #[test]
    fn codex_agent_uses_configured_color() {
        let codex = make_agent_row("opaque-id", "Codex", "OpenAI");
        let claude = make_agent_row("claude-code", "Claude Code", "Anthropic");

        assert_eq!(agent_brand_color(&codex), Some(CODEX_COLOR));
        assert_ne!(agent_brand_color(&claude), Some(CODEX_COLOR));
    }

    #[test]
    fn gemini_agent_uses_google_blue() {
        let gemini = make_agent_row("opaque-id", "Gemini CLI", "Google");
        let cursor = make_agent_row("cursor", "Cursor CLI", "Anysphere");

        assert_eq!(agent_brand_color(&gemini), Some(GEMINI_COLOR));
        assert_ne!(agent_brand_color(&cursor), Some(GEMINI_COLOR));
    }

    #[test]
    fn unconfigured_agents_use_neutral_name_style() {
        let cursor = make_agent_row("cursor", "Cursor CLI", "Anysphere");

        assert_eq!(agent_brand_color(&cursor), None);
        assert_eq!(agent_name_style(&cursor).fg, None);
    }
}
