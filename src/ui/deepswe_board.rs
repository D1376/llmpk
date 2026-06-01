use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Style, Stylize},
    text::Line,
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap},
    Frame,
};

use super::chart::{chart_capacity, render_metric_chart, ChartPreference, ChartRow};
use super::{
    deepswe_metric, detail_line, fmt_f, fmt_price, header_cell, highlight_matches, price_color,
    push_unique, score_color, selected_row_style, truncate, AppState, DeepSweKey,
};
use crate::board::Board;
use crate::deepswe;
use ratatui::widgets::TableState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DeepSweColumn {
    Index,
    Model,
    Pass,
    Cost,
    Time,
    Tokens,
    Steps,
    Passed,
}

impl DeepSweColumn {
    fn header(self) -> &'static str {
        match self {
            DeepSweColumn::Index => "#",
            DeepSweColumn::Model => "Model",
            DeepSweColumn::Pass => "Pass@1",
            DeepSweColumn::Cost => "Cost",
            DeepSweColumn::Time => "Time",
            DeepSweColumn::Tokens => "Tokens",
            DeepSweColumn::Steps => "Steps",
            DeepSweColumn::Passed => "Passed",
        }
    }

    fn width(self) -> Constraint {
        match self {
            DeepSweColumn::Index => Constraint::Length(3),
            DeepSweColumn::Model => Constraint::Min(18),
            DeepSweColumn::Pass => Constraint::Length(7),
            DeepSweColumn::Cost => Constraint::Length(8),
            DeepSweColumn::Time => Constraint::Length(8),
            DeepSweColumn::Tokens => Constraint::Length(8),
            DeepSweColumn::Steps => Constraint::Length(6),
            DeepSweColumn::Passed => Constraint::Length(9),
        }
    }

    fn order(self) -> u8 {
        match self {
            DeepSweColumn::Index => 0,
            DeepSweColumn::Model => 1,
            DeepSweColumn::Pass => 2,
            DeepSweColumn::Cost => 3,
            DeepSweColumn::Time => 4,
            DeepSweColumn::Tokens => 5,
            DeepSweColumn::Steps => 6,
            DeepSweColumn::Passed => 7,
        }
    }
}

pub(super) fn deepswe_columns(width: u16, sort_key: DeepSweKey) -> Vec<DeepSweColumn> {
    let mut columns = vec![
        DeepSweColumn::Index,
        DeepSweColumn::Model,
        DeepSweColumn::Pass,
        DeepSweColumn::Cost,
    ];
    push_unique(&mut columns, deepswe_column_for_key(sort_key));
    if width >= 62 {
        push_unique(&mut columns, DeepSweColumn::Time);
    }
    if width >= 76 {
        push_unique(&mut columns, DeepSweColumn::Tokens);
    }
    if width >= 88 {
        push_unique(&mut columns, DeepSweColumn::Steps);
    }
    if width >= 100 {
        push_unique(&mut columns, DeepSweColumn::Passed);
    }
    columns.sort_by_key(|column| column.order());
    columns
}

fn deepswe_column_for_key(key: DeepSweKey) -> DeepSweColumn {
    match key {
        DeepSweKey::Pass => DeepSweColumn::Pass,
        DeepSweKey::Cost => DeepSweColumn::Cost,
        DeepSweKey::Time => DeepSweColumn::Time,
        DeepSweKey::Tokens => DeepSweColumn::Tokens,
        DeepSweKey::Steps => DeepSweColumn::Steps,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn render_deepswe_table(
    frame: &mut Frame,
    area: Rect,
    all_rows: &[deepswe::Row],
    indices: &[usize],
    table_state: &mut std::collections::HashMap<Board, TableState>,
    board: Board,
    sort_key: DeepSweKey,
    filter_tokens: &[String],
    compact: bool,
) {
    let columns = deepswe_columns(area.width, sort_key);
    let header = Row::new(columns.iter().map(|column| header_cell(column.header())));

    let table_rows = indices.iter().enumerate().filter_map(|(i, &idx)| {
        all_rows.get(idx).map(|row| {
            Row::new(
                columns
                    .iter()
                    .map(|column| deepswe_cell(*column, i, row, filter_tokens)),
            )
        })
    });

    let widths: Vec<Constraint> = columns.iter().map(|column| column.width()).collect();
    let title = format!("DeepSWE ({})", indices.len());

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

fn deepswe_cell(
    column: DeepSweColumn,
    index: usize,
    row: &deepswe::Row,
    filter_tokens: &[String],
) -> Cell<'static> {
    match column {
        DeepSweColumn::Index => {
            Cell::from(format!("{:>2}", index + 1)).style(Style::default().fg(Color::DarkGray))
        }
        DeepSweColumn::Model => Cell::from(highlight_matches(
            &row.display_model(),
            filter_tokens,
            Style::default().bold(),
        )),
        DeepSweColumn::Pass => {
            let pct = row.pass_rate.map(|v| v * 100.0);
            Cell::from(fmt_pct(row.pass_rate)).style(score_color(pct, 20.0, 50.0))
        }
        DeepSweColumn::Cost => {
            Cell::from(fmt_price(row.mean_cost_usd, 2, "")).style(price_color(
                row.mean_cost_usd,
                3.0,
                10.0,
            ))
        }
        DeepSweColumn::Time => Cell::from(fmt_duration(row.mean_duration_seconds))
            .style(Style::default().fg(Color::Blue)),
        DeepSweColumn::Tokens => Cell::from(fmt_compact_f(row.total_tokens())),
        DeepSweColumn::Steps => Cell::from(fmt_f(row.mean_agent_steps, 0)),
        DeepSweColumn::Passed => Cell::from(match (row.n_passed, row.n_attempted) {
            (Some(p), Some(a)) => format!("{p}/{a}"),
            _ => "-".into(),
        })
        .style(Style::default().fg(Color::DarkGray)),
    }
}

pub(super) fn render_deepswe_chart(
    frame: &mut Frame,
    area: Rect,
    all_rows: &[deepswe::Row],
    indices: &[usize],
    app: &AppState,
) {
    let key = app.deepswe_sort.key;
    let max_bars = chart_capacity(area);
    let chart_rows: Vec<ChartRow> = indices
        .iter()
        .filter_map(|&idx| {
            let row = all_rows.get(idx)?;
            let value = deepswe_metric(row, key)?;
            if !value.is_finite() {
                return None;
            }
            Some(ChartRow {
                name: row.display_model(),
                meta: String::new(),
                value,
                value_label: deepswe_chart_text_value(row, key),
                color: Some(deepswe_model_color(row)),
                color_seed: row.model.clone(),
            })
        })
        .take(max_bars)
        .collect();

    let title = format!(
        "DeepSWE chart - {} {} (top {}/{})",
        deepswe_key_label(key),
        app.deepswe_sort.dir.arrow(),
        chart_rows.len(),
        indices.len(),
    );
    let empty = format!(
        "no data for {} - switch sort key (a/p/t/u/s) or press m for table",
        deepswe_key_label(key)
    );
    let preference = deepswe_chart_preference(key);
    render_metric_chart(frame, area, &title, &empty, &chart_rows, preference);
}

pub(super) fn render_deepswe_detail(
    frame: &mut Frame,
    area: Rect,
    row: Option<&deepswe::Row>,
) {
    let max = area.width.saturating_sub(4) as usize;
    let lines = match row {
        Some(row) => vec![
            Line::styled(
                truncate(&row.display_model(), max.max(8)),
                Style::default().fg(Color::Cyan).bold(),
            ),
            detail_line("Model", &row.model, Style::default().fg(Color::DarkGray)),
            Line::from(""),
            detail_line(
                "Pass@1",
                fmt_pct(row.pass_rate),
                score_color(row.pass_rate.map(|v| v * 100.0), 20.0, 50.0),
            ),
            detail_line(
                "Cost",
                fmt_price(row.mean_cost_usd, 2, "/task"),
                price_color(row.mean_cost_usd, 3.0, 10.0),
            ),
            detail_line(
                "Time",
                fmt_duration(row.mean_duration_seconds),
                Style::default().fg(Color::Blue),
            ),
            detail_line(
                "Tokens",
                fmt_compact_f(row.total_tokens()),
                Style::default(),
            ),
            detail_line("Steps", fmt_f(row.mean_agent_steps, 0), Style::default()),
            detail_line(
                "Passed",
                match (row.n_passed, row.n_attempted) {
                    (Some(p), Some(a)) => format!("{p}/{a}"),
                    _ => "-".into(),
                },
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

fn deepswe_model_color(row: &deepswe::Row) -> Color {
    let lower = row.model.to_lowercase();
    if lower.contains("claude") {
        Color::Rgb(0xcc, 0x78, 0x5c)
    } else if lower.contains("gpt") || lower.contains("openai") {
        Color::Rgb(0x80, 0x80, 0x80)
    } else if lower.contains("gemini") {
        Color::Rgb(0x42, 0x85, 0xf4)
    } else if lower.contains("deepseek") {
        Color::Rgb(0x22, 0x43, 0xe6)
    } else if lower.contains("kimi") || lower.contains("moonshot") {
        Color::Rgb(0x04, 0x7a, 0xfe)
    } else if lower.contains("grok") {
        Color::Rgb(0x73, 0x6c, 0xd3)
    } else if lower.contains("glm") || lower.contains("zhipu") {
        Color::Rgb(0x1c, 0x7f, 0xf8)
    } else if lower.contains("qwen") || lower.contains("alibaba") {
        Color::Rgb(0xff, 0x70, 0x18)
    } else if lower.contains("mimo") || lower.contains("xiaomi") {
        Color::Rgb(0xff, 0x69, 0x00)
    } else if lower.contains("minimax") {
        Color::Rgb(0xeb, 0x35, 0x68)
    } else {
        super::color_for_seed(&row.model)
    }
}

fn deepswe_chart_preference(key: DeepSweKey) -> ChartPreference {
    match key {
        DeepSweKey::Cost | DeepSweKey::Time | DeepSweKey::Tokens => ChartPreference::Lower,
        DeepSweKey::Pass | DeepSweKey::Steps => ChartPreference::Higher,
    }
}

fn deepswe_chart_text_value(row: &deepswe::Row, key: DeepSweKey) -> String {
    match key {
        DeepSweKey::Pass => fmt_pct(row.pass_rate),
        DeepSweKey::Cost => fmt_price(row.mean_cost_usd, 2, "/task"),
        DeepSweKey::Time => fmt_duration(row.mean_duration_seconds),
        DeepSweKey::Tokens => fmt_compact_f(row.total_tokens()),
        DeepSweKey::Steps => fmt_f(row.mean_agent_steps, 0),
    }
}

pub(super) fn deepswe_key_label(k: DeepSweKey) -> &'static str {
    match k {
        DeepSweKey::Pass => "Pass@1",
        DeepSweKey::Cost => "Cost",
        DeepSweKey::Time => "Time",
        DeepSweKey::Tokens => "Tokens",
        DeepSweKey::Steps => "Steps",
    }
}

fn fmt_pct(v: Option<f64>) -> String {
    match v {
        Some(x) => format!("{:.0}%", x * 100.0),
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
        format!("{:.0}m", seconds / 60.0)
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
