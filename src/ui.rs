use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Cell, TableState},
    Frame,
};

mod aa_board;
mod agents;
mod chart;
mod chrome;
mod deepswe_board;
mod filter;
mod sort;
pub mod state;
#[cfg(test)]
mod test_helpers;

pub use sort::*;
pub use state::*;

use crate::board::{Data, Status};
use filter::filter_tokens;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub fn render(frame: &mut Frame, app: &mut AppState) {
    if frame.area().width < 36 || frame.area().height < 8 {
        chrome::render_tiny(frame);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(2),
        ])
        .split(frame.area());

    chrome::render_tabs(frame, chunks[0], app);
    chrome::render_header(frame, chunks[1], app);
    render_body(frame, chunks[2], app);
    chrome::render_footer(frame, chunks[3], app);

    if app.is_help_open() {
        chrome::render_help_overlay(frame, frame.area());
    }
}

fn render_body(frame: &mut Frame, area: Rect, app: &mut AppState) {
    let board = app.current_board();
    let query = app.current_filter().to_string();
    let view = app.current_view();
    let aa_sort_key = app.aa_sort.key;
    let agent_sort_key = app.agent_sort.key;
    let deepswe_sort_key = app.deepswe_sort.key;
    let indices = app.filter_cache(board, &query).to_vec();
    let filter_tokens = filter_tokens(&query);
    let table_state = &mut app.table_state;

    match app.status.get(&board) {
        Some(Status::Loaded(Data::Aa(models))) => {
            if indices.is_empty() && filter_is_active(&query) {
                chrome::render_filter_empty(frame, area, &query);
                return;
            }
            let selected = table_state
                .get(&board)
                .and_then(TableState::selected)
                .unwrap_or(0);
            match view {
                View::Table => {
                    let (table_area, detail_area) = split_body(area);
                    aa_board::render_aa_table(
                        frame,
                        table_area,
                        models,
                        &indices,
                        table_state,
                        board,
                        aa_sort_key,
                        &filter_tokens,
                        app.compact,
                    );
                    if let Some(detail_area) = detail_area {
                        let model = indices.get(selected).and_then(|&i| models.get(i));
                        aa_board::render_aa_detail(frame, detail_area, model);
                    }
                }
                View::Chart => aa_board::render_aa_chart(frame, area, models, &indices, app),
            }
        }
        Some(Status::Loaded(Data::AaAgents(rows))) => {
            if indices.is_empty() && filter_is_active(&query) {
                chrome::render_filter_empty(frame, area, &query);
                return;
            }
            let selected = table_state
                .get(&board)
                .and_then(TableState::selected)
                .unwrap_or(0);
            match view {
                View::Table => {
                    let (table_area, detail_area) = split_agents_body(area);
                    agents::render_agents_table(
                        frame,
                        table_area,
                        rows,
                        &indices,
                        table_state,
                        board,
                        agent_sort_key,
                        &filter_tokens,
                        app.compact,
                    );
                    if let Some(detail_area) = detail_area {
                        let row = indices.get(selected).and_then(|&i| rows.get(i));
                        agents::render_agent_detail(frame, detail_area, row);
                    }
                }
                View::Chart => agents::render_agents_chart(frame, area, rows, &indices, app),
            }
        }
        Some(Status::Loaded(Data::DeepSwe(rows))) => {
            if indices.is_empty() && filter_is_active(&query) {
                chrome::render_filter_empty(frame, area, &query);
                return;
            }
            let selected = table_state
                .get(&board)
                .and_then(TableState::selected)
                .unwrap_or(0);
            match view {
                View::Table => {
                    let (table_area, detail_area) = split_deepswe_body(area);
                    deepswe_board::render_deepswe_table(
                        frame,
                        table_area,
                        rows,
                        &indices,
                        table_state,
                        board,
                        deepswe_sort_key,
                        &filter_tokens,
                        app.compact,
                    );
                    if let Some(detail_area) = detail_area {
                        let row = indices.get(selected).and_then(|&i| rows.get(i));
                        deepswe_board::render_deepswe_detail(frame, detail_area, row);
                    }
                }
                View::Chart => {
                    deepswe_board::render_deepswe_chart(frame, area, rows, &indices, app)
                }
            }
        }
        Some(Status::Error(e)) => {
            chrome::render_error(frame, area, e);
        }
        Some(Status::Loading) | None => {
            chrome::render_loading(frame, area);
        }
    }
}

fn split_body(area: Rect) -> (Rect, Option<Rect>) {
    if area.width < 116 || area.height < 12 {
        return (area, None);
    }

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(72), Constraint::Length(34)])
        .split(area);
    (chunks[0], Some(chunks[1]))
}

fn split_agents_body(area: Rect) -> (Rect, Option<Rect>) {
    if area.width < 132 || area.height < 12 {
        return (area, None);
    }

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(88), Constraint::Length(38)])
        .split(area);
    (chunks[0], Some(chunks[1]))
}

fn split_deepswe_body(area: Rect) -> (Rect, Option<Rect>) {
    if area.width < 110 || area.height < 12 {
        return (area, None);
    }

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(72), Constraint::Length(34)])
        .split(area);
    (chunks[0], Some(chunks[1]))
}

pub(crate) fn push_unique<T: PartialEq>(items: &mut Vec<T>, item: T) {
    if !items.contains(&item) {
        items.push(item);
    }
}

pub(crate) fn header_cell(label: &str) -> Cell<'_> {
    Cell::from(label).style(
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )
}

pub(crate) fn selected_row_style() -> Style {
    Style::default().add_modifier(Modifier::BOLD)
}

pub(crate) fn color_for_seed(seed: &str) -> Color {
    const PALETTE: &[Color] = &[
        Color::Cyan,
        Color::Magenta,
        Color::Green,
        Color::Yellow,
        Color::Blue,
        Color::Red,
        Color::LightCyan,
        Color::LightMagenta,
        Color::LightGreen,
        Color::LightYellow,
        Color::LightBlue,
        Color::LightRed,
    ];
    let mut h: u64 = 1469598103934665603;
    for b in seed.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    PALETTE[(h as usize) % PALETTE.len()]
}

pub(crate) fn color_from_hex(hex: &str) -> Option<Color> {
    let hex = hex.trim().trim_start_matches('#');
    let bytes = hex.as_bytes();
    if bytes.len() != 6 && bytes.len() != 8 {
        return None;
    }
    let r = parse_hex_byte(bytes, 0)?;
    let g = parse_hex_byte(bytes, 2)?;
    let b = parse_hex_byte(bytes, 4)?;
    Some(Color::Rgb(r, g, b))
}

fn parse_hex_byte(bytes: &[u8], offset: usize) -> Option<u8> {
    Some(hex_nibble(bytes[offset])? << 4 | hex_nibble(bytes[offset + 1])?)
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

pub(crate) fn readable_color_for_dark_bg(color: Color) -> Color {
    const MIN_LUMA: f64 = 128.0;

    match color {
        Color::Black | Color::DarkGray => Color::Gray,
        Color::Rgb(r, g, b) => {
            let luma = rgb_luma(r, g, b);
            if luma >= MIN_LUMA {
                return color;
            }

            let mix = ((MIN_LUMA - luma) / (255.0 - luma)).clamp(0.0, 1.0);
            Color::Rgb(
                lift_channel(r, mix),
                lift_channel(g, mix),
                lift_channel(b, mix),
            )
        }
        _ => color,
    }
}

pub(crate) fn accent_color_for_provider(provider: &str) -> Option<Color> {
    let color = match normalized_provider(provider).as_str() {
        "openai" => Color::Rgb(0x1f, 0x1f, 0x1f),
        "anthropic" => Color::Rgb(0xcc, 0x78, 0x5c),
        "google" => Color::Rgb(0x34, 0xa8, 0x53),
        "meta" => Color::Rgb(0x00, 0x89, 0xf4),
        "deepseek" => Color::Rgb(0x22, 0x43, 0xe6),
        "mistral" => Color::Rgb(0xfd, 0x6f, 0x00),
        "xai" => Color::Rgb(0x73, 0x6c, 0xd3),
        "amazon" | "aws" | "amazonbedrock" => Color::Rgb(0xff, 0x99, 0x00),
        "microsoft" | "azure" | "microsoftazure" => Color::Rgb(0x00, 0x78, 0xd5),
        "minimax" => Color::Rgb(0xeb, 0x35, 0x68),
        "nvidia" => Color::Rgb(0x86, 0xb7, 0x37),
        "kimi" | "moonshot" | "moonshotai" => Color::Rgb(0x04, 0x7a, 0xfe),
        "alibaba" | "qwen" | "alibabacloud" => Color::Rgb(0xff, 0x70, 0x18),
        "zai" | "z" | "zhipu" | "zhipuai" => Color::Rgb(0x1c, 0x7f, 0xf8),
        "upstage" => Color::Rgb(0x7c, 0x59, 0xf5),
        "ibm" => Color::Rgb(0x0f, 0x62, 0xfe),
        "stepfun" => Color::Rgb(0x01, 0x7a, 0xff),
        "perplexity" => Color::Rgb(0x1b, 0x81, 0x8e),
        "cohere" => Color::Rgb(0x39, 0x60, 0x99),
        "xaiinc" => Color::Rgb(0x73, 0x6c, 0xd3),
        "xiaomi" | "mimo" => Color::Rgb(0xff, 0x69, 0x00),
        _ => return None,
    };
    Some(readable_color_for_dark_bg(color))
}

fn normalized_provider(provider: &str) -> String {
    provider
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn rgb_luma(r: u8, g: u8, b: u8) -> f64 {
    f64::from(r) * 0.2126 + f64::from(g) * 0.7152 + f64::from(b) * 0.0722
}

fn lift_channel(channel: u8, mix: f64) -> u8 {
    (f64::from(channel) + (255.0 - f64::from(channel)) * mix).round() as u8
}

pub(crate) fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(n.saturating_sub(1)).collect();
        out.push('\u{2026}');
        out
    }
}

pub(crate) fn detail_line(label: &str, value: impl Into<String>, style: Style) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:<10}"), Style::default().fg(Color::DarkGray)),
        Span::styled(value.into(), style),
    ])
}

pub(crate) fn filter_is_active(query: &str) -> bool {
    !query.trim().is_empty()
}

pub(crate) fn format_filter_label(query: &str, editing: bool) -> String {
    let mut label = if query.is_empty() {
        "<empty>".to_string()
    } else {
        truncate(query, 32)
    };
    if editing {
        label.push('|');
    }
    label
}

pub(crate) fn fmt_f(v: Option<f64>, decimals: usize) -> String {
    match v {
        Some(x) => format!("{x:.*}", decimals),
        None => "-".into(),
    }
}

pub(crate) fn fmt_price(v: Option<f64>, decimals: usize, suffix: &str) -> String {
    match v {
        Some(x) => format!("${x:.*}{suffix}", decimals),
        None => "-".into(),
    }
}

pub(crate) fn fmt_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{}M", n / 1_000_000)
    } else if n >= 1_000 {
        format!("{}K", n / 1_000)
    } else {
        n.to_string()
    }
}

pub(crate) fn format_compact(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 10_000 {
        format!("{}K", n / 1_000)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

pub(crate) fn fmt_duration(v: Option<f64>) -> String {
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

pub(crate) fn fmt_compact_f(v: Option<f64>) -> String {
    match v {
        Some(x) if x.is_finite() && x >= 0.0 => format_compact(x.round() as u64),
        Some(_) | None => "-".into(),
    }
}

pub(crate) fn score_color(v: Option<f64>, low: f64, high: f64) -> Style {
    let Some(x) = v else {
        return Style::default().fg(Color::DarkGray);
    };
    let color = if x >= high - (high - low) * 0.2 {
        Color::Green
    } else if x >= (high + low) / 2.0 {
        Color::Yellow
    } else {
        Color::Red
    };
    Style::default().fg(color).bold()
}

pub(crate) fn price_color(v: Option<f64>, low: f64, high: f64) -> Style {
    let Some(x) = v else {
        return Style::default().fg(Color::DarkGray);
    };
    let color = if x <= low {
        Color::Green
    } else if x <= high {
        Color::Yellow
    } else {
        Color::Red
    };
    Style::default().fg(color)
}

/// Highlight matching tokens in text. Returns a Line with highlighted matches.
pub(crate) fn highlight_matches(text: &str, tokens: &[String], base_style: Style) -> Line<'static> {
    if tokens.is_empty() || tokens.iter().all(|t| t.is_empty()) {
        return Line::styled(text.to_string(), base_style);
    }
    let lower = text.to_lowercase();
    let mut spans = Vec::new();
    let mut last_end = 0;

    // Find first match of any token.
    loop {
        let mut earliest: Option<(usize, usize)> = None; // (start, end)
        for token in tokens {
            if let Some(pos) = lower[last_end..].find(token.as_str()) {
                let abs_pos = last_end + pos;
                let end = abs_pos + token.len();
                match earliest {
                    Some((s, _)) if abs_pos < s => earliest = Some((abs_pos, end)),
                    Some(_) => {}
                    None => earliest = Some((abs_pos, end)),
                }
            }
        }
        if let Some((start, end)) = earliest {
            if start > last_end {
                spans.push(Span::styled(text[last_end..start].to_string(), base_style));
            }
            spans.push(Span::styled(
                text[start..end].to_string(),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
            last_end = end;
        } else {
            break;
        }
    }
    if last_end < text.len() {
        spans.push(Span::styled(text[last_end..].to_string(), base_style));
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::chart::{
        chart_bar_len, chart_capacity, render_metric_chart, ChartPreference, ChartRow,
    };
    use super::test_helpers::*;
    use super::*;
    use crate::board::Board;

    #[test]
    fn parses_hex_colors_and_maps_provider_accents() {
        assert_eq!(
            color_from_hex("#cc785c"),
            Some(Color::Rgb(0xcc, 0x78, 0x5c))
        );
        assert_eq!(
            color_from_hex("#7b61ff00"),
            Some(Color::Rgb(0x7b, 0x61, 0xff))
        );
        assert_eq!(
            accent_color_for_provider("Z.ai"),
            Some(readable_color_for_dark_bg(Color::Rgb(0x1c, 0x7f, 0xf8)))
        );
        assert_eq!(
            accent_color_for_provider("Moonshot AI"),
            Some(readable_color_for_dark_bg(Color::Rgb(0x04, 0x7a, 0xfe)))
        );
        assert_eq!(color_from_hex("#nothex"), None);
        assert_eq!(color_from_hex("aébcd"), None);
    }

    #[test]
    fn dark_provider_accents_are_lifted_for_black_backgrounds() {
        assert_eq!(
            readable_color_for_dark_bg(Color::Rgb(0x1f, 0x1f, 0x1f)),
            Color::Rgb(0x80, 0x80, 0x80)
        );
        assert_eq!(
            readable_color_for_dark_bg(Color::Rgb(0xcc, 0x78, 0x5c)),
            Color::Rgb(0xcc, 0x78, 0x5c)
        );
        assert_eq!(
            accent_color_for_provider("OpenAI"),
            Some(Color::Rgb(0x80, 0x80, 0x80))
        );
    }

    #[test]
    fn horizontal_chart_capacity_scales_with_height() {
        let short = chart_capacity(Rect::new(0, 0, 120, 12));
        let tall = chart_capacity(Rect::new(0, 0, 120, 24));
        let narrow_same_height = chart_capacity(Rect::new(0, 0, 70, 24));

        assert!(tall > short);
        assert_eq!(tall, narrow_same_height);
        assert!(short >= 1);
    }

    #[test]
    fn chart_bar_lengths_handle_direction_and_equal_ranges() {
        let high_better = chart_bar_len(10.0, 1.0, 10.0, 12, ChartPreference::Higher);
        let low_better = chart_bar_len(1.0, 1.0, 10.0, 12, ChartPreference::Lower);
        let equal = chart_bar_len(5.0, 5.0, 5.0, 12, ChartPreference::Higher);
        let missing = chart_bar_len(f64::NAN, 1.0, 10.0, 12, ChartPreference::Higher);

        assert_eq!(high_better, 12);
        assert_eq!(low_better, 12);
        assert_eq!(equal, 12);
        assert_eq!(missing, 0);
    }

    #[test]
    fn narrow_chart_uses_compact_text_without_bars() {
        let rows = vec![ChartRow {
            name: "Very Long Model Name".to_string(),
            meta: "Anthropic".to_string(),
            value: 50.0,
            value_label: "50.0".to_string(),
            color: Some(Color::Rgb(0xcc, 0x78, 0x5c)),
            color_seed: "claude".to_string(),
        }];
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(36, 8)).unwrap();
        terminal
            .draw(|frame| {
                render_metric_chart(
                    frame,
                    Rect::new(0, 0, 36, 8),
                    "chart",
                    "empty",
                    &rows,
                    ChartPreference::Higher,
                )
            })
            .unwrap();
        let text = rendered_text(terminal.backend());

        assert!(text.contains("Top 1 visible"));
        assert!(text.contains("50.0"));
        assert!(text.contains("Very Long Model"));
        assert!(!text.contains("█"));
    }

    #[test]
    fn wide_chart_keeps_value_close_to_label() {
        let rows = vec![ChartRow {
            name: "Codex".to_string(),
            meta: "OpenAI".to_string(),
            value: 60.0,
            value_label: "60.0".to_string(),
            color: Some(Color::Rgb(0xbe, 0xa5, 0xff)),
            color_seed: "codex".to_string(),
        }];
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 8)).unwrap();
        terminal
            .draw(|frame| {
                render_metric_chart(
                    frame,
                    Rect::new(0, 0, 100, 8),
                    "chart",
                    "empty",
                    &rows,
                    ChartPreference::Higher,
                )
            })
            .unwrap();

        let line = rendered_line(terminal.backend(), 100, 2);
        let name_pos = line.find("Codex").expect("row should include chart label");
        let value_pos = line.find("60.0").expect("row should include value");

        assert!(value_pos - (name_pos + "Codex".len()) <= 14);
    }

    #[test]
    fn responsive_aa_columns_keep_active_sort_metric_visible() {
        let cols = aa_board::aa_columns(48, AaKey::Speed);

        assert!(cols.contains(&aa_board::AaColumn::Speed));
        assert!(cols.contains(&aa_board::AaColumn::Model));
        assert!(cols.contains(&aa_board::AaColumn::Intelligence));
        assert!(cols.contains(&aa_board::AaColumn::Price));
        assert!(!cols.contains(&aa_board::AaColumn::Released));
    }

    #[test]
    fn responsive_agent_columns_keep_active_sort_metric_visible() {
        let cols = agents::agent_columns(48, AgentKey::Time);

        assert!(cols.contains(&agents::AgentColumn::Time));
        assert!(cols.contains(&agents::AgentColumn::Agent));
        assert!(cols.contains(&agents::AgentColumn::Score));
        assert!(!cols.contains(&agents::AgentColumn::Released));
    }

    #[test]
    fn render_draws_tiny_and_wide_layouts() {
        let mut tiny = AppState::new();
        let mut tiny_terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(35, 7)).unwrap();
        tiny_terminal
            .draw(|frame| render(frame, &mut tiny))
            .unwrap();
        assert!(rendered_text(tiny_terminal.backend()).contains("larger terminal"));

        let mut wide = AppState::new();
        wide.set_status(
            Board::Aa,
            Status::Loaded(Data::Aa(vec![aa_model(
                "claude-sonnet",
                "Claude Sonnet",
                "Anthropic",
            )])),
        );
        let mut wide_terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(128, 32)).unwrap();
        wide_terminal
            .draw(|frame| render(frame, &mut wide))
            .unwrap();
        let text = rendered_text(wide_terminal.backend());

        assert!(text.contains("AA ("));
        assert!(text.contains("Selected"));
        assert!(text.contains("Claude Sonnet"));
    }

    #[test]
    fn render_draws_aa_chart_view() {
        let mut app = AppState::new();
        app.set_status(
            Board::Aa,
            Status::Loaded(Data::Aa(vec![
                aa_model("claude-sonnet", "Claude Sonnet", "Anthropic"),
                aa_model("gpt-5", "GPT-5", "OpenAI"),
            ])),
        );
        app.toggle_view();

        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(132, 32)).unwrap();
        terminal.draw(|frame| render(frame, &mut app)).unwrap();
        let text = rendered_text(terminal.backend());

        assert!(text.contains("AA chart"));
        assert!(text.contains("Top 2 visible"));
        assert!(text.contains("Anthropic"));
        assert!(text.contains("OpenAI"));
        assert!(text.contains("Claude Sonnet"));
        assert!(text.contains("50.0"));
        assert!(text.contains("view: chart"));
    }

    #[test]
    fn render_draws_agents_chart_view() {
        let mut app = AppState::new();
        app.select_board(1);
        app.set_status(
            Board::AaAgents,
            Status::Loaded(Data::AaAgents(vec![
                agent_row("claude-code", "Claude Code", "Anthropic"),
                agent_row("codex", "Codex", "OpenAI"),
            ])),
        );
        app.toggle_view();

        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(132, 32)).unwrap();
        terminal.draw(|frame| render(frame, &mut app)).unwrap();
        let text = rendered_text(terminal.backend());

        assert!(text.contains("AA Agents chart"));
        assert!(text.contains("Top 2 visible"));
        assert!(text.contains("Anthropic"));
        assert!(text.contains("view: chart"));
        assert!(text.contains("Claude Code"));
        assert!(text.contains("60.0"));
    }

    #[test]
    fn render_draws_agents_table_with_detail_panel() {
        let mut app = AppState::new();
        app.select_board(1);
        app.set_status(
            Board::AaAgents,
            Status::Loaded(Data::AaAgents(vec![
                agent_row("claude-code", "Claude Code", "Anthropic"),
                agent_row("codex", "Codex", "OpenAI"),
                agent_row("cursor", "Cursor CLI", "Anysphere"),
                agent_row("gemini", "Gemini CLI", "Google"),
                agent_row("opencode", "Opencode", "SST"),
                agent_row("goose", "Goose", "Block"),
            ])),
        );

        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(150, 34)).unwrap();
        terminal.draw(|frame| render(frame, &mut app)).unwrap();
        let text = rendered_text(terminal.backend());

        assert!(text.contains("AA Agents ("));
        assert!(!text.contains("Radar"));
        assert!(!text.contains("Legend"));
        assert!(text.contains("Selected"));
        assert!(text.contains("Claude Code"));
        assert!(text.contains("Codex"));
        assert!(text.contains("Pass@1"));
        assert!(text.contains("$1.25/task"));
    }

    fn rendered_text(backend: &ratatui::backend::TestBackend) -> String {
        backend
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    fn rendered_line(backend: &ratatui::backend::TestBackend, width: usize, row: usize) -> String {
        backend.buffer().content()[row * width..(row + 1) * width]
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    #[test]
    fn render_shows_version_in_title() {
        let mut app = AppState::new();
        app.set_status(
            Board::Aa,
            Status::Loaded(Data::Aa(vec![aa_model("test", "Test Model", "Provider")])),
        );
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24)).unwrap();
        terminal.draw(|frame| render(frame, &mut app)).unwrap();
        let text = rendered_text(terminal.backend());

        assert!(text.contains("llmpk v"));
    }

    #[test]
    fn render_shows_position_indicator() {
        let mut app = AppState::new();
        app.set_status(
            Board::Aa,
            Status::Loaded(Data::Aa(vec![
                aa_model("m0", "Model 0", "P"),
                aa_model("m1", "Model 1", "P"),
                aa_model("m2", "Model 2", "P"),
                aa_model("m3", "Model 3", "P"),
                aa_model("m4", "Model 4", "P"),
            ])),
        );
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24)).unwrap();
        terminal.draw(|frame| render(frame, &mut app)).unwrap();
        let text = rendered_text(terminal.backend());

        assert!(text.contains("1/5"));
    }
}
