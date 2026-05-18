use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::HashMap;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Cell, TableState},
    Frame,
};

mod aa_board;
mod agents;
mod arena_board;
mod chart;
mod chrome;

use crate::aa;
use crate::arena;
use crate::board::{Board, Data, Status};
use crate::coding_agents;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

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
    pub boards: &'static [Board],
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

    pub fn move_page_down(&mut self) {
        let board = self.current_board();
        let len = self.row_count(board);
        if len == 0 {
            return;
        }
        let page = 10;
        let st = self.table_state.entry(board).or_default();
        let i = st.selected().map(|i| i + page).unwrap_or(0).min(len - 1);
        st.select(Some(i));
    }

    pub fn move_page_up(&mut self) {
        let board = self.current_board();
        let st = self.table_state.entry(board).or_default();
        let i = st.selected().unwrap_or(0).saturating_sub(10);
        st.select(Some(i));
    }

    pub fn move_to_top(&mut self) {
        let board = self.current_board();
        if self.row_count(board) > 0 {
            let st = self.table_state.entry(board).or_default();
            st.select(Some(0));
        }
    }

    pub fn move_to_bottom(&mut self) {
        let board = self.current_board();
        let len = self.row_count(board);
        if len > 0 {
            let st = self.table_state.entry(board).or_default();
            st.select(Some(len - 1));
        }
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

    pub fn selected_name(&self) -> Option<String> {
        let board = self.current_board();
        let idx = selected_index(self, board);
        match self.status.get(&board) {
            Some(Status::Loaded(Data::Aa(models))) => {
                let query = self.filter_query(board);
                let filtered = filter_aa_models(models, query);
                filtered.get(idx).map(|m| m.name.clone())
            }
            Some(Status::Loaded(Data::AaAgents(rows))) => {
                let query = self.filter_query(board);
                let filtered = filter_agent_rows(rows, query);
                filtered.get(idx).map(|r| r.label())
            }
            Some(Status::Loaded(Data::Arena(entries))) => {
                let query = self.filter_query(board);
                let filtered = filter_arena_entries(entries, query);
                filtered.get(idx).map(|e| e.name.clone())
            }
            _ => None,
        }
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
    let arena_sort_key = app.arena_sort.key;
    let table_state = &mut app.table_state;

    match app.status.get(&board) {
        Some(Status::Loaded(Data::Aa(models))) => {
            let filtered = filter_aa_models(models, &query);
            if filtered.is_empty() && filter_is_active(&query) {
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
                        &filtered,
                        table_state,
                        board,
                        aa_sort_key,
                    );
                    if let Some(detail_area) = detail_area {
                        aa_board::render_aa_detail(frame, detail_area, filtered.get(selected));
                    }
                }
                View::Chart => aa_board::render_aa_chart(frame, area, &filtered, app),
            }
        }
        Some(Status::Loaded(Data::AaAgents(rows))) => {
            let filtered = filter_agent_rows(rows, &query);
            if filtered.is_empty() && filter_is_active(&query) {
                chrome::render_filter_empty(frame, area, &query);
                return;
            }
            let selected = table_state
                .get(&board)
                .and_then(TableState::selected)
                .unwrap_or(0);
            match view {
                View::Table => {
                    let (table_area, radar_area) = split_agents_table_body(area);
                    agents::render_agents_table(
                        frame,
                        table_area,
                        &filtered,
                        table_state,
                        board,
                        agent_sort_key,
                    );
                    if let Some(radar_area) = radar_area {
                        agents::render_agents_radar_panel(frame, radar_area, &filtered, selected);
                    }
                }
                View::Chart => agents::render_agents_chart(frame, area, &filtered, app),
            }
        }
        Some(Status::Loaded(Data::Arena(entries))) => {
            let filtered = filter_arena_entries(entries, &query);
            if filtered.is_empty() && filter_is_active(&query) {
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
                    let kind = match board {
                        Board::Arena(s) => s.kind(),
                        _ => arena::Kind::Text,
                    };
                    arena_board::render_arena_table(
                        frame,
                        table_area,
                        &filtered,
                        table_state,
                        board,
                        arena_sort_key,
                        kind,
                    );
                    if let Some(detail_area) = detail_area {
                        arena_board::render_arena_detail(
                            frame,
                            detail_area,
                            filtered.get(selected),
                            board,
                        );
                    }
                }
                View::Chart => arena_board::render_arena_chart(frame, area, &filtered, app, board),
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

fn split_agents_table_body(area: Rect) -> (Rect, Option<Rect>) {
    if area.width < 126 || area.height < 16 {
        return (area, None);
    }

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(74), Constraint::Length(46)])
        .split(area);
    (chunks[0], Some(chunks[1]))
}

fn push_unique<T: PartialEq>(items: &mut Vec<T>, item: T) {
    if !items.contains(&item) {
        items.push(item);
    }
}

fn header_cell(label: &str) -> Cell<'_> {
    Cell::from(label).style(
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )
}

fn selected_row_style() -> Style {
    Style::default().add_modifier(Modifier::BOLD)
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

fn detail_line(label: &str, value: impl Into<String>, style: Style) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:<10}"), Style::default().fg(Color::DarkGray)),
        Span::styled(value.into(), style),
    ])
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

fn filter_aa_models<'a>(models: &'a [aa::Model], query: &str) -> Cow<'a, [aa::Model]> {
    let tokens = filter_tokens(query);
    if tokens.is_empty() {
        return Cow::Borrowed(models);
    }
    Cow::Owned(
        models
            .iter()
            .filter(|model| aa_matches_filter(model, &tokens))
            .cloned()
            .collect(),
    )
}

fn filter_agent_rows<'a>(
    rows: &'a [coding_agents::AgentRow],
    query: &str,
) -> Cow<'a, [coding_agents::AgentRow]> {
    let tokens = filter_tokens(query);
    if tokens.is_empty() {
        return Cow::Borrowed(rows);
    }
    Cow::Owned(
        rows.iter()
            .filter(|row| agent_matches_filter(row, &tokens))
            .cloned()
            .collect(),
    )
}

fn filter_arena_entries<'a>(entries: &'a [arena::Entry], query: &str) -> Cow<'a, [arena::Entry]> {
    let tokens = filter_tokens(query);
    if tokens.is_empty() {
        return Cow::Borrowed(entries);
    }
    Cow::Owned(
        entries
            .iter()
            .filter(|entry| arena_matches_filter(entry, &tokens))
            .cloned()
            .collect(),
    )
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

fn fmt_f(v: Option<f64>, decimals: usize) -> String {
    match v {
        Some(x) => format!("{x:.*}", decimals),
        None => "-".into(),
    }
}

fn fmt_price(v: Option<f64>, decimals: usize, suffix: &str) -> String {
    match v {
        Some(x) => format!("${x:.*}{suffix}", decimals),
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

#[cfg(test)]
mod tests {
    use super::chart::{chart_bar_value, ChartPreference};
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
        assert_eq!(Board::shortcut(11), Some('='));
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
        let cols = aa_board::aa_columns(48, AaKey::Speed);

        assert!(cols.contains(&aa_board::AaColumn::Speed));
        assert!(cols.contains(&aa_board::AaColumn::Model));
        assert!(cols.contains(&aa_board::AaColumn::Intelligence));
        assert!(cols.contains(&aa_board::AaColumn::Price));
        assert!(!cols.contains(&aa_board::AaColumn::Released));
    }

    #[test]
    fn responsive_arena_columns_keep_context_sort_visible() {
        let cols = arena_board::arena_columns(60, arena::Kind::Text, ArenaKey::Context);

        assert!(cols.contains(&arena_board::ArenaColumn::Context));
        assert!(cols.contains(&arena_board::ArenaColumn::InputPrice));
        assert!(cols.contains(&arena_board::ArenaColumn::OutputPrice));
        assert!(!cols.contains(&arena_board::ArenaColumn::License));
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

    #[test]
    fn render_draws_agents_table_radar_panel() {
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
        assert!(text.contains("Radar"));
        assert!(text.contains("Legend"));
        assert!(text.contains("Top visible"));
        assert!(text.contains("Claude Code"));
        assert!(text.contains("Codex"));
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

    fn make_models(n: usize) -> Vec<aa::Model> {
        (0..n)
            .map(|i| aa_model(&format!("m{i}"), &format!("Model {i}"), "Provider"))
            .collect()
    }

    #[test]
    fn page_down_moves_by_ten_rows() {
        let mut app = AppState::new();
        app.set_status(Board::Aa, Status::Loaded(Data::Aa(make_models(30))));
        assert_eq!(selected_index(&app, Board::Aa), 0);

        app.move_page_down();
        assert_eq!(selected_index(&app, Board::Aa), 10);

        app.move_page_down();
        assert_eq!(selected_index(&app, Board::Aa), 20);

        app.move_page_down();
        assert_eq!(selected_index(&app, Board::Aa), 29);
    }

    #[test]
    fn page_up_stops_at_zero() {
        let mut app = AppState::new();
        app.set_status(Board::Aa, Status::Loaded(Data::Aa(make_models(30))));
        app.move_to_bottom();
        assert_eq!(selected_index(&app, Board::Aa), 29);

        app.move_page_up();
        assert_eq!(selected_index(&app, Board::Aa), 19);

        app.move_page_up();
        assert_eq!(selected_index(&app, Board::Aa), 9);

        app.move_page_up();
        assert_eq!(selected_index(&app, Board::Aa), 0);

        // Already at top, stays at 0
        app.move_page_up();
        assert_eq!(selected_index(&app, Board::Aa), 0);
    }

    #[test]
    fn move_to_top_and_bottom() {
        let mut app = AppState::new();
        app.set_status(Board::Aa, Status::Loaded(Data::Aa(make_models(20))));

        app.move_to_bottom();
        assert_eq!(selected_index(&app, Board::Aa), 19);

        app.move_to_top();
        assert_eq!(selected_index(&app, Board::Aa), 0);
    }

    #[test]
    fn move_to_bottom_on_empty_list() {
        let mut app = AppState::new();
        app.set_status(Board::Aa, Status::Loaded(Data::Aa(vec![])));

        app.move_to_bottom();
        // Should not panic, selection stays None
        assert_eq!(
            app.table_state.get(&Board::Aa).and_then(|s| s.selected()),
            None
        );
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
        app.set_status(Board::Aa, Status::Loaded(Data::Aa(make_models(5))));
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24)).unwrap();
        terminal.draw(|frame| render(frame, &mut app)).unwrap();
        let text = rendered_text(terminal.backend());

        assert!(text.contains("1/5"));
    }
}
