use std::collections::HashMap;
use std::time::Instant;

use ratatui::widgets::TableState;

use crate::board::{Board, Data, Status};

use super::filter::*;
use super::sort::*;

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
    pub compact: bool,
    pub aa_sort: AaSort,
    pub agent_sort: AgentSort,
    pub deepswe_sort: DeepSweSort,
    filter_caches: HashMap<Board, FilterCache>,
    pub copy_feedback_at: Option<Instant>,
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
            compact: false,
            aa_sort: AaSort {
                key: AaKey::Intelligence,
                dir: SortDir::Desc,
            },
            agent_sort: AgentSort {
                key: AgentKey::Index,
                dir: SortDir::Desc,
            },
            deepswe_sort: DeepSweSort {
                key: DeepSweKey::Pass,
                dir: SortDir::Desc,
            },
            filter_caches: HashMap::new(),
            copy_feedback_at: None,
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
        match status {
            Status::Loaded(mut data) => {
                self.sort_data(&mut data);
                self.status.insert(board, Status::Loaded(data));
                self.reset_selection(board);
            }
            other => {
                self.status.insert(board, other);
            }
        }
        self.filter_caches.remove(&board);
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

    pub fn row_count(&self, board: Board) -> usize {
        let query = self.filter_query(board);
        match self.status.get(&board) {
            Some(Status::Loaded(Data::Aa(v))) => count_matching_aa(v, query),
            Some(Status::Loaded(Data::AaAgents(v))) => count_matching_agents(v, query),
            Some(Status::Loaded(Data::DeepSwe(v))) => count_matching_deepswe(v, query),
            _ => 0,
        }
    }

    pub fn selected_idx(&self) -> usize {
        self.table_state
            .get(&self.current_board())
            .and_then(TableState::selected)
            .unwrap_or(0)
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
        self.filter_caches.remove(&board);
        self.reset_selection(board);
    }

    pub fn pop_filter_char(&mut self) {
        let board = self.current_board();
        if let Some(query) = self.filters.get_mut(&board) {
            query.pop();
        }
        self.drop_empty_filter(board);
        self.filter_caches.remove(&board);
        self.reset_selection(board);
    }

    pub fn clear_current_filter(&mut self) {
        let board = self.current_board();
        self.filters.remove(&board);
        self.filter_caches.remove(&board);
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
            Board::DeepSwe => {
                let new_key = match key {
                    'a' => Some(DeepSweKey::Pass),
                    'p' => Some(DeepSweKey::Cost),
                    't' => Some(DeepSweKey::Time),
                    'u' => Some(DeepSweKey::Tokens),
                    's' => Some(DeepSweKey::Steps),
                    _ => None,
                };
                if let Some(k) = new_key {
                    if self.deepswe_sort.key == k {
                        self.deepswe_sort.dir = self.deepswe_sort.dir.toggle();
                    } else {
                        self.deepswe_sort.key = k;
                        self.deepswe_sort.dir = if matches!(
                            k,
                            DeepSweKey::Cost | DeepSweKey::Time | DeepSweKey::Tokens
                        ) {
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

    pub fn selected_name(&mut self) -> Option<String> {
        let board = self.current_board();
        let idx = self.selected_idx();
        let query = self.filter_query(board).to_string();
        let indices = self.filter_cache(board, &query);
        let data_idx = *indices.get(idx)?;
        match self.status.get(&board) {
            Some(Status::Loaded(Data::Aa(models))) => models.get(data_idx).map(|m| m.name.clone()),
            Some(Status::Loaded(Data::AaAgents(rows))) => rows.get(data_idx).map(|r| r.label()),
            Some(Status::Loaded(Data::DeepSwe(rows))) => {
                rows.get(data_idx).map(|r| r.display_model())
            }
            _ => None,
        }
    }

    pub fn toggle_dir(&mut self) {
        match self.current_board() {
            Board::Aa => self.aa_sort.dir = self.aa_sort.dir.toggle(),
            Board::AaAgents => self.agent_sort.dir = self.agent_sort.dir.toggle(),
            Board::DeepSwe => self.deepswe_sort.dir = self.deepswe_sort.dir.toggle(),
        }
        self.resort_current();
    }

    fn resort_current(&mut self) {
        let board = self.current_board();
        if let Some(Status::Loaded(data)) = self.status.get_mut(&board) {
            sort_with(data, &self.aa_sort, &self.agent_sort, &self.deepswe_sort);
        }
        self.filter_caches.remove(&board);
        let has_rows = self.row_count(board) > 0;
        if let Some(st) = self.table_state.get_mut(&board) {
            st.select(if has_rows { Some(0) } else { None });
        }
    }

    fn sort_data(&self, data: &mut Data) {
        sort_with(data, &self.aa_sort, &self.agent_sort, &self.deepswe_sort);
    }

    /// Return cached filter indices for the given board. Rebuilds if query changed.
    pub fn filter_cache(&mut self, board: Board, query: &str) -> &[usize] {
        let needs_rebuild = self
            .filter_caches
            .get(&board)
            .is_none_or(|c| c.query != query);

        if needs_rebuild {
            let indices = match self.status.get(&board) {
                Some(Status::Loaded(Data::Aa(models))) => {
                    let tokens = filter_tokens(query);
                    if tokens.is_empty() {
                        (0..models.len()).collect()
                    } else {
                        models
                            .iter()
                            .enumerate()
                            .filter(|(_, m)| aa_matches_filter(m, &tokens))
                            .map(|(i, _)| i)
                            .collect()
                    }
                }
                Some(Status::Loaded(Data::AaAgents(rows))) => {
                    let tokens = filter_tokens(query);
                    if tokens.is_empty() {
                        (0..rows.len()).collect()
                    } else {
                        rows.iter()
                            .enumerate()
                            .filter(|(_, r)| agent_matches_filter(r, &tokens))
                            .map(|(i, _)| i)
                            .collect()
                    }
                }
                Some(Status::Loaded(Data::DeepSwe(rows))) => {
                    let tokens = filter_tokens(query);
                    if tokens.is_empty() {
                        (0..rows.len()).collect()
                    } else {
                        rows.iter()
                            .enumerate()
                            .filter(|(_, r)| deepswe_matches_filter(r, &tokens))
                            .map(|(i, _)| i)
                            .collect()
                    }
                }
                _ => Vec::new(),
            };
            self.filter_caches.insert(
                board,
                FilterCache {
                    indices,
                    query: query.to_string(),
                },
            );
        }

        &self.filter_caches[&board].indices
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::*;
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
        app.set_status(
            Board::Aa,
            Status::Loaded(Data::Aa(vec![aa_model(
                "claude-sonnet",
                "Claude Sonnet",
                "Anthropic",
            )])),
        );
        app.set_status(
            Board::AaAgents,
            Status::Loaded(Data::AaAgents(vec![
                agent_row("claude-code", "Claude Code", "Anthropic"),
                agent_row("codex", "Codex", "OpenAI"),
            ])),
        );

        app.select_board(1);
        app.begin_filter();
        for c in "openai".chars() {
            app.push_filter_char(c);
        }

        assert_eq!(app.row_count(Board::AaAgents), 1);
        assert_eq!(app.row_count(Board::Aa), 1);
        assert_eq!(app.filter_query(Board::Aa), "");
        assert_eq!(app.filter_query(Board::AaAgents), "openai");
    }

    #[test]
    fn chart_view_is_scoped_to_the_current_board() {
        let mut app = AppState::new();

        assert_eq!(app.current_view(), View::Table);
        app.toggle_view();
        assert_eq!(app.current_view(), View::Chart);

        app.select_board(1);
        assert_eq!(app.current_board(), Board::AaAgents);
        assert_eq!(app.current_view(), View::Table);

        app.toggle_view();
        assert_eq!(app.current_view(), View::Chart);

        app.select_board(0);
        assert_eq!(app.current_view(), View::Chart);
    }

    #[test]
    fn page_down_moves_by_ten_rows() {
        let mut app = AppState::new();
        app.set_status(Board::Aa, Status::Loaded(Data::Aa(make_models(30))));
        assert_eq!(app.selected_idx(), 0);

        app.move_page_down();
        assert_eq!(app.selected_idx(), 10);

        app.move_page_down();
        assert_eq!(app.selected_idx(), 20);

        app.move_page_down();
        assert_eq!(app.selected_idx(), 29);
    }

    #[test]
    fn page_up_stops_at_zero() {
        let mut app = AppState::new();
        app.set_status(Board::Aa, Status::Loaded(Data::Aa(make_models(30))));
        app.move_to_bottom();
        assert_eq!(app.selected_idx(), 29);

        app.move_page_up();
        assert_eq!(app.selected_idx(), 19);

        app.move_page_up();
        assert_eq!(app.selected_idx(), 9);

        app.move_page_up();
        assert_eq!(app.selected_idx(), 0);

        app.move_page_up();
        assert_eq!(app.selected_idx(), 0);
    }

    #[test]
    fn move_to_top_and_bottom() {
        let mut app = AppState::new();
        app.set_status(Board::Aa, Status::Loaded(Data::Aa(make_models(20))));

        app.move_to_bottom();
        assert_eq!(app.selected_idx(), 19);

        app.move_to_top();
        assert_eq!(app.selected_idx(), 0);
    }

    #[test]
    fn move_to_bottom_on_empty_list() {
        let mut app = AppState::new();
        app.set_status(Board::Aa, Status::Loaded(Data::Aa(vec![])));

        app.move_to_bottom();
        assert_eq!(
            app.table_state.get(&Board::Aa).and_then(|s| s.selected()),
            None
        );
    }

    #[test]
    fn filter_cache_returns_all_indices_when_no_filter() {
        let mut app = AppState::new();
        app.set_status(Board::Aa, Status::Loaded(Data::Aa(make_models(5))));
        let indices = app.filter_cache(Board::Aa, "");
        assert_eq!(indices, &[0, 1, 2, 3, 4]);
    }

    #[test]
    fn filter_cache_returns_matching_indices() {
        let mut app = AppState::new();
        app.set_status(
            Board::Aa,
            Status::Loaded(Data::Aa(vec![
                aa_model("claude-sonnet", "Claude Sonnet", "Anthropic"),
                aa_model("gpt-5", "GPT-5", "OpenAI"),
                aa_model("claude-opus", "Claude Opus", "Anthropic"),
            ])),
        );
        let indices = app.filter_cache(Board::Aa, "anthropic");
        assert_eq!(indices, &[0, 2]);
    }

    #[test]
    fn filter_cache_invalidates_on_query_change() {
        let mut app = AppState::new();
        app.set_status(
            Board::Aa,
            Status::Loaded(Data::Aa(vec![
                aa_model("claude-sonnet", "Claude Sonnet", "Anthropic"),
                aa_model("gpt-5", "GPT-5", "OpenAI"),
            ])),
        );
        let indices = app.filter_cache(Board::Aa, "openai");
        assert_eq!(indices, &[1]);
        let indices = app.filter_cache(Board::Aa, "anthropic");
        assert_eq!(indices, &[0]);
    }

    #[test]
    fn sort_invalidates_filter_cache() {
        let mut app = AppState::new();
        let mut m1 = aa_model("claude", "Claude", "Anthropic");
        m1.intelligence_index = Some(60.0);
        m1.price_1m_blended_3_to_1 = Some(5.0);
        let mut m2 = aa_model("gpt", "GPT", "OpenAI");
        m2.intelligence_index = Some(50.0);
        m2.price_1m_blended_3_to_1 = Some(1.0);
        app.set_status(Board::Aa, Status::Loaded(Data::Aa(vec![m1, m2])));

        // Initially sorted by intelligence desc: [claude(60), gpt(50)]
        let indices = app.filter_cache(Board::Aa, "").to_vec();
        assert_eq!(indices, &[0, 1]);

        // Switch sort to price (ascending) — data re-sorted to [gpt($1), claude($5)]
        app.cycle_sort('p');

        // Cache should be invalidated; filter should see the new order
        let indices = app.filter_cache(Board::Aa, "").to_vec();
        if let Some(Status::Loaded(Data::Aa(models))) = app.status.get(&Board::Aa) {
            assert_eq!(
                models[indices[0]].name, "GPT",
                "gpt should be first (cheaper)"
            );
            assert_eq!(models[indices[1]].name, "Claude", "claude should be second");
        }

        // Verify filter still works correctly after sort
        let indices = app.filter_cache(Board::Aa, "claude").to_vec();
        assert_eq!(indices.len(), 1);
        if let Some(Status::Loaded(Data::Aa(models))) = app.status.get(&Board::Aa) {
            assert_eq!(models[indices[0]].name, "Claude");
        }
    }
}
