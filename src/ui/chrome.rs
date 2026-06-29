use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs, Wrap},
    Frame,
};

use super::{
    aa_board, agents, deepswe_board, filter_is_active, format_filter_label, truncate, AppState,
    View, SPINNER_FRAMES, VERSION,
};
use crate::board::{Board, Data, Status};

pub(super) fn render_help_overlay(frame: &mut Frame, area: Rect) {
    let w = 82.min(area.width.saturating_sub(2));
    let popup = centered_rect(area, w, 42);
    frame.render_widget(ratatui::widgets::Clear, popup);

    let dim = Style::default().fg(Color::DarkGray);
    let head = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let lines = vec![
        Line::styled("Boards", head),
        help_row("AA", "artificialanalysis.ai composite ranking of LLMs"),
        help_row(
            "AA Agents",
            "artificialanalysis.ai coding-agent benchmark index",
        ),
        help_row("DeepSWE", "datacurve.ai DeepSWE coding-agent benchmark"),
        Line::from(""),
        Line::styled("Navigation", head),
        help_row("q  Esc  Ctrl-C", "Quit llmpk"),
        help_row("?  h", "Toggle this help overlay"),
        help_row("[   ]", "Previous / next board (cycles)"),
        help_row("1  2  3", "Jump directly to board 1-3"),
        help_row("r", "Reload the current board (refetch from source)"),
        help_row("y", "Copy selected model name to clipboard (OSC 52)"),
        help_row("↑  ↓  k  j", "Move the highlighted row"),
        help_row("PgUp  PgDn", "Move by 10 rows"),
        help_row("Home  End  g  G", "Jump to first / last row"),
        help_row("mouse scroll", "Scroll the table (when supported)"),
        Line::from(""),
        Line::styled("View & sort", head),
        help_row("m", "Toggle table / chart view"),
        help_row("o", "Reverse current sort direction (asc <-> desc)"),
        help_row("z", "Toggle compact mode (tighter column spacing)"),
        Line::styled("  AA sort keys", dim),
        help_row("i", "Intelligence Index — composite quality score"),
        help_row("s", "Output Speed — tokens generated per second"),
        help_row("p", "Blended Price — USD per 1M input+output tokens"),
        help_row("c", "Context Window — max tokens the model accepts"),
        Line::styled("  AA Agents sort keys", dim),
        help_row("i", "Index — Artificial Analysis Coding Agent Index"),
        help_row("a", "Pass@1 — mean benchmark reward"),
        help_row("p", "Cost — mean USD per task"),
        help_row("t", "Time — mean wall-clock task runtime"),
        help_row("u", "Tokens — mean total token usage per task"),
        help_row("s", "Turns — mean agent turns per task"),
        Line::styled("  DeepSWE sort keys", dim),
        help_row("a", "Pass@1 — attempt pass rate"),
        help_row("p", "Cost — mean USD per task"),
        help_row("t", "Time — mean duration in seconds"),
        help_row("u", "Tokens — mean total tokens"),
        help_row("s", "Steps — mean agent steps"),
        Line::from(""),
        Line::styled("Filter", head),
        help_row(
            "/",
            "Begin editing a substring filter for the current board",
        ),
        help_row("type", "Narrows visible rows in real time"),
        help_row("Backspace", "Delete a character while editing"),
        help_row("Ctrl-U", "Clear the filter"),
        help_row("Enter  Esc", "Finish editing (filter stays applied)"),
        Line::from(""),
        Line::styled(
            format!("llmpk v{VERSION}"),
            Style::default().fg(Color::DarkGray),
        ),
        Line::styled("Press ?, h, or Esc to close", dim),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Keybindings & metrics ")
        .style(Style::default().fg(Color::Gray));
    let p = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    frame.render_widget(p, popup);
}

fn help_row(keys: &str, desc: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("  {keys:<16}"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::raw(desc.to_string()),
    ])
}

pub(super) fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    Rect::new(x, y, w, h)
}

pub(super) fn render_tiny(frame: &mut Frame) {
    let p = Paragraph::new("llmpk needs a larger terminal")
        .style(Style::default().fg(Color::Yellow))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("llmpk v{VERSION}")),
        );
    frame.render_widget(p, frame.area());
}

pub(super) fn render_tabs(frame: &mut Frame, area: Rect, app: &AppState) {
    let titles: Vec<Line> = app
        .boards
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let key = Board::shortcut(i).unwrap_or(' ');
            Line::from(vec![
                Span::styled(
                    format!("{key}"),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                status_marker(app.status.get(b)),
                Span::raw(tab_label(*b)),
            ])
        })
        .collect();
    let tabs = Tabs::new(titles)
        .select(app.current)
        .block(Block::default().borders(Borders::ALL).title("Boards"))
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .divider(" ");
    frame.render_widget(tabs, area);
}

fn status_marker(status: Option<&Status>) -> Span<'static> {
    match status {
        Some(Status::Loading) => Span::styled(spinner_frame(), Style::default().fg(Color::Yellow)),
        Some(Status::Error(_)) => Span::styled("!", Style::default().fg(Color::Red)),
        _ => Span::raw(""),
    }
}

fn tab_label(board: Board) -> &'static str {
    match board {
        Board::Aa => "AA",
        Board::AaAgents => "AAg",
        Board::DeepSwe => "DSwe",
    }
}

pub(super) fn spinner_frame() -> &'static str {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let idx = (ms / 80) as usize % SPINNER_FRAMES.len();
    SPINNER_FRAMES[idx]
}

pub(super) fn render_header(frame: &mut Frame, area: Rect, app: &AppState) {
    let board = app.current_board();
    let query = app.current_filter();
    let status_span = match app.status.get(&board) {
        Some(Status::Loaded(Data::Aa(v))) => {
            let visible = app.row_count(board);
            let label = if filter_is_active(query) {
                format!("{visible}/{} models", v.len())
            } else {
                format!("{} models", v.len())
            };
            Span::styled(label, Style::default().fg(Color::Green))
        }
        Some(Status::Loaded(Data::AaAgents(v))) => {
            let visible = app.row_count(board);
            let label = if filter_is_active(query) {
                format!("{visible}/{} agents", v.len())
            } else {
                format!("{} agents", v.len())
            };
            Span::styled(label, Style::default().fg(Color::Green))
        }
        Some(Status::Loaded(Data::DeepSwe(v))) => {
            let visible = app.row_count(board);
            let label = if filter_is_active(query) {
                format!("{visible}/{} models", v.len())
            } else {
                format!("{} models", v.len())
            };
            Span::styled(label, Style::default().fg(Color::Green))
        }
        Some(Status::Loading) | None => Span::styled(
            format!("{} loading...", spinner_frame()),
            Style::default().fg(Color::Yellow),
        ),
        Some(Status::Error(e)) => Span::styled(
            format!("error: {e}"),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
    };

    let sort_label = match board {
        Board::Aa => format!(
            "sort: {} {}",
            aa_board::aa_key_label(app.aa_sort.key),
            app.aa_sort.dir.arrow()
        ),
        Board::AaAgents => format!(
            "sort: {} {}",
            agents::agent_key_label(app.agent_sort.key),
            app.agent_sort.dir.arrow()
        ),
        Board::DeepSwe => format!(
            "sort: {} {}",
            deepswe_board::deepswe_key_label(app.deepswe_sort.key),
            app.deepswe_sort.dir.arrow()
        ),
    };
    let source = match board {
        Board::Aa => "artificialanalysis.ai",
        Board::AaAgents => "artificialanalysis.ai/agents/coding-agents",
        Board::DeepSwe => "deepswe.datacurve.ai",
    };

    // Build segments in priority order (highest priority = last to drop).
    // Each segment is (spans, width_chars).
    let filter_segment = if app.is_filter_editing() || filter_is_active(query) {
        let label = format!(
            "filter: {}",
            format_filter_label(query, app.is_filter_editing())
        );
        let w = 6 + label.len(); // "  |  " + label
        Some((
            vec![
                Span::raw("  |  "),
                Span::styled(label, Style::default().fg(Color::Yellow)),
            ],
            w,
        ))
    } else {
        None
    };

    let sort_segment = {
        let w = 6 + sort_label.len();
        (
            vec![
                Span::raw("  |  "),
                Span::styled(sort_label, Style::default().fg(Color::Cyan)),
            ],
            w,
        )
    };
    let view_segment = {
        let vl = view_label(app.current_view());
        (
            vec![
                Span::raw("  |  "),
                Span::styled(vl, Style::default().fg(Color::Blue)),
            ],
            6 + vl.len(),
        )
    };
    let status_segment = {
        let sl = status_span.width();
        (vec![Span::raw("  |  "), status_span], 6 + sl)
    };
    let source_segment = {
        (
            vec![Span::styled(
                source,
                Style::default().add_modifier(Modifier::BOLD),
            )],
            source.len(),
        )
    };

    // Priority order: source (lowest), status, view, sort, filter (highest).
    // Drop from the start of this vec when overflowing.
    let mut segments: Vec<(Vec<Span>, usize)> =
        vec![source_segment, status_segment, view_segment, sort_segment];
    if let Some(f) = filter_segment {
        segments.push(f);
    }

    // Copy feedback — show "Copied!" for 2 seconds after clipboard copy.
    let copy_segment = app
        .copy_feedback_at
        .filter(|t| t.elapsed().as_secs() < 2)
        .map(|_| {
            let label = "Copied!";
            (
                vec![
                    Span::raw("  |  "),
                    Span::styled(
                        label,
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                ],
                6 + label.len(),
            )
        });
    if let Some(c) = copy_segment {
        segments.push(c);
    }

    // Compute available width (area minus borders).
    let avail = area.width.saturating_sub(2) as usize;
    // Drop lowest-priority segments until it fits.
    while segments.len() > 1 {
        let total: usize = segments.iter().map(|(_, w)| w).sum();
        if total <= avail {
            break;
        }
        segments.remove(0);
    }

    let spans: Vec<Span> = segments.into_iter().flat_map(|(s, _)| s).collect();
    let line = Line::from(spans);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!("llmpk v{VERSION} - {}", board.label()));
    frame.render_widget(Paragraph::new(line).block(block), area);
}

pub(super) fn render_filter_empty(frame: &mut Frame, area: Rect, query: &str) {
    let p = Paragraph::new(format!(
        "no rows match filter: {}\n\npress / to edit or Ctrl-U to clear",
        truncate(query, 48)
    ))
    .style(Style::default().fg(Color::Yellow))
    .block(Block::default().borders(Borders::ALL).title("Leaderboard"))
    .wrap(Wrap { trim: true });
    frame.render_widget(p, area);
}

pub(super) fn render_loading(frame: &mut Frame, area: Rect) {
    let spinner = spinner_frame();
    let inner = centered_rect(area, 44, 7);
    frame.render_widget(ratatui::widgets::Clear, inner);

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(
                spinner,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                "loading leaderboard data",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::styled(
            "  fetching from source, please wait...",
            Style::default().fg(Color::DarkGray),
        ),
        Line::from(""),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            " llmpk ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
    let p = Paragraph::new(lines).block(block);
    frame.render_widget(p, inner);
}

pub(super) fn render_error(frame: &mut Frame, area: Rect, error: &str) {
    let inner = centered_rect(area, 52, 9);
    frame.render_widget(ratatui::widgets::Clear, inner);

    let max = inner.width.saturating_sub(4).max(8) as usize;
    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(
                "\u{2717}",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                "failed to load leaderboard",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::styled(
            format!("  {}", truncate(error, max.saturating_sub(2))),
            Style::default().fg(Color::DarkGray),
        ),
        Line::from(""),
        Line::from(vec![
            Span::raw("  press "),
            Span::styled(
                "r",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" to retry"),
        ]),
        Line::from(""),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red))
        .title(Span::styled(
            " Error ",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ));
    let p = Paragraph::new(lines).block(block).wrap(Wrap { trim: true });
    frame.render_widget(p, inner);
}

pub(super) fn render_footer(frame: &mut Frame, area: Rect, app: &AppState) {
    if app.is_filter_editing() {
        let line = vec![
            key("type"),
            text(" narrow filter  "),
            key("Backspace"),
            text(" delete char  "),
            key("Ctrl-U"),
            text(" clear filter  "),
            key("Enter/Esc"),
            text(" done"),
        ];
        let p = Paragraph::new(Line::from(line)).style(Style::default().fg(Color::Gray));
        frame.render_widget(p, area);
        return;
    }

    let board = app.current_board();
    let total = app.row_count(board);
    let selected = app.selected_idx();
    let unfiltered = match app.status.get(&board) {
        Some(Status::Loaded(Data::Aa(v))) => v.len(),
        Some(Status::Loaded(Data::AaAgents(v))) => v.len(),
        Some(Status::Loaded(Data::DeepSwe(v))) => v.len(),
        _ => 0,
    };
    let filter_info = if filter_is_active(app.current_filter()) && total != unfiltered {
        format!("  showing {total}/{unfiltered}")
    } else {
        String::new()
    };
    let pos = if total > 0 && app.current_view() == View::Table {
        format!("  {}/{}", selected + 1, total)
    } else {
        String::new()
    };

    let nav = vec![
        key("? / h"),
        text(" help  "),
        key("q"),
        text(" quit  "),
        key("[ ]"),
        text(" board  "),
        key("j/k"),
        text(" row  "),
        key("g/G"),
        text(" top/bot  "),
        key("r"),
        text(" reload  "),
        key("y"),
        text(" copy"),
        Span::styled(pos, Style::default().fg(Color::Cyan)),
        Span::styled(filter_info, Style::default().fg(Color::Yellow)),
    ];

    let sort_keys = match app.current_board() {
        Board::Aa => "i/s/p/c",
        Board::AaAgents => "i/a/p/t/u/s",
        Board::DeepSwe => "a/p/t/u/s",
    };
    let view_action = match app.current_view() {
        View::Table => " chart  ",
        View::Chart => " table  ",
    };
    let mut actions = vec![
        key(sort_keys),
        text(" sort  "),
        key("o"),
        text(" asc/desc  "),
        key("m"),
        text(view_action),
        key("/"),
        text(" filter"),
    ];
    if filter_is_active(app.current_filter()) {
        actions.push(text("  "));
        actions.push(key("Ctrl-U"));
        actions.push(text(" clear filter"));
    }

    let p = Paragraph::new(vec![Line::from(nav), Line::from(actions)])
        .style(Style::default().fg(Color::Gray));
    frame.render_widget(p, area);
}

fn key(s: &str) -> Span<'_> {
    Span::styled(
        s,
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )
}

fn text(s: &str) -> Span<'_> {
    Span::raw(s)
}

fn view_label(view: View) -> &'static str {
    match view {
        View::Table => "view: table",
        View::Chart => "view: chart",
    }
}
