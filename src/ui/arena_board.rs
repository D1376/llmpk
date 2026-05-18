use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Style, Stylize},
    text::Line,
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap},
    Frame,
};

use super::chart::{
    chart_capacity, render_chart_summary, render_metric_chart, split_chart_body, ChartPreference,
    ChartRow,
};
use super::{
    arena_metric, detail_line, fmt_f, fmt_price, fmt_tokens, format_compact, header_cell,
    price_color, push_unique, score_color, selected_row_style, truncate, AppState, ArenaKey,
};
use crate::arena;
use crate::board::Board;
use ratatui::widgets::TableState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ArenaColumn {
    Rank,
    Model,
    Organization,
    Rating,
    Votes,
    InputPrice,
    OutputPrice,
    ImagePrice,
    VideoPrice,
    Context,
    License,
}

impl ArenaColumn {
    fn header(self) -> &'static str {
        match self {
            ArenaColumn::Rank => "#",
            ArenaColumn::Model => "Model",
            ArenaColumn::Organization => "Org",
            ArenaColumn::Rating => "Rating",
            ArenaColumn::Votes => "Votes",
            ArenaColumn::InputPrice => "In $/M",
            ArenaColumn::OutputPrice => "Out $/M",
            ArenaColumn::ImagePrice => "$/img",
            ArenaColumn::VideoPrice => "$/sec",
            ArenaColumn::Context => "Ctx",
            ArenaColumn::License => "License",
        }
    }

    fn width(self) -> Constraint {
        match self {
            ArenaColumn::Rank => Constraint::Length(3),
            ArenaColumn::Model => Constraint::Min(12),
            ArenaColumn::Organization => Constraint::Length(12),
            ArenaColumn::Rating => Constraint::Length(7),
            ArenaColumn::Votes => Constraint::Length(7),
            ArenaColumn::InputPrice | ArenaColumn::OutputPrice => Constraint::Length(7),
            ArenaColumn::ImagePrice | ArenaColumn::VideoPrice => Constraint::Length(7),
            ArenaColumn::Context => Constraint::Length(7),
            ArenaColumn::License => Constraint::Length(12),
        }
    }

    fn order(self) -> u8 {
        match self {
            ArenaColumn::Rank => 0,
            ArenaColumn::Model => 1,
            ArenaColumn::Organization => 2,
            ArenaColumn::Rating => 3,
            ArenaColumn::Votes => 4,
            ArenaColumn::InputPrice => 5,
            ArenaColumn::OutputPrice => 6,
            ArenaColumn::ImagePrice => 7,
            ArenaColumn::VideoPrice => 8,
            ArenaColumn::Context => 9,
            ArenaColumn::License => 10,
        }
    }
}

pub(super) fn arena_columns(width: u16, kind: arena::Kind, sort_key: ArenaKey) -> Vec<ArenaColumn> {
    let mut columns = vec![
        ArenaColumn::Rank,
        ArenaColumn::Model,
        ArenaColumn::Rating,
        ArenaColumn::Votes,
    ];
    match kind {
        arena::Kind::Text => {
            push_unique(&mut columns, ArenaColumn::InputPrice);
            push_unique(&mut columns, ArenaColumn::OutputPrice);
            if width >= 74 {
                push_unique(&mut columns, ArenaColumn::Organization);
            }
            if width >= 88 || matches!(sort_key, ArenaKey::Context) {
                push_unique(&mut columns, ArenaColumn::Context);
            }
            if width >= 104 {
                push_unique(&mut columns, ArenaColumn::License);
            }
        }
        arena::Kind::Image => {
            push_unique(&mut columns, ArenaColumn::ImagePrice);
            if width >= 66 {
                push_unique(&mut columns, ArenaColumn::Organization);
            }
            if width >= 84 {
                push_unique(&mut columns, ArenaColumn::License);
            }
        }
        arena::Kind::Video => {
            push_unique(&mut columns, ArenaColumn::VideoPrice);
            if width >= 66 {
                push_unique(&mut columns, ArenaColumn::Organization);
            }
            if width >= 84 {
                push_unique(&mut columns, ArenaColumn::License);
            }
        }
    }
    columns.sort_by_key(|column| column.order());
    columns
}

pub(super) fn render_arena_table(
    frame: &mut Frame,
    area: Rect,
    entries: &[arena::Entry],
    table_state: &mut std::collections::HashMap<Board, TableState>,
    board: Board,
    sort_key: ArenaKey,
    kind: arena::Kind,
) {
    let columns = arena_columns(area.width, kind, sort_key);

    let header = Row::new(columns.iter().map(|column| header_cell(column.header())));

    let rows = entries
        .iter()
        .map(|entry| Row::new(columns.iter().map(|column| arena_cell(*column, entry))));

    let widths: Vec<Constraint> = columns.iter().map(|column| column.width()).collect();
    let title = format!("Arena ({})", entries.len());

    let table = Table::new(rows, widths)
        .header(header.height(1))
        .row_highlight_style(selected_row_style())
        .highlight_symbol("> ")
        .block(Block::default().borders(Borders::ALL).title(title))
        .column_spacing(1);

    let st = table_state.entry(board).or_default();
    frame.render_stateful_widget(table, area, st);
}

fn arena_cell(column: ArenaColumn, entry: &arena::Entry) -> Cell<'static> {
    match column {
        ArenaColumn::Rank => {
            let rank = entry
                .rank
                .map(|rank| rank.to_string())
                .unwrap_or_else(|| "-".into());
            Cell::from(format!("{rank:>3}")).style(Style::default().fg(Color::DarkGray))
        }
        ArenaColumn::Model => Cell::from(entry.name.clone()).style(Style::default().bold()),
        ArenaColumn::Organization => {
            Cell::from(entry.organization.clone().unwrap_or_else(|| "-".into()))
                .style(Style::default().fg(Color::Magenta))
        }
        ArenaColumn::Rating => {
            Cell::from(fmt_f(entry.rating, 1)).style(score_color(entry.rating, 1100.0, 1500.0))
        }
        ArenaColumn::Votes => Cell::from(
            entry
                .votes
                .map(format_compact)
                .unwrap_or_else(|| "-".into()),
        )
        .style(Style::default().fg(Color::Blue)),
        ArenaColumn::InputPrice => {
            Cell::from(fmt_f(entry.input_price, 2)).style(price_color(entry.input_price))
        }
        ArenaColumn::OutputPrice => {
            Cell::from(fmt_f(entry.output_price, 2)).style(price_color(entry.output_price))
        }
        ArenaColumn::ImagePrice => {
            Cell::from(fmt_f(entry.price_per_image, 3)).style(price_color(entry.price_per_image))
        }
        ArenaColumn::VideoPrice => {
            Cell::from(fmt_f(entry.price_per_second, 3)).style(price_color(entry.price_per_second))
        }
        ArenaColumn::Context => Cell::from(
            entry
                .context_length
                .map(fmt_tokens)
                .unwrap_or_else(|| "-".into()),
        ),
        ArenaColumn::License => Cell::from(entry.license.clone().unwrap_or_else(|| "-".into()))
            .style(Style::default().fg(Color::DarkGray)),
    }
}

pub(super) fn render_arena_detail(
    frame: &mut Frame,
    area: Rect,
    entry: Option<&arena::Entry>,
    board: Board,
) {
    let kind = match board {
        Board::Arena(s) => s.kind(),
        _ => arena::Kind::Text,
    };
    let max = area.width.saturating_sub(4) as usize;
    let lines = match entry {
        Some(entry) => {
            let mut lines = vec![
                Line::styled(
                    truncate(&entry.name, max.max(8)),
                    Style::default().fg(Color::Cyan).bold(),
                ),
                Line::from(""),
                detail_line(
                    "Rank",
                    entry
                        .rank
                        .map(|rank| rank.to_string())
                        .unwrap_or_else(|| "-".into()),
                    Style::default().fg(Color::Gray),
                ),
                detail_line(
                    "Org",
                    entry.organization.clone().unwrap_or_else(|| "-".into()),
                    Style::default().fg(Color::Magenta),
                ),
                detail_line(
                    "Rating",
                    fmt_f(entry.rating, 1),
                    score_color(entry.rating, 1100.0, 1500.0),
                ),
                detail_line(
                    "Votes",
                    entry
                        .votes
                        .map(format_compact)
                        .unwrap_or_else(|| "-".into()),
                    Style::default().fg(Color::Blue),
                ),
            ];
            match kind {
                arena::Kind::Text => {
                    lines.push(detail_line(
                        "Input",
                        fmt_price(entry.input_price, 2, "/M"),
                        price_color(entry.input_price),
                    ));
                    lines.push(detail_line(
                        "Output",
                        fmt_price(entry.output_price, 2, "/M"),
                        price_color(entry.output_price),
                    ));
                    lines.push(detail_line(
                        "Context",
                        entry
                            .context_length
                            .map(fmt_tokens)
                            .unwrap_or_else(|| "-".into()),
                        Style::default(),
                    ));
                }
                arena::Kind::Image => lines.push(detail_line(
                    "Price",
                    fmt_price(entry.price_per_image, 3, "/img"),
                    price_color(entry.price_per_image),
                )),
                arena::Kind::Video => lines.push(detail_line(
                    "Price",
                    fmt_price(entry.price_per_second, 3, "/sec"),
                    price_color(entry.price_per_second),
                )),
            }
            lines.push(detail_line(
                "License",
                entry.license.clone().unwrap_or_else(|| "-".into()),
                Style::default().fg(Color::Gray),
            ));
            lines
        }
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

pub(super) fn render_arena_chart(
    frame: &mut Frame,
    area: Rect,
    entries: &[arena::Entry],
    app: &AppState,
    board: Board,
) {
    let key = app.arena_sort.key;
    let kind = match board {
        Board::Arena(s) => s.kind(),
        _ => arena::Kind::Text,
    };
    let (chart_area, summary_area) = split_chart_body(area);
    let max_bars = chart_capacity(chart_area);
    let rows: Vec<ChartRow> = entries
        .iter()
        .filter_map(|entry| {
            let value = arena_metric(entry, key)?;
            let meta = entry.organization.clone().unwrap_or_else(|| "-".into());
            let color_seed = format!("{}:{meta}", entry.name);
            Some(ChartRow {
                name: entry.name.clone(),
                meta,
                value,
                value_label: arena_chart_text_value(entry, key, kind),
                color: None,
                color_seed,
            })
        })
        .take(max_bars)
        .collect();

    let title = format!(
        "{} chart - {} {} (top {}/{})",
        board.label(),
        arena_key_label(key),
        app.arena_sort.dir.arrow(),
        rows.len(),
        entries.len(),
    );
    let empty = format!(
        "no data for {} - switch sort key (n/i/v/p/c) or press m for table",
        arena_key_label(key)
    );
    let preference = arena_chart_preference(key);
    render_metric_chart(frame, chart_area, &title, &empty, &rows, preference);

    if let Some(summary_area) = summary_area {
        render_chart_summary(
            frame,
            summary_area,
            &rows,
            entries.len(),
            arena_key_label(key),
            preference,
        );
    }
}

fn arena_chart_preference(key: ArenaKey) -> ChartPreference {
    match key {
        ArenaKey::Rank | ArenaKey::Price => ChartPreference::Lower,
        ArenaKey::Rating | ArenaKey::Votes | ArenaKey::Context => ChartPreference::Higher,
    }
}

fn arena_chart_text_value(entry: &arena::Entry, key: ArenaKey, kind: arena::Kind) -> String {
    match key {
        ArenaKey::Rank => entry
            .rank
            .map(|rank| format!("#{rank}"))
            .unwrap_or_else(|| "-".into()),
        ArenaKey::Rating => fmt_f(entry.rating, 1),
        ArenaKey::Votes => entry
            .votes
            .map(format_compact)
            .unwrap_or_else(|| "-".into()),
        ArenaKey::Price => match kind {
            arena::Kind::Text => match (entry.input_price, entry.output_price) {
                (Some(input), Some(output)) => format!("${input:.2}/${output:.2}"),
                (Some(input), None) => format!("${input:.2} in"),
                (None, Some(output)) => format!("${output:.2} out"),
                (None, None) => "-".into(),
            },
            arena::Kind::Image => fmt_price(entry.price_per_image, 3, "/img"),
            arena::Kind::Video => fmt_price(entry.price_per_second, 3, "/sec"),
        },
        ArenaKey::Context => entry
            .context_length
            .map(fmt_tokens)
            .unwrap_or_else(|| "-".into()),
    }
}

pub(super) fn arena_key_label(k: ArenaKey) -> &'static str {
    match k {
        ArenaKey::Rank => "Rank",
        ArenaKey::Rating => "Rating",
        ArenaKey::Votes => "Votes",
        ArenaKey::Price => "Price",
        ArenaKey::Context => "Context",
    }
}
