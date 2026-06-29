use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Style, Stylize},
    text::Line,
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap},
    Frame,
};

use super::chart::{chart_capacity, render_metric_chart, ChartPreference, ChartRow};
use super::{
    aa_metric, accent_color_for_provider, color_from_hex, detail_line, fmt_f, fmt_price,
    fmt_tokens, header_cell, highlight_matches, price_color, push_unique,
    readable_color_for_dark_bg, score_color, selected_row_style, truncate, AaKey, AppState,
};
use crate::aa;
use crate::board::Board;
use ratatui::widgets::TableState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AaColumn {
    Index,
    Model,
    Provider,
    Intelligence,
    Speed,
    Price,
    Context,
    Released,
    Open,
}

impl AaColumn {
    fn header(self) -> &'static str {
        match self {
            AaColumn::Index => "#",
            AaColumn::Model => "Model",
            AaColumn::Provider => "Provider",
            AaColumn::Intelligence => "Intel",
            AaColumn::Speed => "t/s",
            AaColumn::Price => "$/M",
            AaColumn::Context => "Ctx",
            AaColumn::Released => "Release",
            AaColumn::Open => "Open",
        }
    }

    fn width(self) -> Constraint {
        match self {
            AaColumn::Index => Constraint::Length(3),
            AaColumn::Model => Constraint::Min(12),
            AaColumn::Provider => Constraint::Length(11),
            AaColumn::Intelligence => Constraint::Length(6),
            AaColumn::Speed => Constraint::Length(6),
            AaColumn::Price => Constraint::Length(7),
            AaColumn::Context => Constraint::Length(7),
            AaColumn::Released => Constraint::Length(10),
            AaColumn::Open => Constraint::Length(5),
        }
    }

    fn order(self) -> u8 {
        match self {
            AaColumn::Index => 0,
            AaColumn::Model => 1,
            AaColumn::Provider => 2,
            AaColumn::Intelligence => 3,
            AaColumn::Speed => 4,
            AaColumn::Price => 5,
            AaColumn::Context => 6,
            AaColumn::Released => 7,
            AaColumn::Open => 8,
        }
    }
}

pub(super) fn aa_columns(width: u16, sort_key: AaKey) -> Vec<AaColumn> {
    let mut columns = vec![
        AaColumn::Index,
        AaColumn::Model,
        AaColumn::Intelligence,
        AaColumn::Price,
    ];
    push_unique(&mut columns, aa_column_for_key(sort_key));
    if width >= 62 {
        push_unique(&mut columns, AaColumn::Provider);
    }
    if width >= 74 {
        push_unique(&mut columns, AaColumn::Context);
    }
    if width >= 84 {
        push_unique(&mut columns, AaColumn::Speed);
    }
    if width >= 98 {
        push_unique(&mut columns, AaColumn::Open);
    }
    if width >= 110 {
        push_unique(&mut columns, AaColumn::Released);
    }
    columns.sort_by_key(|column| column.order());
    columns
}

fn aa_column_for_key(key: AaKey) -> AaColumn {
    match key {
        AaKey::Intelligence => AaColumn::Intelligence,
        AaKey::Speed => AaColumn::Speed,
        AaKey::Price => AaColumn::Price,
        AaKey::Context => AaColumn::Context,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn render_aa_table(
    frame: &mut Frame,
    area: Rect,
    all_models: &[aa::Model],
    indices: &[usize],
    table_state: &mut std::collections::HashMap<Board, TableState>,
    board: Board,
    sort_key: AaKey,
    filter_tokens: &[String],
    compact: bool,
) {
    let columns = aa_columns(area.width, sort_key);
    let header = Row::new(columns.iter().map(|column| header_cell(column.header())));

    let rows = indices.iter().enumerate().filter_map(|(i, &idx)| {
        all_models.get(idx).map(|m| {
            Row::new(
                columns
                    .iter()
                    .map(|column| aa_cell(*column, i, m, filter_tokens)),
            )
        })
    });

    let widths: Vec<Constraint> = columns.iter().map(|column| column.width()).collect();
    let title = format!("AA ({})", indices.len());

    let spacing = if compact { 0 } else { 1 };
    let table = Table::new(rows, widths)
        .header(header.height(1))
        .row_highlight_style(selected_row_style())
        .highlight_symbol("> ")
        .block(Block::default().borders(Borders::ALL).title(title))
        .column_spacing(spacing);

    let st = table_state.entry(board).or_default();
    frame.render_stateful_widget(table, area, st);
}

fn aa_cell(
    column: AaColumn,
    index: usize,
    model: &aa::Model,
    filter_tokens: &[String],
) -> Cell<'static> {
    match column {
        AaColumn::Index => {
            Cell::from(format!("{:>2}", index + 1)).style(Style::default().fg(Color::DarkGray))
        }
        AaColumn::Model => Cell::from(highlight_matches(
            &model.name,
            filter_tokens,
            Style::default().bold(),
        )),
        AaColumn::Provider => Cell::from(model.provider().to_string())
            .style(provider_style(model.provider(), aa_model_color(model))),
        AaColumn::Intelligence => Cell::from(fmt_f(model.intelligence_index, 1))
            .style(score_color(model.intelligence_index, 30.0, 60.0)),
        AaColumn::Speed => {
            Cell::from(fmt_f(model.speed(), 0)).style(Style::default().fg(Color::Blue))
        }
        AaColumn::Price => Cell::from(fmt_f(model.price_1m_blended_3_to_1, 2)).style(price_color(
            model.price_1m_blended_3_to_1,
            2.0,
            10.0,
        )),
        AaColumn::Context => Cell::from(
            model
                .context_window_tokens
                .map(fmt_tokens)
                .unwrap_or_else(|| "-".into()),
        ),
        AaColumn::Released => Cell::from(model.release_date.clone().unwrap_or_else(|| "-".into()))
            .style(Style::default().fg(Color::DarkGray)),
        AaColumn::Open => Cell::from(if model.is_open_weights == Some(true) {
            "yes"
        } else {
            ""
        })
        .style(Style::default().fg(Color::Green)),
    }
}

pub(super) fn render_aa_detail(frame: &mut Frame, area: Rect, model: Option<&aa::Model>) {
    let max = area.width.saturating_sub(4) as usize;
    let lines = match model {
        Some(model) => vec![
            Line::styled(
                truncate(&model.name, max.max(8)),
                Style::default().fg(Color::Cyan).bold(),
            ),
            detail_line(
                "ID",
                model.display_id(),
                Style::default().fg(Color::DarkGray),
            ),
            Line::from(""),
            detail_line(
                "Provider",
                model.provider(),
                provider_style(model.provider(), aa_model_color(model)),
            ),
            detail_line(
                "Intel",
                fmt_f(model.intelligence_index, 1),
                score_color(model.intelligence_index, 30.0, 60.0),
            ),
            detail_line(
                "Speed",
                format!("{} t/s", fmt_f(model.speed(), 0)),
                Style::default().fg(Color::Blue),
            ),
            detail_line(
                "Price",
                fmt_price(model.price_1m_blended_3_to_1, 2, "/M"),
                price_color(model.price_1m_blended_3_to_1, 2.0, 10.0),
            ),
            detail_line(
                "Context",
                model
                    .context_window_tokens
                    .map(fmt_tokens)
                    .unwrap_or_else(|| "-".into()),
                Style::default(),
            ),
            detail_line(
                "Release",
                model.release_date.clone().unwrap_or_else(|| "-".into()),
                Style::default().fg(Color::Gray),
            ),
            detail_line(
                "Weights",
                match model.is_open_weights {
                    Some(true) => "open",
                    Some(false) => "closed",
                    None => "-",
                },
                Style::default().fg(Color::Green),
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

pub(super) fn render_aa_chart(
    frame: &mut Frame,
    area: Rect,
    all_models: &[aa::Model],
    indices: &[usize],
    app: &AppState,
) {
    let key = app.aa_sort.key;
    let max_bars = chart_capacity(area);
    let rows: Vec<ChartRow> = indices
        .iter()
        .filter_map(|&idx| {
            let m = all_models.get(idx)?;
            let value = aa_metric(m, key)?;
            if !value.is_finite() {
                return None;
            }
            Some(ChartRow {
                name: m.name.clone(),
                meta: m.provider().to_string(),
                value,
                value_label: aa_chart_text_value(m, key),
                color: aa_model_color(m),
                color_seed: m.display_id().to_string(),
            })
        })
        .take(max_bars)
        .collect();

    let title = format!(
        "AA chart - {} {} (top {}/{})",
        aa_key_label(key),
        app.aa_sort.dir.arrow(),
        rows.len(),
        indices.len(),
    );
    let empty = format!(
        "no data for {} - switch sort key (i/s/p/c) or press m for table",
        aa_key_label(key)
    );
    let preference = aa_chart_preference(key);
    render_metric_chart(frame, area, &title, &empty, &rows, preference);
}

fn aa_model_color(model: &aa::Model) -> Option<Color> {
    model
        .provider_color()
        .and_then(color_from_hex)
        .map(readable_color_for_dark_bg)
        .or_else(|| accent_color_for_provider(model.provider()))
}

fn provider_style(provider: &str, color: Option<Color>) -> Style {
    Style::default()
        .fg(color.unwrap_or(Color::Magenta))
        .add_modifier(
            if accent_color_for_provider(provider).is_some() || color.is_some() {
                ratatui::style::Modifier::BOLD
            } else {
                ratatui::style::Modifier::empty()
            },
        )
}

fn aa_chart_preference(key: AaKey) -> ChartPreference {
    match key {
        AaKey::Price => ChartPreference::Lower,
        AaKey::Intelligence | AaKey::Speed | AaKey::Context => ChartPreference::Higher,
    }
}

fn aa_chart_text_value(m: &aa::Model, key: AaKey) -> String {
    match key {
        AaKey::Intelligence => fmt_f(m.intelligence_index, 1),
        AaKey::Speed => format!("{} t/s", fmt_f(m.speed(), 0)),
        AaKey::Price => format!("${}", fmt_f(m.price_1m_blended_3_to_1, 2)),
        AaKey::Context => m
            .context_window_tokens
            .map(fmt_tokens)
            .unwrap_or_else(|| "-".into()),
    }
}

pub(super) fn aa_key_label(k: AaKey) -> &'static str {
    match k {
        AaKey::Intelligence => "Intelligence",
        AaKey::Speed => "Speed",
        AaKey::Price => "Price",
        AaKey::Context => "Context",
    }
}
