use std::cmp::Ordering;
use std::collections::HashMap;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{
        Bar, BarChart, BarGroup, Block, Borders, Cell, Paragraph, Row, Table, TableState, Tabs,
        Wrap,
    },
    Frame,
};

use crate::aa;
use crate::arena;
use crate::board::{Board, Data, Status};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDir {
    Desc,
    Asc,
}

impl SortDir {
    fn arrow(self) -> &'static str {
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

/// Sort key referenced by a stable identifier; meaning depends on board.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AaKey {
    Intelligence,
    Speed,
    Price,
    Context,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArenaKey {
    Rank,
    Rating,
    Votes,
    Price,
    Context,
}

#[derive(Debug, Clone)]
pub struct AaSort {
    pub key: AaKey,
    pub dir: SortDir,
}

#[derive(Debug, Clone)]
pub struct ArenaSort {
    pub key: ArenaKey,
    pub dir: SortDir,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Table,
    Chart,
}

pub struct AppState {
    pub boards: Vec<Board>,
    pub current: usize,
    pub status: HashMap<Board, Status>,
    pub table_state: HashMap<Board, TableState>,
    pub aa_sort: AaSort,
    pub arena_sort: ArenaSort,
    pub aa_view: View,
}

impl AppState {
    pub fn new() -> Self {
        let boards = Board::all();
        Self {
            boards,
            current: 0,
            status: HashMap::new(),
            table_state: HashMap::new(),
            aa_sort: AaSort {
                key: AaKey::Intelligence,
                dir: SortDir::Desc,
            },
            arena_sort: ArenaSort {
                key: ArenaKey::Rank,
                dir: SortDir::Asc,
            },
            aa_view: View::Table,
        }
    }

    pub fn toggle_aa_view(&mut self) {
        self.aa_view = match self.aa_view {
            View::Table => View::Chart,
            View::Chart => View::Table,
        };
    }

    pub fn current_board(&self) -> Board {
        self.boards[self.current]
    }

    pub fn set_status(&mut self, board: Board, status: Status) {
        if let Status::Loaded(data) = &status {
            let mut data = data.clone();
            self.sort_data(&mut data);
            self.status.insert(board, Status::Loaded(data));
            let st = self.table_state.entry(board).or_default();
            st.select(Some(0));
        } else {
            self.status.insert(board, status);
        }
    }

    pub fn select_board(&mut self, idx: usize) {
        if idx >= self.boards.len() {
            return;
        }
        self.current = idx;
    }

    pub fn cycle_board(&mut self, delta: i32) {
        let n = self.boards.len() as i32;
        let next = ((self.current as i32 + delta).rem_euclid(n)) as usize;
        self.current = next;
    }

    pub fn move_down(&mut self) {
        let board = self.current_board();
        let len = self.row_count(board);
        if len == 0 {
            return;
        }
        let st = self.table_state.entry(board).or_default();
        let i = st.selected().map(|i| i + 1).unwrap_or(0).min(len - 1);
        st.select(Some(i));
    }

    pub fn move_up(&mut self) {
        let board = self.current_board();
        let st = self.table_state.entry(board).or_default();
        let i = st.selected().unwrap_or(0).saturating_sub(1);
        st.select(Some(i));
    }

    fn row_count(&self, board: Board) -> usize {
        match self.status.get(&board) {
            Some(Status::Loaded(Data::Aa(v))) => v.len(),
            Some(Status::Loaded(Data::Arena(v))) => v.len(),
            _ => 0,
        }
    }

    pub fn cycle_sort(&mut self, key: char) {
        let board = self.current_board();
        match board {
            Board::Aa => {
                let new_key = match key {
                    'i' => Some(AaKey::Intelligence),
                    's' => Some(AaKey::Speed),
                    'p' => Some(AaKey::Price),
                    'c' => Some(AaKey::Context),
                    _ => None,
                };
                if let Some(k) = new_key {
                    if self.aa_sort.key == k {
                        self.aa_sort.dir = self.aa_sort.dir.toggle();
                    } else {
                        self.aa_sort.key = k;
                        self.aa_sort.dir = if matches!(k, AaKey::Price) {
                            SortDir::Asc
                        } else {
                            SortDir::Desc
                        };
                    }
                }
            }
            Board::Arena(_) => {
                let new_key = match key {
                    'k' | 'n' => Some(ArenaKey::Rank),
                    'i' => Some(ArenaKey::Rating),
                    'v' => Some(ArenaKey::Votes),
                    'p' => Some(ArenaKey::Price),
                    'c' => Some(ArenaKey::Context),
                    _ => None,
                };
                if let Some(k) = new_key {
                    if self.arena_sort.key == k {
                        self.arena_sort.dir = self.arena_sort.dir.toggle();
                    } else {
                        self.arena_sort.key = k;
                        self.arena_sort.dir = if matches!(k, ArenaKey::Rank | ArenaKey::Price) {
                            SortDir::Asc
                        } else {
                            SortDir::Desc
                        };
                    }
                }
            }
        }
        self.resort_current();
    }

    pub fn toggle_dir(&mut self) {
        match self.current_board() {
            Board::Aa => self.aa_sort.dir = self.aa_sort.dir.toggle(),
            Board::Arena(_) => self.arena_sort.dir = self.arena_sort.dir.toggle(),
        }
        self.resort_current();
    }

    fn resort_current(&mut self) {
        let board = self.current_board();
        if let Some(Status::Loaded(data)) = self.status.get_mut(&board) {
            let mut taken = std::mem::replace(data, Data::Aa(Vec::new()));
            let aa = self.aa_sort.clone();
            let arena = self.arena_sort.clone();
            sort_with(&mut taken, &aa, &arena);
            *data = taken;
        }
        let has_rows = self.row_count(board) > 0;
        if let Some(st) = self.table_state.get_mut(&board) {
            st.select(if has_rows { Some(0) } else { None });
        }
    }

    fn sort_data(&self, data: &mut Data) {
        sort_with(data, &self.aa_sort, &self.arena_sort);
    }
}

fn sort_with(data: &mut Data, aa_sort: &AaSort, arena_sort: &ArenaSort) {
    match data {
        Data::Aa(models) => sort_aa(models, aa_sort),
        Data::Arena(entries) => sort_arena(entries, arena_sort),
    }
}

fn sort_aa(models: &mut [aa::Model], sort: &AaSort) {
    let key = sort.key;
    let dir = sort.dir;
    models.sort_by(|a, b| {
        let av = aa_metric(a, key);
        let bv = aa_metric(b, key);
        let ord = cmp_opt(av, bv);
        match dir {
            SortDir::Desc => ord.reverse(),
            SortDir::Asc => ord,
        }
    });
}

fn aa_metric(m: &aa::Model, key: AaKey) -> Option<f64> {
    match key {
        AaKey::Intelligence => m.intelligence_index,
        AaKey::Speed => m.speed(),
        AaKey::Price => m.price_1m_blended_3_to_1,
        AaKey::Context => m.context_window_tokens.map(|x| x as f64),
    }
}

fn sort_arena(entries: &mut [arena::Entry], sort: &ArenaSort) {
    let key = sort.key;
    let dir = sort.dir;
    entries.sort_by(|a, b| {
        let av = arena_metric(a, key);
        let bv = arena_metric(b, key);
        let ord = cmp_opt(av, bv);
        match dir {
            SortDir::Desc => ord.reverse(),
            SortDir::Asc => ord,
        }
    });
}

fn arena_metric(e: &arena::Entry, key: ArenaKey) -> Option<f64> {
    match key {
        ArenaKey::Rank => e.rank.map(|x| x as f64),
        ArenaKey::Rating => e.rating,
        ArenaKey::Votes => e.votes.map(|x| x as f64),
        ArenaKey::Price => e
            .input_price
            .or(e.output_price)
            .or(e.price_per_image)
            .or(e.price_per_second),
        ArenaKey::Context => e.context_length.map(|x| x as f64),
    }
}

fn cmp_opt(a: Option<f64>, b: Option<f64>) -> Ordering {
    match (a, b) {
        (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(Ordering::Equal),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

pub fn render(frame: &mut Frame, app: &mut AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(2),
        ])
        .split(frame.area());

    render_tabs(frame, chunks[0], app);
    render_header(frame, chunks[1], app);
    render_body(frame, chunks[2], app);
    render_footer(frame, chunks[3], app);
}

fn render_tabs(frame: &mut Frame, area: Rect, app: &AppState) {
    let titles: Vec<Line> = app
        .boards
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let key = b.shortcut(i).unwrap_or(' ');
            Line::from(vec![
                Span::styled(
                    format!("{key}"),
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                ),
                Span::raw(":"),
                Span::raw(b.label()),
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
        .divider(" | ");
    frame.render_widget(tabs, area);
}

fn render_header(frame: &mut Frame, area: Rect, app: &AppState) {
    let board = app.current_board();
    let status_span = match app.status.get(&board) {
        Some(Status::Loaded(Data::Aa(v))) => Span::styled(
            format!("{} models", v.len()),
            Style::default().fg(Color::Green),
        ),
        Some(Status::Loaded(Data::Arena(v))) => Span::styled(
            format!("{} entries", v.len()),
            Style::default().fg(Color::Green),
        ),
        Some(Status::Loading) | None => {
            Span::styled("loading...", Style::default().fg(Color::Yellow))
        }
        Some(Status::Error(e)) => Span::styled(
            format!("error: {e}"),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
    };

    let sort_label = match board {
        Board::Aa => format!(
            "sort: {} {}",
            aa_key_label(app.aa_sort.key),
            app.aa_sort.dir.arrow()
        ),
        Board::Arena(_) => format!(
            "sort: {} {}",
            arena_key_label(app.arena_sort.key),
            app.arena_sort.dir.arrow()
        ),
    };
    let source = match board {
        Board::Aa => "artificialanalysis.ai".to_string(),
        Board::Arena(s) => format!("arena.ai/leaderboard/{}", s.path()),
    };

    let line = Line::from(vec![
        Span::styled(source, Style::default().add_modifier(Modifier::BOLD)),
        Span::raw("    "),
        status_span,
        Span::raw("    "),
        Span::styled(sort_label, Style::default().fg(Color::Cyan)),
    ]);
    let block = Block::default().borders(Borders::ALL).title("llmpk");
    frame.render_widget(Paragraph::new(line).block(block), area);
}

fn render_body(frame: &mut Frame, area: Rect, app: &mut AppState) {
    let board = app.current_board();
    match app.status.get(&board).cloned() {
        Some(Status::Loaded(Data::Aa(models))) => match app.aa_view {
            View::Table => render_aa_table(frame, area, &models, app, board),
            View::Chart => render_aa_chart(frame, area, &models, app),
        },
        Some(Status::Loaded(Data::Arena(entries))) => {
            render_arena_table(frame, area, &entries, app, board)
        }
        Some(Status::Error(e)) => {
            let p = Paragraph::new(format!("error: {e}\n\npress r to retry"))
                .style(Style::default().fg(Color::Red))
                .block(Block::default().borders(Borders::ALL).title("Leaderboard"))
                .wrap(Wrap { trim: true });
            frame.render_widget(p, area);
        }
        Some(Status::Loading) | None => {
            let p = Paragraph::new("loading...")
                .style(Style::default().fg(Color::Yellow))
                .block(Block::default().borders(Borders::ALL).title("Leaderboard"));
            frame.render_widget(p, area);
        }
    }
}

fn render_aa_table(
    frame: &mut Frame,
    area: Rect,
    models: &[aa::Model],
    app: &mut AppState,
    board: Board,
) {
    let header = Row::new(
        [
            "#",
            "Model",
            "Provider",
            "Intel",
            "Speed t/s",
            "Price $/M",
            "Context",
            "Released",
            "Open",
        ]
        .into_iter()
        .map(|h| {
            Cell::from(h).style(
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
        }),
    );

    let rows = models.iter().enumerate().map(|(i, m)| {
        let intel_style = score_color(m.intelligence_index, 30.0, 60.0);
        let price_style = price_color(m.price_1m_blended_3_to_1);
        Row::new(vec![
            Cell::from(format!("{:>2}", i + 1)).style(Style::default().fg(Color::DarkGray)),
            Cell::from(m.name.clone()).style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from(m.provider().to_string()).style(Style::default().fg(Color::Magenta)),
            Cell::from(fmt_f(m.intelligence_index, 1)).style(intel_style),
            Cell::from(fmt_f(m.speed(), 0)).style(Style::default().fg(Color::Blue)),
            Cell::from(fmt_f(m.price_1m_blended_3_to_1, 2)).style(price_style),
            Cell::from(
                m.context_window_tokens
                    .map(fmt_tokens)
                    .unwrap_or_else(|| "-".into()),
            ),
            Cell::from(m.release_date.clone().unwrap_or_else(|| "-".into()))
                .style(Style::default().fg(Color::DarkGray)),
            Cell::from(if m.is_open_weights == Some(true) { "yes" } else { "" })
                .style(Style::default().fg(Color::Green)),
        ])
    });

    let widths = [
        Constraint::Length(3),
        Constraint::Min(28),
        Constraint::Length(12),
        Constraint::Length(7),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(8),
        Constraint::Length(11),
        Constraint::Length(5),
    ];

    let table = Table::new(rows, widths)
        .header(header.height(1))
        .row_highlight_style(Style::default().bg(Color::DarkGray).bold())
        .highlight_symbol("> ")
        .block(Block::default().borders(Borders::ALL).title("Leaderboard"));

    let st = app.table_state.entry(board).or_default();
    frame.render_stateful_widget(table, area, st);
}

fn render_aa_chart(frame: &mut Frame, area: Rect, models: &[aa::Model], app: &AppState) {
    let key = app.aa_sort.key;
    let max_bars = (area.height as usize).saturating_sub(2).max(1);
    let displayed: Vec<&aa::Model> = models
        .iter()
        .filter(|m| aa_metric(m, key).is_some())
        .take(max_bars)
        .collect();

    if displayed.is_empty() {
        let p = Paragraph::new(format!(
            "no data for {} — switch sort key (i/s/p/c) or press v for table",
            aa_key_label(key)
        ))
        .style(Style::default().fg(Color::Yellow))
        .block(Block::default().borders(Borders::ALL).title("Leaderboard chart"));
        frame.render_widget(p, area);
        return;
    }

    let scale = chart_scale(key);
    let bars: Vec<Bar> = displayed
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let v = aa_metric(m, key).unwrap_or(0.0).max(0.0);
            let scaled = ((v * scale) as u64).max(1);
            let label = format!("{:>2}. {}", i + 1, truncate(&m.name, 26));
            let color = model_color(m);
            Bar::default()
                .label(Line::from(label))
                .value(scaled)
                .text_value(chart_text_value(m, key))
                .style(Style::default().fg(color))
                .value_style(Style::default().fg(Color::Black).bg(color))
        })
        .collect();

    let title = format!(
        "Leaderboard chart — {} {} (top {}/{})",
        aa_key_label(key),
        app.aa_sort.dir.arrow(),
        displayed.len(),
        models.len(),
    );

    let chart = BarChart::default()
        .block(Block::default().borders(Borders::ALL).title(title))
        .data(BarGroup::default().bars(&bars))
        .direction(Direction::Horizontal)
        .bar_width(1)
        .bar_gap(0)
        .label_style(Style::default().fg(Color::Gray));

    frame.render_widget(chart, area);
}

fn chart_scale(key: AaKey) -> f64 {
    match key {
        AaKey::Intelligence => 100.0,
        AaKey::Speed => 10.0,
        AaKey::Price => 100.0,
        AaKey::Context => 1.0,
    }
}

fn chart_text_value(m: &aa::Model, key: AaKey) -> String {
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

fn model_color(m: &aa::Model) -> Color {
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
    for b in m.id.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    PALETTE[(h as usize) % PALETTE.len()]
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(n.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

fn render_arena_table(
    frame: &mut Frame,
    area: Rect,
    entries: &[arena::Entry],
    app: &mut AppState,
    board: Board,
) {
    let kind = match board {
        Board::Arena(s) => s.kind(),
        _ => arena::Kind::Text,
    };

    let mut headers: Vec<&'static str> = vec!["#", "Model", "Org", "Rating", "Votes"];
    match kind {
        arena::Kind::Text => headers.extend(["In $/M", "Out $/M", "Context", "License"]),
        arena::Kind::Image => headers.extend(["$/img", "License"]),
        arena::Kind::Video => headers.extend(["$/sec", "License"]),
    }

    let header = Row::new(headers.into_iter().map(|h| {
        Cell::from(h).style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
    }));

    let rows = entries.iter().map(|e| {
        let rank = e
            .rank
            .map(|x| format!("{x}"))
            .unwrap_or_else(|| "-".into());
        let rating_style = score_color(e.rating, 1100.0, 1500.0);
        let mut cells = vec![
            Cell::from(format!("{rank:>3}")).style(Style::default().fg(Color::DarkGray)),
            Cell::from(e.name.clone()).style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from(e.organization.clone().unwrap_or_else(|| "-".into()))
                .style(Style::default().fg(Color::Magenta)),
            Cell::from(fmt_f(e.rating, 1)).style(rating_style),
            Cell::from(
                e.votes
                    .map(|v| format_compact(v))
                    .unwrap_or_else(|| "-".into()),
            )
            .style(Style::default().fg(Color::Blue)),
        ];
        match kind {
            arena::Kind::Text => {
                cells.push(
                    Cell::from(fmt_f(e.input_price, 2)).style(price_color(e.input_price)),
                );
                cells.push(
                    Cell::from(fmt_f(e.output_price, 2)).style(price_color(e.output_price)),
                );
                cells.push(Cell::from(
                    e.context_length
                        .map(fmt_tokens)
                        .unwrap_or_else(|| "-".into()),
                ));
                cells.push(
                    Cell::from(e.license.clone().unwrap_or_else(|| "-".into()))
                        .style(Style::default().fg(Color::DarkGray)),
                );
            }
            arena::Kind::Image => {
                cells.push(
                    Cell::from(fmt_f(e.price_per_image, 3))
                        .style(price_color(e.price_per_image)),
                );
                cells.push(
                    Cell::from(e.license.clone().unwrap_or_else(|| "-".into()))
                        .style(Style::default().fg(Color::DarkGray)),
                );
            }
            arena::Kind::Video => {
                cells.push(
                    Cell::from(fmt_f(e.price_per_second, 3))
                        .style(price_color(e.price_per_second)),
                );
                cells.push(
                    Cell::from(e.license.clone().unwrap_or_else(|| "-".into()))
                        .style(Style::default().fg(Color::DarkGray)),
                );
            }
        }
        Row::new(cells)
    });

    let widths: Vec<Constraint> = match kind {
        arena::Kind::Text => vec![
            Constraint::Length(4),
            Constraint::Min(28),
            Constraint::Length(14),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(14),
        ],
        arena::Kind::Image | arena::Kind::Video => vec![
            Constraint::Length(4),
            Constraint::Min(28),
            Constraint::Length(14),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(14),
        ],
    };

    let table = Table::new(rows, widths)
        .header(header.height(1))
        .row_highlight_style(Style::default().bg(Color::DarkGray).bold())
        .highlight_symbol("> ")
        .block(Block::default().borders(Borders::ALL).title("Leaderboard"));

    let st = app.table_state.entry(board).or_default();
    frame.render_stateful_widget(table, area, st);
}

fn render_footer(frame: &mut Frame, area: Rect, app: &AppState) {
    let mut line = vec![
        key("q"),
        text(" quit  "),
        key("r"),
        text(" refresh  "),
        key("[ ]"),
        text(" tab  "),
        key("1-9 0 -"),
        text(" jump  "),
    ];
    match app.current_board() {
        Board::Aa => {
            line.push(key("i s p c"));
            line.push(text(" sort intel/speed/price/context  "));
            line.push(key("v"));
            line.push(text(match app.aa_view {
                View::Table => " chart  ",
                View::Chart => " table  ",
            }));
        }
        Board::Arena(_) => {
            line.push(key("n i v p c"));
            line.push(text(" sort rank/rating/votes/price/ctx  "));
        }
    }
    line.extend([key("o"), text(" toggle dir  "), key("up/down"), text(" move")]);

    let p = Paragraph::new(Line::from(line))
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(Color::Gray));
    frame.render_widget(p, area);
}

fn key(s: &str) -> Span<'_> {
    Span::styled(
        s,
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
    )
}

fn text(s: &str) -> Span<'_> {
    Span::raw(s)
}

fn fmt_f(v: Option<f64>, decimals: usize) -> String {
    match v {
        Some(x) => format!("{x:.*}", decimals),
        None => "-".into(),
    }
}

fn fmt_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{}M", n / 1_000_000)
    } else if n >= 1_000 {
        format!("{}K", n / 1_000)
    } else {
        n.to_string()
    }
}

fn format_compact(n: u64) -> String {
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

fn aa_key_label(k: AaKey) -> &'static str {
    match k {
        AaKey::Intelligence => "Intelligence",
        AaKey::Speed => "Speed",
        AaKey::Price => "Price",
        AaKey::Context => "Context",
    }
}

fn arena_key_label(k: ArenaKey) -> &'static str {
    match k {
        ArenaKey::Rank => "Rank",
        ArenaKey::Rating => "Rating",
        ArenaKey::Votes => "Votes",
        ArenaKey::Price => "Price",
        ArenaKey::Context => "Context",
    }
}

fn score_color(v: Option<f64>, low: f64, high: f64) -> Style {
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

fn price_color(v: Option<f64>) -> Style {
    let Some(x) = v else {
        return Style::default().fg(Color::DarkGray);
    };
    let color = if x <= 1.0 {
        Color::Green
    } else if x <= 5.0 {
        Color::Yellow
    } else {
        Color::Red
    };
    Style::default().fg(color)
}
