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
use crate::coding_agents;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentKey {
    Index,
    Pass,
    Cost,
    Time,
    Tokens,
    Turns,
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

#[derive(Debug, Clone)]
pub struct AgentSort {
    pub key: AgentKey,
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
    pub views: HashMap<Board, View>,
    pub filters: HashMap<Board, String>,
    pub filter_editing: bool,
    pub help_open: bool,
    pub aa_sort: AaSort,
    pub arena_sort: ArenaSort,
    pub agent_sort: AgentSort,
}

impl AppState {
    pub fn new() -> Self {
        let boards = Board::all();
        Self {
            boards,
            current: 0,
            status: HashMap::new(),
            table_state: HashMap::new(),
            views: HashMap::new(),
            filters: HashMap::new(),
            filter_editing: false,
            help_open: false,
            aa_sort: AaSort {
                key: AaKey::Intelligence,
                dir: SortDir::Desc,
            },
            arena_sort: ArenaSort {
                key: ArenaKey::Rank,
                dir: SortDir::Asc,
            },
            agent_sort: AgentSort {
                key: AgentKey::Index,
                dir: SortDir::Desc,
            },
        }
    }

    pub fn current_view(&self) -> View {
        self.views
            .get(&self.current_board())
            .copied()
            .unwrap_or(View::Table)
    }

    pub fn toggle_help(&mut self) {
        self.help_open = !self.help_open;
    }

    pub fn close_help(&mut self) {
        self.help_open = false;
    }

    pub fn is_help_open(&self) -> bool {
        self.help_open
    }

    pub fn toggle_view(&mut self) {
        let board = self.current_board();
        let next = match self.current_view() {
            View::Table => View::Chart,
            View::Chart => View::Table,
        };
        if next == View::Table {
            self.views.remove(&board);
        } else {
            self.views.insert(board, next);
        }
    }

    pub fn current_board(&self) -> Board {
        self.boards[self.current]
    }

    pub fn set_status(&mut self, board: Board, status: Status) {
        if let Status::Loaded(data) = &status {
            let mut data = data.clone();
            self.sort_data(&mut data);
            self.status.insert(board, Status::Loaded(data));
            self.reset_selection(board);
        } else {
            self.status.insert(board, status);
        }
    }

    pub fn select_board(&mut self, idx: usize) {
        if idx >= self.boards.len() {
            return;
        }
        self.current = idx;
        self.clamp_selection(self.current_board());
    }

    pub fn cycle_board(&mut self, delta: i32) {
        let n = self.boards.len() as i32;
        let next = ((self.current as i32 + delta).rem_euclid(n)) as usize;
        self.current = next;
        self.clamp_selection(self.current_board());
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
        let query = self.filter_query(board);
        match self.status.get(&board) {
            Some(Status::Loaded(Data::Aa(v))) => count_matching_aa(v, query),
            Some(Status::Loaded(Data::AaAgents(v))) => count_matching_agents(v, query),
            Some(Status::Loaded(Data::Arena(v))) => count_matching_arena(v, query),
            _ => 0,
        }
    }

    pub fn begin_filter(&mut self) {
        self.filter_editing = true;
        let board = self.current_board();
        self.filters.entry(board).or_default();
    }

    pub fn finish_filter(&mut self) {
        self.filter_editing = false;
        self.drop_empty_current_filter();
    }

    pub fn is_filter_editing(&self) -> bool {
        self.filter_editing
    }

    pub fn filter_query(&self, board: Board) -> &str {
        self.filters.get(&board).map(String::as_str).unwrap_or("")
    }

    pub fn current_filter(&self) -> &str {
        self.filter_query(self.current_board())
    }

    pub fn push_filter_char(&mut self, c: char) {
        if c.is_control() {
            return;
        }
        let board = self.current_board();
        self.filters.entry(board).or_default().push(c);
        self.reset_selection(board);
    }

    pub fn pop_filter_char(&mut self) {
        let board = self.current_board();
        if let Some(query) = self.filters.get_mut(&board) {
            query.pop();
        }
        self.drop_empty_filter(board);
        self.reset_selection(board);
    }

    pub fn clear_current_filter(&mut self) {
        let board = self.current_board();
        self.filters.remove(&board);
        self.reset_selection(board);
    }

    fn drop_empty_current_filter(&mut self) {
        let board = self.current_board();
        self.drop_empty_filter(board);
    }

    fn drop_empty_filter(&mut self, board: Board) {
        if self
            .filters
            .get(&board)
            .is_some_and(|query| query.is_empty())
        {
            self.filters.remove(&board);
        }
    }

    fn reset_selection(&mut self, board: Board) {
        let len = self.row_count(board);
        let st = self.table_state.entry(board).or_default();
        st.select(if len > 0 { Some(0) } else { None });
    }

    fn clamp_selection(&mut self, board: Board) {
        let len = self.row_count(board);
        let st = self.table_state.entry(board).or_default();
        if len == 0 {
            st.select(None);
            return;
        }
        let idx = st.selected().unwrap_or(0).min(len - 1);
        st.select(Some(idx));
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
            Board::AaAgents => {
                let new_key = match key {
                    'i' => Some(AgentKey::Index),
                    'a' => Some(AgentKey::Pass),
                    'p' => Some(AgentKey::Cost),
                    't' => Some(AgentKey::Time),
                    'u' => Some(AgentKey::Tokens),
                    's' => Some(AgentKey::Turns),
                    _ => None,
                };
                if let Some(k) = new_key {
                    if self.agent_sort.key == k {
                        self.agent_sort.dir = self.agent_sort.dir.toggle();
                    } else {
                        self.agent_sort.key = k;
                        self.agent_sort.dir = if matches!(
                            k,
                            AgentKey::Cost | AgentKey::Time | AgentKey::Tokens | AgentKey::Turns
                        ) {
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
            Board::AaAgents => self.agent_sort.dir = self.agent_sort.dir.toggle(),
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
            let agents = self.agent_sort.clone();
            sort_with(&mut taken, &aa, &arena, &agents);
            *data = taken;
        }
        let has_rows = self.row_count(board) > 0;
        if let Some(st) = self.table_state.get_mut(&board) {
            st.select(if has_rows { Some(0) } else { None });
        }
    }

    fn sort_data(&self, data: &mut Data) {
        sort_with(data, &self.aa_sort, &self.arena_sort, &self.agent_sort);
    }
}

fn sort_with(data: &mut Data, aa_sort: &AaSort, arena_sort: &ArenaSort, agent_sort: &AgentSort) {
    match data {
        Data::Aa(models) => sort_aa(models, aa_sort),
        Data::AaAgents(rows) => sort_agents(rows, agent_sort),
        Data::Arena(entries) => sort_arena(entries, arena_sort),
    }
}

fn sort_aa(models: &mut [aa::Model], sort: &AaSort) {
    let key = sort.key;
    let dir = sort.dir;
    models.sort_by(|a, b| {
        let av = aa_metric(a, key);
        let bv = aa_metric(b, key);
        cmp_opt_for_dir(av, bv, dir)
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

fn sort_agents(rows: &mut [coding_agents::AgentRow], sort: &AgentSort) {
    let key = sort.key;
    let dir = sort.dir;
    rows.sort_by(|a, b| {
        let av = agent_metric(a, key);
        let bv = agent_metric(b, key);
        cmp_opt_for_dir(av, bv, dir)
    });
}

fn agent_metric(row: &coding_agents::AgentRow, key: AgentKey) -> Option<f64> {
    match key {
        AgentKey::Index => row.index_score,
        AgentKey::Pass => row.mean.reward,
        AgentKey::Cost => row.mean.cost_usd,
        AgentKey::Time => row.mean.agent_wall_time_sec,
        AgentKey::Tokens => row.mean.total_tokens,
        AgentKey::Turns => row.mean.steps,
    }
}

fn sort_arena(entries: &mut [arena::Entry], sort: &ArenaSort) {
    let key = sort.key;
    let dir = sort.dir;
    entries.sort_by(|a, b| {
        let av = arena_metric(a, key);
        let bv = arena_metric(b, key);
        cmp_opt_for_dir(av, bv, dir)
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

fn cmp_opt_for_dir(a: Option<f64>, b: Option<f64>, dir: SortDir) -> Ordering {
    match (a, b) {
        (Some(x), Some(y)) => {
            let ord = x.partial_cmp(&y).unwrap_or(Ordering::Equal);
            match dir {
                SortDir::Desc => ord.reverse(),
                SortDir::Asc => ord,
            }
        }
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

pub fn render(frame: &mut Frame, app: &mut AppState) {
    if frame.area().width < 36 || frame.area().height < 8 {
        render_tiny(frame);
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

    render_tabs(frame, chunks[0], app);
    render_header(frame, chunks[1], app);
    render_body(frame, chunks[2], app);
    render_footer(frame, chunks[3], app);

    if app.is_help_open() {
        render_help_overlay(frame, frame.area());
    }
}

fn render_help_overlay(frame: &mut Frame, area: Rect) {
    let popup = centered_rect(area, 82, 42);
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
        help_row(
            "Arena",
            "arena.ai community-vote leaderboards (10 modalities)",
        ),
        Line::from(""),
        Line::styled("Navigation", head),
        help_row("q  Esc  Ctrl-C", "Quit llmpk"),
        help_row("?  h", "Toggle this help overlay"),
        help_row("[   ]", "Previous / next board (cycles)"),
        help_row("1-9  0  -  =", "Jump directly to board 1-12"),
        help_row("r", "Reload the current board (refetch from source)"),
        help_row("↑  ↓  k  j", "Move the highlighted row"),
        Line::from(""),
        Line::styled("View & sort", head),
        help_row("m", "Toggle table / chart view"),
        help_row("o", "Reverse current sort direction (asc <-> desc)"),
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
        Line::styled("  Arena sort keys", dim),
        help_row("n", "Rank — leaderboard position (1 = best)"),
        help_row("i", "Rating — ELO-style score from head-to-head votes"),
        help_row("v", "Votes — number of human comparisons collected"),
        help_row(
            "p",
            "Price — $/M tokens, $/image, or $/sec (board-dependent)",
        ),
        help_row("c", "Context Window — max tokens the model accepts"),
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

fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    Rect::new(x, y, w, h)
}

fn render_tiny(frame: &mut Frame) {
    let p = Paragraph::new("llmpk needs a larger terminal")
        .style(Style::default().fg(Color::Yellow))
        .block(Block::default().borders(Borders::ALL).title("llmpk"));
    frame.render_widget(p, frame.area());
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
        Some(Status::Loading) => Span::styled("*", Style::default().fg(Color::Yellow)),
        Some(Status::Error(_)) => Span::styled("!", Style::default().fg(Color::Red)),
        _ => Span::raw(""),
    }
}

fn tab_label(board: Board) -> String {
    match board {
        Board::Aa => "AA".to_string(),
        Board::AaAgents => "AAg".to_string(),
        Board::Arena(arena::Slug::Text) => "Txt".to_string(),
        Board::Arena(arena::Slug::Search) => "Srch".to_string(),
        Board::Arena(arena::Slug::Vision) => "Vis".to_string(),
        Board::Arena(arena::Slug::Document) => "Doc".to_string(),
        Board::Arena(arena::Slug::Code) => "Code".to_string(),
        Board::Arena(arena::Slug::TextToImage) => "T2I".to_string(),
        Board::Arena(arena::Slug::ImageEdit) => "Edit".to_string(),
        Board::Arena(arena::Slug::TextToVideo) => "T2V".to_string(),
        Board::Arena(arena::Slug::ImageToVideo) => "I2V".to_string(),
        Board::Arena(arena::Slug::VideoEdit) => "VEd".to_string(),
    }
}

fn render_header(frame: &mut Frame, area: Rect, app: &AppState) {
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
        Some(Status::Loaded(Data::Arena(v))) => {
            let visible = app.row_count(board);
            let label = if filter_is_active(query) {
                format!("{visible}/{} entries", v.len())
            } else {
                format!("{} entries", v.len())
            };
            Span::styled(label, Style::default().fg(Color::Green))
        }
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
        Board::AaAgents => format!(
            "sort: {} {}",
            agent_key_label(app.agent_sort.key),
            app.agent_sort.dir.arrow()
        ),
        Board::Arena(_) => format!(
            "sort: {} {}",
            arena_key_label(app.arena_sort.key),
            app.arena_sort.dir.arrow()
        ),
    };
    let source = match board {
        Board::Aa => "artificialanalysis.ai".to_string(),
        Board::AaAgents => "artificialanalysis.ai/agents/coding-agents".to_string(),
        Board::Arena(s) => format!("arena.ai/leaderboard/{}", s.path()),
    };

    let mut spans = vec![
        Span::styled(source, Style::default().add_modifier(Modifier::BOLD)),
        Span::raw("  |  "),
        status_span,
        Span::raw("  |  "),
        Span::styled(
            view_label(app.current_view()),
            Style::default().fg(Color::Blue),
        ),
        Span::raw("  |  "),
        Span::styled(sort_label, Style::default().fg(Color::Cyan)),
    ];
    if app.is_filter_editing() || filter_is_active(query) {
        spans.push(Span::raw("  |  "));
        spans.push(Span::styled(
            format!(
                "filter: {}",
                format_filter_label(query, app.is_filter_editing())
            ),
            Style::default().fg(Color::Yellow),
        ));
    }

    let line = Line::from(spans);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!("llmpk - {}", board.label()));
    frame.render_widget(Paragraph::new(line).block(block), area);
}

fn render_body(frame: &mut Frame, area: Rect, app: &mut AppState) {
    let board = app.current_board();
    let query = app.current_filter().to_string();
    match app.status.get(&board).cloned() {
        Some(Status::Loaded(Data::Aa(models))) => {
            let models = filter_aa_models(&models, &query);
            if models.is_empty() && filter_is_active(&query) {
                render_filter_empty(frame, area, &query);
                return;
            }
            match app.current_view() {
                View::Table => {
                    let selected = selected_index(app, board);
                    let (table_area, detail_area) = split_body(area);
                    render_aa_table(frame, table_area, &models, app, board);
                    if let Some(detail_area) = detail_area {
                        render_aa_detail(frame, detail_area, models.get(selected));
                    }
                }
                View::Chart => render_aa_chart(frame, area, &models, app),
            }
        }
        Some(Status::Loaded(Data::AaAgents(rows))) => {
            let rows = filter_agent_rows(&rows, &query);
            if rows.is_empty() && filter_is_active(&query) {
                render_filter_empty(frame, area, &query);
                return;
            }
            match app.current_view() {
                View::Table => {
                    let selected = selected_index(app, board);
                    let (table_area, detail_area) = split_body(area);
                    render_agents_table(frame, table_area, &rows, app, board);
                    if let Some(detail_area) = detail_area {
                        render_agent_detail(frame, detail_area, rows.get(selected));
                    }
                }
                View::Chart => render_agents_chart(frame, area, &rows, app),
            }
        }
        Some(Status::Loaded(Data::Arena(entries))) => {
            let entries = filter_arena_entries(&entries, &query);
            if entries.is_empty() && filter_is_active(&query) {
                render_filter_empty(frame, area, &query);
                return;
            }
            match app.current_view() {
                View::Table => {
                    let selected = selected_index(app, board);
                    let (table_area, detail_area) = split_body(area);
                    render_arena_table(frame, table_area, &entries, app, board);
                    if let Some(detail_area) = detail_area {
                        render_arena_detail(frame, detail_area, entries.get(selected), board);
                    }
                }
                View::Chart => render_arena_chart(frame, area, &entries, app, board),
            }
        }
        Some(Status::Error(e)) => {
            let p = Paragraph::new(format!("error: {e}\n\npress r to retry"))
                .style(Style::default().fg(Color::Red))
                .block(Block::default().borders(Borders::ALL).title("Leaderboard"))
                .wrap(Wrap { trim: true });
            frame.render_widget(p, area);
        }
        Some(Status::Loading) | None => {
            let p = Paragraph::new("loading leaderboard data...")
                .style(Style::default().fg(Color::Yellow))
                .block(Block::default().borders(Borders::ALL).title("Leaderboard"));
            frame.render_widget(p, area);
        }
    }
}

fn render_filter_empty(frame: &mut Frame, area: Rect, query: &str) {
    let p = Paragraph::new(format!(
        "no rows match filter: {}\n\npress / to edit or Ctrl-U to clear",
        truncate(query, 48)
    ))
    .style(Style::default().fg(Color::Yellow))
    .block(Block::default().borders(Borders::ALL).title("Leaderboard"))
    .wrap(Wrap { trim: true });
    frame.render_widget(p, area);
}

fn selected_index(app: &AppState, board: Board) -> usize {
    app.table_state
        .get(&board)
        .and_then(TableState::selected)
        .unwrap_or(0)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AaColumn {
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

fn aa_columns(width: u16, sort_key: AaKey) -> Vec<AaColumn> {
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

fn push_unique<T: PartialEq>(items: &mut Vec<T>, item: T) {
    if !items.contains(&item) {
        items.push(item);
    }
}

fn render_aa_table(
    frame: &mut Frame,
    area: Rect,
    models: &[aa::Model],
    app: &mut AppState,
    board: Board,
) {
    let columns = aa_columns(area.width, app.aa_sort.key);
    let header = Row::new(columns.iter().map(|column| header_cell(column.header())));

    let rows = models
        .iter()
        .enumerate()
        .map(|(i, m)| Row::new(columns.iter().map(|column| aa_cell(*column, i, m))));

    let widths: Vec<Constraint> = columns.iter().map(|column| column.width()).collect();
    let title = format!("AA models ({})", models.len());

    let table = Table::new(rows, widths)
        .header(header.height(1))
        .row_highlight_style(Style::default().bg(Color::DarkGray).fg(Color::White).bold())
        .highlight_symbol("> ")
        .block(Block::default().borders(Borders::ALL).title(title))
        .column_spacing(1);

    let st = app.table_state.entry(board).or_default();
    frame.render_stateful_widget(table, area, st);
}

fn header_cell(label: &str) -> Cell<'_> {
    Cell::from(label).style(
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )
}

fn aa_cell(column: AaColumn, index: usize, model: &aa::Model) -> Cell<'static> {
    match column {
        AaColumn::Index => {
            Cell::from(format!("{:>2}", index + 1)).style(Style::default().fg(Color::DarkGray))
        }
        AaColumn::Model => Cell::from(model.name.clone()).style(Style::default().bold()),
        AaColumn::Provider => {
            Cell::from(model.provider().to_string()).style(Style::default().fg(Color::Magenta))
        }
        AaColumn::Intelligence => Cell::from(fmt_f(model.intelligence_index, 1))
            .style(score_color(model.intelligence_index, 30.0, 60.0)),
        AaColumn::Speed => {
            Cell::from(fmt_f(model.speed(), 0)).style(Style::default().fg(Color::Blue))
        }
        AaColumn::Price => Cell::from(fmt_f(model.price_1m_blended_3_to_1, 2))
            .style(price_color(model.price_1m_blended_3_to_1)),
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

fn render_aa_detail(frame: &mut Frame, area: Rect, model: Option<&aa::Model>) {
    let max = area.width.saturating_sub(4) as usize;
    let lines = match model {
        Some(model) => vec![
            Line::styled(
                truncate(&model.name, max.max(8)),
                Style::default().fg(Color::Cyan).bold(),
            ),
            Line::from(""),
            detail_line(
                "Provider",
                model.provider(),
                Style::default().fg(Color::Magenta),
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
                price_color(model.price_1m_blended_3_to_1),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentColumn {
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

fn agent_columns(width: u16, sort_key: AgentKey) -> Vec<AgentColumn> {
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

fn render_agents_table(
    frame: &mut Frame,
    area: Rect,
    rows: &[coding_agents::AgentRow],
    app: &mut AppState,
    board: Board,
) {
    let columns = agent_columns(area.width, app.agent_sort.key);
    let header = Row::new(columns.iter().map(|column| header_cell(column.header())));

    let table_rows = rows
        .iter()
        .enumerate()
        .map(|(i, row)| Row::new(columns.iter().map(|column| agent_cell(*column, i, row))));

    let widths: Vec<Constraint> = columns.iter().map(|column| column.width()).collect();
    let title = format!("AA coding agents ({})", rows.len());

    let table = Table::new(table_rows, widths)
        .header(header.height(1))
        .row_highlight_style(Style::default().bg(Color::DarkGray).fg(Color::White).bold())
        .highlight_symbol("> ")
        .block(Block::default().borders(Borders::ALL).title(title))
        .column_spacing(1);

    let st = app.table_state.entry(board).or_default();
    frame.render_stateful_widget(table, area, st);
}

fn agent_cell(column: AgentColumn, index: usize, row: &coding_agents::AgentRow) -> Cell<'static> {
    match column {
        AgentColumn::Index => {
            Cell::from(format!("{:>2}", index + 1)).style(Style::default().fg(Color::DarkGray))
        }
        AgentColumn::Agent => Cell::from(row.agent().to_string()).style(Style::default().bold()),
        AgentColumn::Model => Cell::from(row.model().to_string()),
        AgentColumn::Provider => {
            Cell::from(row.provider().to_string()).style(Style::default().fg(Color::Magenta))
        }
        AgentColumn::Score => {
            Cell::from(fmt_pct(row.index_score, 1)).style(score_color_pct(row.index_score))
        }
        AgentColumn::Pass => {
            Cell::from(fmt_pct(row.mean.reward, 1)).style(score_color_pct(row.mean.reward))
        }
        AgentColumn::Cost => {
            Cell::from(fmt_price(row.mean.cost_usd, 2, "")).style(price_color(row.mean.cost_usd))
        }
        AgentColumn::Time => Cell::from(fmt_duration(row.mean.agent_wall_time_sec))
            .style(Style::default().fg(Color::Blue)),
        AgentColumn::Tokens => Cell::from(fmt_compact_f(row.mean.total_tokens)),
        AgentColumn::Turns => Cell::from(fmt_f(row.mean.steps, 1)),
        AgentColumn::Released => Cell::from(row.release_date.clone().unwrap_or_else(|| "-".into()))
            .style(Style::default().fg(Color::DarkGray)),
    }
}

fn render_agent_detail(frame: &mut Frame, area: Rect, row: Option<&coding_agents::AgentRow>) {
    let max = area.width.saturating_sub(4) as usize;
    let lines = match row {
        Some(row) => vec![
            Line::styled(
                truncate(&row.label(), max.max(8)),
                Style::default().fg(Color::Cyan).bold(),
            ),
            Line::from(""),
            detail_line("Agent", row.agent(), Style::default().fg(Color::Cyan)),
            detail_line("Model", row.model(), Style::default()),
            detail_line(
                "Provider",
                row.provider(),
                Style::default().fg(Color::Magenta),
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
                price_color(row.mean.cost_usd),
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

fn detail_line(label: &str, value: impl Into<String>, style: Style) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:<9}"), Style::default().fg(Color::DarkGray)),
        Span::styled(value.into(), style),
    ])
}

fn render_aa_chart(frame: &mut Frame, area: Rect, models: &[aa::Model], app: &AppState) {
    let key = app.aa_sort.key;
    let (chart_area, summary_area) = split_chart_body(area);
    let max_bars = chart_capacity(chart_area);
    let rows: Vec<ChartRow> = models
        .iter()
        .filter_map(|m| {
            let value = aa_metric(m, key)?;
            Some(ChartRow {
                name: m.name.clone(),
                meta: m.provider().to_string(),
                value,
                value_label: aa_chart_text_value(m, key),
                color_seed: m.id.clone(),
            })
        })
        .take(max_bars)
        .collect();

    let title = format!(
        "AA chart - {} {} (top {}/{})",
        aa_key_label(key),
        app.aa_sort.dir.arrow(),
        rows.len(),
        models.len(),
    );
    let empty = format!(
        "no data for {} - switch sort key (i/s/p/c) or press m for table",
        aa_key_label(key)
    );
    let preference = aa_chart_preference(key);
    render_metric_chart(frame, chart_area, &title, &empty, &rows, preference);

    if let Some(summary_area) = summary_area {
        render_chart_summary(
            frame,
            summary_area,
            &rows,
            models.len(),
            aa_key_label(key),
            preference,
        );
    }
}

fn render_agents_chart(
    frame: &mut Frame,
    area: Rect,
    rows: &[coding_agents::AgentRow],
    app: &AppState,
) {
    let key = app.agent_sort.key;
    let (chart_area, summary_area) = split_chart_body(area);
    let max_bars = chart_capacity(chart_area);
    let chart_rows: Vec<ChartRow> = rows
        .iter()
        .filter_map(|row| {
            let value = agent_metric(row, key)?;
            Some(ChartRow {
                name: row.label(),
                meta: row.provider().to_string(),
                value,
                value_label: agent_chart_text_value(row, key),
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
        rows.len(),
    );
    let empty = format!(
        "no data for {} - switch sort key (i/a/p/t/u/s) or press m for table",
        agent_key_label(key)
    );
    let preference = agent_chart_preference(key);
    render_metric_chart(frame, chart_area, &title, &empty, &chart_rows, preference);

    if let Some(summary_area) = summary_area {
        render_chart_summary(
            frame,
            summary_area,
            &chart_rows,
            rows.len(),
            agent_key_label(key),
            preference,
        );
    }
}

fn render_arena_chart(
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
            Some(ChartRow {
                name: entry.name.clone(),
                meta: meta.clone(),
                value,
                value_label: arena_chart_text_value(entry, key, kind),
                color_seed: format!("{}:{meta}", entry.name),
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

#[derive(Debug, Clone)]
struct ChartRow {
    name: String,
    meta: String,
    value: f64,
    value_label: String,
    color_seed: String,
}

#[derive(Debug, Clone, Copy)]
enum ChartPreference {
    Higher,
    Lower,
}

impl ChartPreference {
    fn hint(self) -> &'static str {
        match self {
            ChartPreference::Higher => "higher is better",
            ChartPreference::Lower => "lower is better",
        }
    }
}

fn split_chart_body(area: Rect) -> (Rect, Option<Rect>) {
    if area.width < 118 || area.height < 12 {
        return (area, None);
    }

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(72), Constraint::Length(34)])
        .split(area);
    (chunks[0], Some(chunks[1]))
}

fn chart_capacity(area: Rect) -> usize {
    (area.height as usize).saturating_sub(2).max(1)
}

fn render_metric_chart(
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
            let color = color_for_seed(&row.color_seed);
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

fn render_chart_summary(
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

fn chart_bounds(rows: &[ChartRow]) -> (f64, f64) {
    rows.iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), row| {
            (min.min(row.value), max.max(row.value))
        })
}

fn chart_bar_value(value: f64, min: f64, max: f64, preference: ChartPreference) -> u64 {
    if !min.is_finite() || !max.is_finite() || (max - min).abs() < f64::EPSILON {
        return 1000;
    }

    let normalized = match preference {
        ChartPreference::Higher => (value - min) / (max - min),
        ChartPreference::Lower => (max - value) / (max - min),
    };
    (normalized.clamp(0.0, 1.0) * 999.0).round() as u64 + 1
}

fn aa_chart_preference(key: AaKey) -> ChartPreference {
    match key {
        AaKey::Price => ChartPreference::Lower,
        AaKey::Intelligence | AaKey::Speed | AaKey::Context => ChartPreference::Higher,
    }
}

fn agent_chart_preference(key: AgentKey) -> ChartPreference {
    match key {
        AgentKey::Cost | AgentKey::Time | AgentKey::Tokens | AgentKey::Turns => {
            ChartPreference::Lower
        }
        AgentKey::Index | AgentKey::Pass => ChartPreference::Higher,
    }
}

fn arena_chart_preference(key: ArenaKey) -> ChartPreference {
    match key {
        ArenaKey::Rank | ArenaKey::Price => ChartPreference::Lower,
        ArenaKey::Rating | ArenaKey::Votes | ArenaKey::Context => ChartPreference::Higher,
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

fn format_chart_number(value: f64) -> String {
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

fn color_for_seed(seed: &str) -> Color {
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

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(n.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

fn filter_is_active(query: &str) -> bool {
    !query.trim().is_empty()
}

fn format_filter_label(query: &str, editing: bool) -> String {
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

fn count_matching_aa(models: &[aa::Model], query: &str) -> usize {
    let tokens = filter_tokens(query);
    if tokens.is_empty() {
        return models.len();
    }
    models
        .iter()
        .filter(|model| aa_matches_filter(model, &tokens))
        .count()
}

fn count_matching_agents(rows: &[coding_agents::AgentRow], query: &str) -> usize {
    let tokens = filter_tokens(query);
    if tokens.is_empty() {
        return rows.len();
    }
    rows.iter()
        .filter(|row| agent_matches_filter(row, &tokens))
        .count()
}

fn count_matching_arena(entries: &[arena::Entry], query: &str) -> usize {
    let tokens = filter_tokens(query);
    if tokens.is_empty() {
        return entries.len();
    }
    entries
        .iter()
        .filter(|entry| arena_matches_filter(entry, &tokens))
        .count()
}

fn filter_aa_models(models: &[aa::Model], query: &str) -> Vec<aa::Model> {
    let tokens = filter_tokens(query);
    if tokens.is_empty() {
        return models.to_vec();
    }
    models
        .iter()
        .filter(|model| aa_matches_filter(model, &tokens))
        .cloned()
        .collect()
}

fn filter_agent_rows(
    rows: &[coding_agents::AgentRow],
    query: &str,
) -> Vec<coding_agents::AgentRow> {
    let tokens = filter_tokens(query);
    if tokens.is_empty() {
        return rows.to_vec();
    }
    rows.iter()
        .filter(|row| agent_matches_filter(row, &tokens))
        .cloned()
        .collect()
}

fn filter_arena_entries(entries: &[arena::Entry], query: &str) -> Vec<arena::Entry> {
    let tokens = filter_tokens(query);
    if tokens.is_empty() {
        return entries.to_vec();
    }
    entries
        .iter()
        .filter(|entry| arena_matches_filter(entry, &tokens))
        .cloned()
        .collect()
}

fn filter_tokens(query: &str) -> Vec<String> {
    query.split_whitespace().map(str::to_lowercase).collect()
}

fn aa_matches_filter(model: &aa::Model, tokens: &[String]) -> bool {
    let weights = match model.is_open_weights {
        Some(true) => "open open-weights",
        Some(false) => "closed proprietary",
        None => "",
    };
    let haystack = format!(
        "{} {} {} {} {}",
        model.id,
        model.name,
        model.provider(),
        model.release_date.as_deref().unwrap_or(""),
        weights
    )
    .to_lowercase();
    tokens.iter().all(|token| haystack.contains(token))
}

fn agent_matches_filter(row: &coding_agents::AgentRow, tokens: &[String]) -> bool {
    let haystack = format!(
        "{} {} {} {} {} {} {} {} {}",
        row.id,
        row.agent_name,
        row.agent(),
        row.model(),
        row.provider(),
        row.display_label.as_deref().unwrap_or(""),
        row.model_name.as_deref().unwrap_or(""),
        row.host_model_slug.as_deref().unwrap_or(""),
        row.release_date.as_deref().unwrap_or("")
    )
    .to_lowercase();
    tokens.iter().all(|token| haystack.contains(token))
}

fn arena_matches_filter(entry: &arena::Entry, tokens: &[String]) -> bool {
    let haystack = format!(
        "{} {} {} {}",
        entry.rank.map(|rank| rank.to_string()).unwrap_or_default(),
        entry.name,
        entry.organization.as_deref().unwrap_or(""),
        entry.license.as_deref().unwrap_or("")
    )
    .to_lowercase();
    tokens.iter().all(|token| haystack.contains(token))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArenaColumn {
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

fn arena_columns(width: u16, kind: arena::Kind, sort_key: ArenaKey) -> Vec<ArenaColumn> {
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
    let columns = arena_columns(area.width, kind, app.arena_sort.key);

    let header = Row::new(columns.iter().map(|column| header_cell(column.header())));

    let rows = entries
        .iter()
        .map(|entry| Row::new(columns.iter().map(|column| arena_cell(*column, entry))));

    let widths: Vec<Constraint> = columns.iter().map(|column| column.width()).collect();
    let title = format!("Arena entries ({})", entries.len());

    let table = Table::new(rows, widths)
        .header(header.height(1))
        .row_highlight_style(Style::default().bg(Color::DarkGray).fg(Color::White).bold())
        .highlight_symbol("> ")
        .block(Block::default().borders(Borders::ALL).title(title))
        .column_spacing(1);

    let st = app.table_state.entry(board).or_default();
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

fn render_arena_detail(frame: &mut Frame, area: Rect, entry: Option<&arena::Entry>, board: Board) {
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

fn render_footer(frame: &mut Frame, area: Rect, app: &AppState) {
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

    let nav = vec![
        key("? / h"),
        text(" help & keybindings  "),
        key("q"),
        text(" quit  "),
        key("[ ]"),
        text(" board  "),
        key("↑/↓"),
        text(" move row  "),
        key("r"),
        text(" reload"),
    ];

    let sort_keys = match app.current_board() {
        Board::Aa => "i/s/p/c",
        Board::AaAgents => "i/a/p/t/u/s",
        Board::Arena(_) => "n/i/v/p/c",
    };
    let view_action = match app.current_view() {
        View::Table => " chart view  ",
        View::Chart => " table view  ",
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

fn fmt_f(v: Option<f64>, decimals: usize) -> String {
    match v {
        Some(x) => format!("{x:.*}", decimals),
        None => "-".into(),
    }
}

fn fmt_pct(v: Option<f64>, decimals: usize) -> String {
    match v {
        Some(x) => format!("{:.*}", decimals, x * 100.0),
        None => "-".into(),
    }
}

fn fmt_price(v: Option<f64>, decimals: usize, suffix: &str) -> String {
    match v {
        Some(x) => format!("${x:.*}{suffix}", decimals),
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

fn fmt_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{}M", n / 1_000_000)
    } else if n >= 1_000 {
        format!("{}K", n / 1_000)
    } else {
        n.to_string()
    }
}

fn fmt_compact_f(v: Option<f64>) -> String {
    match v {
        Some(x) if x.is_finite() && x >= 0.0 => format_compact(x.round() as u64),
        Some(_) | None => "-".into(),
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

fn agent_key_label(k: AgentKey) -> &'static str {
    match k {
        AgentKey::Index => "Index",
        AgentKey::Pass => "Pass@1",
        AgentKey::Cost => "Cost",
        AgentKey::Time => "Time",
        AgentKey::Tokens => "Tokens",
        AgentKey::Turns => "Turns",
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

fn view_label(view: View) -> &'static str {
    match view {
        View::Table => "view: table",
        View::Chart => "view: chart",
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

fn score_color_pct(v: Option<f64>) -> Style {
    score_color(v.map(|x| x * 100.0), 30.0, 60.0)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_board_filter_counts_aa_rows_and_can_clear() {
        let mut app = AppState::new();
        app.set_status(
            Board::Aa,
            Status::Loaded(Data::Aa(vec![
                aa_model("claude-sonnet", "Claude Sonnet", "Anthropic"),
                aa_model("gpt-5", "GPT-5", "OpenAI"),
            ])),
        );

        assert_eq!(app.row_count(Board::Aa), 2);

        app.begin_filter();
        for c in "anthropic".chars() {
            app.push_filter_char(c);
        }

        assert_eq!(app.row_count(Board::Aa), 1);
        assert_eq!(
            app.table_state
                .get(&Board::Aa)
                .and_then(|state| state.selected()),
            Some(0)
        );

        app.clear_current_filter();

        assert_eq!(app.row_count(Board::Aa), 2);
    }

    #[test]
    fn filters_are_scoped_to_the_current_board() {
        let mut app = AppState::new();
        let arena_board = Board::Arena(arena::Slug::Text);
        app.set_status(
            Board::Aa,
            Status::Loaded(Data::Aa(vec![aa_model(
                "claude-sonnet",
                "Claude Sonnet",
                "Anthropic",
            )])),
        );
        app.set_status(
            arena_board,
            Status::Loaded(Data::Arena(vec![
                arena_entry("Claude Sonnet", "Anthropic", "Proprietary"),
                arena_entry("Llama", "Meta", "llama"),
            ])),
        );

        app.select_board(2);
        app.begin_filter();
        for c in "meta".chars() {
            app.push_filter_char(c);
        }

        assert_eq!(app.row_count(arena_board), 1);
        assert_eq!(app.row_count(Board::Aa), 1);
        assert_eq!(app.filter_query(Board::Aa), "");
        assert_eq!(app.filter_query(arena_board), "meta");
    }

    #[test]
    fn chart_view_is_scoped_to_the_current_board() {
        let mut app = AppState::new();
        let arena_board = Board::Arena(arena::Slug::Text);

        assert_eq!(app.current_view(), View::Table);
        app.toggle_view();
        assert_eq!(app.current_view(), View::Chart);

        app.select_board(2);
        assert_eq!(app.current_board(), arena_board);
        assert_eq!(app.current_view(), View::Table);

        app.toggle_view();
        assert_eq!(app.current_view(), View::Chart);

        app.select_board(0);
        assert_eq!(app.current_view(), View::Chart);
    }

    #[test]
    fn twelfth_board_uses_equals_shortcut() {
        let app = AppState::new();

        assert_eq!(app.boards.len(), 12);
        assert_eq!(app.boards[1], Board::AaAgents);
        assert_eq!(app.boards[11].shortcut(11), Some('='));
    }

    #[test]
    fn descending_sorts_keep_missing_metrics_last() {
        let mut app = AppState::new();
        let mut missing = aa_model("missing", "Missing Intel", "Unknown");
        missing.intelligence_index = None;
        app.set_status(
            Board::Aa,
            Status::Loaded(Data::Aa(vec![
                missing,
                aa_model("scored", "Scored Intel", "OpenAI"),
            ])),
        );

        let Some(Status::Loaded(Data::Aa(models))) = app.status.get(&Board::Aa) else {
            panic!("AA data should be loaded");
        };
        assert_eq!(models[0].name, "Scored Intel");
        assert_eq!(models[1].name, "Missing Intel");
    }

    #[test]
    fn descending_agent_sorts_keep_missing_metrics_last() {
        let mut app = AppState::new();
        let mut missing = agent_row("missing", "Missing Agent", "Unknown");
        missing.index_score = None;
        app.set_status(
            Board::AaAgents,
            Status::Loaded(Data::AaAgents(vec![
                missing,
                agent_row("scored", "Scored Agent", "Known"),
            ])),
        );

        let Some(Status::Loaded(Data::AaAgents(rows))) = app.status.get(&Board::AaAgents) else {
            panic!("AA Agents data should be loaded");
        };
        assert_eq!(rows[0].agent(), "Scored Agent");
        assert_eq!(rows[1].agent(), "Missing Agent");
    }

    #[test]
    fn lower_is_better_chart_values_prefer_smaller_metrics() {
        let cheap = chart_bar_value(1.0, 1.0, 10.0, ChartPreference::Lower);
        let expensive = chart_bar_value(10.0, 1.0, 10.0, ChartPreference::Lower);

        assert!(cheap > expensive);
    }

    #[test]
    fn responsive_aa_columns_keep_active_sort_metric_visible() {
        let cols = aa_columns(48, AaKey::Speed);

        assert!(cols.contains(&AaColumn::Speed));
        assert!(cols.contains(&AaColumn::Model));
        assert!(cols.contains(&AaColumn::Intelligence));
        assert!(cols.contains(&AaColumn::Price));
        assert!(!cols.contains(&AaColumn::Released));
    }

    #[test]
    fn responsive_arena_columns_keep_context_sort_visible() {
        let cols = arena_columns(60, arena::Kind::Text, ArenaKey::Context);

        assert!(cols.contains(&ArenaColumn::Context));
        assert!(cols.contains(&ArenaColumn::InputPrice));
        assert!(cols.contains(&ArenaColumn::OutputPrice));
        assert!(!cols.contains(&ArenaColumn::License));
    }

    #[test]
    fn responsive_agent_columns_keep_active_sort_metric_visible() {
        let cols = agent_columns(48, AgentKey::Time);

        assert!(cols.contains(&AgentColumn::Time));
        assert!(cols.contains(&AgentColumn::Agent));
        assert!(cols.contains(&AgentColumn::Score));
        assert!(!cols.contains(&AgentColumn::Released));
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

        assert!(text.contains("AA models"));
        assert!(text.contains("Selected"));
        assert!(text.contains("Claude Sonnet"));
    }

    #[test]
    fn render_draws_arena_chart_view() {
        let mut app = AppState::new();
        let board = Board::Arena(arena::Slug::Text);
        app.select_board(2);
        app.set_status(
            board,
            Status::Loaded(Data::Arena(vec![
                arena_entry("Claude Sonnet", "Anthropic", "Proprietary"),
                arena_entry("GPT", "OpenAI", "Proprietary"),
            ])),
        );
        app.toggle_view();

        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(132, 32)).unwrap();
        terminal.draw(|frame| render(frame, &mut app)).unwrap();
        let text = rendered_text(terminal.backend());

        assert!(text.contains("Arena Text chart"));
        assert!(text.contains("Chart stats"));
        assert!(text.contains("view: chart"));
        assert!(text.contains("Claude Sonnet"));
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
        assert!(text.contains("Chart stats"));
        assert!(text.contains("view: chart"));
        assert!(text.contains("Claude Code"));
    }

    fn rendered_text(backend: &ratatui::backend::TestBackend) -> String {
        backend
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    fn aa_model(id: &str, name: &str, provider: &str) -> aa::Model {
        aa::Model {
            id: id.to_string(),
            name: name.to_string(),
            model_creators: Some(aa::Creator {
                name: provider.to_string(),
            }),
            intelligence_index: Some(50.0),
            timescale: Some(aa::Timescale {
                median_output_speed: Some(100.0),
            }),
            price_1m_blended_3_to_1: Some(1.0),
            context_window_tokens: Some(200_000),
            release_date: Some("2026-01-01".to_string()),
            is_open_weights: Some(false),
        }
    }

    fn arena_entry(name: &str, organization: &str, license: &str) -> arena::Entry {
        arena::Entry {
            rank: Some(1),
            name: name.to_string(),
            rating: Some(1200.0),
            votes: Some(10_000),
            organization: Some(organization.to_string()),
            license: Some(license.to_string()),
            input_price: Some(1.0),
            output_price: Some(2.0),
            context_length: Some(128_000),
            price_per_image: None,
            price_per_second: None,
        }
    }

    fn agent_row(id: &str, agent: &str, provider: &str) -> coding_agents::AgentRow {
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
}
