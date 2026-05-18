use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    text::Line,
    widgets::{Bar, BarChart, BarGroup, Block, Borders, Paragraph, Wrap},
    Frame,
};

use super::{color_for_seed, detail_line, truncate};

#[derive(Debug, Clone)]
pub(super) struct ChartRow {
    pub name: String,
    pub meta: String,
    pub value: f64,
    pub value_label: String,
    pub color: Option<Color>,
    pub color_seed: String,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum ChartPreference {
    Higher,
    Lower,
}

impl ChartPreference {
    pub(super) fn hint(self) -> &'static str {
        match self {
            ChartPreference::Higher => "higher is better",
            ChartPreference::Lower => "lower is better",
        }
    }
}

pub(super) fn split_chart_body(area: Rect) -> (Rect, Option<Rect>) {
    if area.width < 118 || area.height < 12 {
        return (area, None);
    }

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(72), Constraint::Length(34)])
        .split(area);
    (chunks[0], Some(chunks[1]))
}

pub(super) fn chart_capacity(area: Rect) -> usize {
    (area.height as usize).saturating_sub(2).max(1)
}

pub(super) fn render_metric_chart(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    empty: &str,
    rows: &[ChartRow],
    preference: ChartPreference,
) {
    if rows.is_empty() {
        let p = Paragraph::new(empty.to_string())
            .style(Style::default().fg(Color::Yellow))
            .block(Block::default().borders(Borders::ALL).title("Chart"))
            .wrap(Wrap { trim: true });
        frame.render_widget(p, area);
        return;
    }

    let (min, max) = chart_bounds(rows);
    let label_width = (area.width as usize).saturating_sub(24).clamp(10, 34);
    let bars: Vec<Bar> = rows
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let scaled = chart_bar_value(row.value, min, max, preference);
            let label = format!("{:>2}. {}", i + 1, truncate(&row.name, label_width));
            let color = row.color.unwrap_or_else(|| color_for_seed(&row.color_seed));
            Bar::default()
                .label(Line::from(label))
                .value(scaled)
                .text_value(row.value_label.clone())
                .style(Style::default().fg(color))
                .value_style(Style::default().fg(Color::Black).bg(color))
        })
        .collect();

    let chart = BarChart::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .title_bottom(preference.hint()),
        )
        .data(BarGroup::default().bars(&bars))
        .direction(Direction::Horizontal)
        .bar_width(1)
        .bar_gap(0)
        .label_style(Style::default().fg(Color::Gray));

    frame.render_widget(chart, area);
}

pub(super) fn render_chart_summary(
    frame: &mut Frame,
    area: Rect,
    rows: &[ChartRow],
    total: usize,
    metric: &str,
    preference: ChartPreference,
) {
    let lines = if let Some(lead) = rows.first() {
        let (min, max) = chart_bounds(rows);
        vec![
            detail_line("Metric", metric, Style::default().fg(Color::Cyan).bold()),
            detail_line("Mode", preference.hint(), Style::default().fg(Color::Gray)),
            detail_line(
                "Shown",
                format!("{}/{}", rows.len(), total),
                Style::default().fg(Color::Green),
            ),
            Line::from(""),
            Line::styled("Top row", Style::default().fg(Color::DarkGray)),
            Line::styled(
                truncate(&lead.name, area.width.saturating_sub(4) as usize),
                Style::default().fg(Color::Cyan).bold(),
            ),
            detail_line(
                "Value",
                lead.value_label.clone(),
                Style::default().fg(Color::Yellow),
            ),
            detail_line(
                "Group",
                lead.meta.clone(),
                Style::default().fg(Color::Magenta),
            ),
            Line::from(""),
            detail_line(
                "Range",
                format!(
                    "{} - {}",
                    format_chart_number(min),
                    format_chart_number(max)
                ),
                Style::default().fg(Color::Gray),
            ),
        ]
    } else {
        vec![Line::styled(
            "No chartable rows",
            Style::default().fg(Color::DarkGray),
        )]
    };

    let p = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("Chart stats"))
        .wrap(Wrap { trim: true });
    frame.render_widget(p, area);
}

pub(super) fn chart_bounds(rows: &[ChartRow]) -> (f64, f64) {
    rows.iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), row| {
            (min.min(row.value), max.max(row.value))
        })
}

pub(super) fn chart_bar_value(value: f64, min: f64, max: f64, preference: ChartPreference) -> u64 {
    if !min.is_finite() || !max.is_finite() || (max - min).abs() < f64::EPSILON {
        return 1000;
    }

    let normalized = match preference {
        ChartPreference::Higher => (value - min) / (max - min),
        ChartPreference::Lower => (max - value) / (max - min),
    };
    (normalized.clamp(0.0, 1.0) * 999.0).round() as u64 + 1
}

pub(super) fn format_chart_number(value: f64) -> String {
    if value.abs() >= 1_000_000.0 {
        format!("{:.1}M", value / 1_000_000.0)
    } else if value.abs() >= 10_000.0 {
        format!("{:.1}K", value / 1_000.0)
    } else if value.abs() >= 100.0 {
        format!("{value:.0}")
    } else if value.abs() >= 10.0 {
        format!("{value:.1}")
    } else {
        format!("{value:.2}")
    }
}
