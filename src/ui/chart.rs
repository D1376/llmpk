use ratatui::{
    layout::Rect,
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use super::{color_for_seed, truncate};

const HEADER_LINES: u16 = 1;
const RANK_WIDTH: usize = 3;
const VALUE_WIDTH: usize = 8;
const NAME_VALUE_GAP: usize = 2;
const MIN_META_WIDTH: usize = 3;
const MAX_META_WIDTH: usize = 18;
const MIN_NAME_WIDTH: usize = 8;
const MAX_NAME_WIDTH: usize = 34;
const MIN_BAR_WIDTH: usize = 8;
const MAX_BAR_WIDTH: usize = 48;

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

pub(super) fn chart_capacity(area: Rect) -> usize {
    area.height
        .saturating_sub(2)
        .saturating_sub(HEADER_LINES)
        .max(1) as usize
}

pub(super) fn render_metric_chart(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    empty: &str,
    rows: &[ChartRow],
    preference: ChartPreference,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .title_bottom(preference.hint());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if rows.is_empty() {
        let p = Paragraph::new(empty)
            .style(Style::default().fg(Color::Yellow))
            .wrap(Wrap { trim: true });
        frame.render_widget(p, inner);
        return;
    }

    let max_rows = row_capacity(inner);
    let lines = if horizontal_layout(inner.width as usize, rows, max_rows).is_some() {
        horizontal_chart_lines(rows, preference, inner)
    } else {
        compact_chart_lines(rows, preference, inner)
    };

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn compact_chart_lines(
    rows: &[ChartRow],
    preference: ChartPreference,
    area: Rect,
) -> Vec<Line<'static>> {
    let max_name = area.width.saturating_sub(14) as usize;
    let max_rows = row_capacity(area);
    let (min, max) = chart_bounds(rows);
    let mut lines = vec![Line::styled(
        format!(
            "Top {} visible | range {} - {} | {}",
            rows.len(),
            format_chart_number(min),
            format_chart_number(max),
            preference.hint()
        ),
        Style::default().fg(Color::DarkGray),
    )];

    for (i, row) in rows.iter().take(max_rows).enumerate() {
        let color = row.color.unwrap_or_else(|| color_for_seed(&row.color_seed));
        lines.push(Line::from(vec![
            Span::styled(
                format!("{:>2} ", i + 1),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                format!("{:<8} ", truncate(&row.value_label, 8)),
                Style::default().fg(color).bold(),
            ),
            Span::styled(truncate(&row.name, max_name), Style::default()),
        ]));
    }
    lines
}

fn horizontal_chart_lines(
    rows: &[ChartRow],
    preference: ChartPreference,
    area: Rect,
) -> Vec<Line<'static>> {
    let max_rows = row_capacity(area);
    let layout = horizontal_layout(area.width as usize, rows, max_rows).expect("checked by caller");
    let (min, max) = chart_bounds(rows);
    let mut lines = vec![Line::styled(
        format!(
            "Top {} visible | range {} - {} | {}",
            rows.len(),
            format_chart_number(min),
            format_chart_number(max),
            preference.hint()
        ),
        Style::default().fg(Color::DarkGray),
    )];

    for (i, row) in rows.iter().take(max_rows).enumerate() {
        let color = row.color.unwrap_or_else(|| color_for_seed(&row.color_seed));
        let bar_len = chart_bar_len(row.value, min, max, layout.bar_width, preference);
        let meta = truncate(&row.meta, layout.meta_width);
        let name = truncate(&row.name, layout.name_width);
        let value = truncate(&row.value_label, VALUE_WIDTH);
        let mut spans = Vec::new();
        spans.push(Span::styled(
            format!("{:>2} ", i + 1),
            Style::default().fg(Color::DarkGray),
        ));
        spans.push(Span::styled(
            format!("{:<width$} ", meta, width = layout.meta_width),
            Style::default().fg(color).bold(),
        ));
        spans.push(Span::styled(
            format!("{:<width$}", name, width = layout.name_width),
            Style::default(),
        ));
        spans.push(Span::raw(" ".repeat(NAME_VALUE_GAP)));
        spans.push(Span::styled(
            format!("{:>VALUE_WIDTH$} ", value),
            Style::default().fg(color).bold(),
        ));
        spans.push(Span::styled(
            "█".repeat(bar_len),
            Style::default().fg(color),
        ));
        lines.push(Line::from(spans));
    }
    lines
}

pub(super) fn chart_bounds(rows: &[ChartRow]) -> (f64, f64) {
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    for value in rows.iter().map(|row| row.value).filter(|v| v.is_finite()) {
        min = min.min(value);
        max = max.max(value);
    }
    if min.is_finite() && max.is_finite() {
        (min, max)
    } else {
        (0.0, 0.0)
    }
}

pub(super) fn chart_bar_len(
    value: f64,
    min: f64,
    max: f64,
    width: usize,
    preference: ChartPreference,
) -> usize {
    if width == 0 || !value.is_finite() || !min.is_finite() || !max.is_finite() {
        return 0;
    }
    if (max - min).abs() < f64::EPSILON {
        return width;
    }
    let normalized = match preference {
        ChartPreference::Higher => (value - min) / (max - min),
        ChartPreference::Lower => (max - value) / (max - min),
    };
    ((normalized.clamp(0.0, 1.0) * width as f64).round() as usize).clamp(1, width)
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

fn row_capacity(area: Rect) -> usize {
    area.height.saturating_sub(HEADER_LINES).max(1) as usize
}

#[derive(Debug, Clone, Copy)]
struct HorizontalLayout {
    meta_width: usize,
    name_width: usize,
    bar_width: usize,
}

fn horizontal_layout(width: usize, rows: &[ChartRow], max_rows: usize) -> Option<HorizontalLayout> {
    let longest_visible_meta = rows
        .iter()
        .take(max_rows)
        .map(|row| row.meta.chars().count())
        .max()
        .unwrap_or(MIN_META_WIDTH);
    let meta_width = longest_visible_meta.clamp(MIN_META_WIDTH, MAX_META_WIDTH);
    let fixed = RANK_WIDTH + meta_width + 1 + NAME_VALUE_GAP + VALUE_WIDTH + 1;
    let flexible = width.checked_sub(fixed)?;
    if flexible < MIN_NAME_WIDTH + MIN_BAR_WIDTH {
        return None;
    }

    let longest_visible_name = rows
        .iter()
        .take(max_rows)
        .map(|row| row.name.chars().count())
        .max()
        .unwrap_or(MIN_NAME_WIDTH);
    let max_name = MAX_NAME_WIDTH.min(flexible - MIN_BAR_WIDTH);
    let name_width = longest_visible_name.clamp(MIN_NAME_WIDTH, max_name);
    let bar_width = (flexible - name_width).clamp(MIN_BAR_WIDTH, MAX_BAR_WIDTH);
    Some(HorizontalLayout {
        meta_width,
        name_width,
        bar_width,
    })
}
