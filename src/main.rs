mod aa;
mod arena;
mod board;
mod rsc;
mod ui;

use std::io;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use board::{Board, Status};
use ui::AppState;

type Msg = (Board, anyhow::Result<board::Data>);

fn main() -> Result<()> {
    let mut terminal = setup_terminal().context("setting up terminal")?;
    let result = run(&mut terminal);
    teardown_terminal(&mut terminal).ok();
    result
}

type Term = Terminal<CrosstermBackend<io::Stdout>>;

fn setup_terminal() -> Result<Term> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    Ok(Terminal::new(backend)?)
}

fn teardown_terminal(terminal: &mut Term) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn run(terminal: &mut Term) -> Result<()> {
    let mut app = AppState::new();
    let (tx, rx) = mpsc::channel::<Msg>();

    let initial = app.current_board();
    app.set_status(initial, Status::Loading);
    spawn_fetch(initial, tx.clone());

    let tick = Duration::from_millis(120);
    let mut last_draw = Instant::now() - tick;

    loop {
        let mut prefetch: Vec<Board> = Vec::new();
        while let Ok((board, result)) = rx.try_recv() {
            match result {
                Ok(data) => app.set_status(board, Status::Loaded(data)),
                Err(e) => app.set_status(board, Status::Error(format!("{e:#}"))),
            }
            if let Some(idx) = app.boards.iter().position(|&b| b == board) {
                let n = app.boards.len();
                for delta in [n - 1, 1] {
                    let neighbor = app.boards[(idx + delta) % n];
                    if !app.status.contains_key(&neighbor)
                        && !prefetch.contains(&neighbor)
                    {
                        prefetch.push(neighbor);
                    }
                }
            }
        }
        for b in prefetch {
            app.set_status(b, Status::Loading);
            spawn_fetch(b, tx.clone());
        }

        if last_draw.elapsed() >= tick {
            terminal.draw(|f| ui::render(f, &mut app))?;
            last_draw = Instant::now();
        }

        if !event::poll(Duration::from_millis(80))? {
            continue;
        }
        match event::read()? {
            Event::Key(k) if k.kind != KeyEventKind::Press => {}
            Event::Key(k) if handle_key(k, &mut app, &tx) => return Ok(()),
            Event::Key(_) => {}
            Event::Resize(_, _) => {
                last_draw = Instant::now() - tick;
            }
            _ => {}
        }
    }
}

fn handle_key(k: KeyEvent, app: &mut AppState, tx: &mpsc::Sender<Msg>) -> bool {
    if k.modifiers.contains(KeyModifiers::CONTROL) && matches!(k.code, KeyCode::Char('c')) {
        return true;
    }
    if app.is_help_open() {
        match k.code {
            KeyCode::Char('?') | KeyCode::Esc | KeyCode::Char('q') => app.close_help(),
            _ => {}
        }
        return false;
    }
    if k.modifiers.contains(KeyModifiers::CONTROL) && matches!(k.code, KeyCode::Char('u')) {
        app.clear_current_filter();
        return false;
    }
    if app.is_filter_editing() {
        handle_filter_key(k, app);
        return false;
    }

    match k.code {
        KeyCode::Char('q') | KeyCode::Esc => return true,
        KeyCode::Char('?') => app.toggle_help(),
        KeyCode::Char('r') => {
            let b = app.current_board();
            app.set_status(b, Status::Loading);
            spawn_fetch(b, tx.clone());
        }
        KeyCode::Char('[') => {
            app.cycle_board(-1);
            ensure_loaded(app, tx);
        }
        KeyCode::Char(']') => {
            app.cycle_board(1);
            ensure_loaded(app, tx);
        }
        KeyCode::Char('o') => app.toggle_dir(),
        KeyCode::Char('m') => app.toggle_view(),
        KeyCode::Char('v') if matches!(app.current_board(), Board::Aa) => app.toggle_view(),
        KeyCode::Char('/') => app.begin_filter(),
        KeyCode::Down | KeyCode::Char('j') => app.move_down(),
        KeyCode::Up | KeyCode::Char('k') => app.move_up(),
        KeyCode::Char(c @ ('1'..='9' | '0' | '-')) => {
            if let Some(idx) = board_idx_from_char(c, app) {
                app.select_board(idx);
                ensure_loaded(app, tx);
            } else {
                app.cycle_sort(c);
            }
        }
        KeyCode::Char(c) => app.cycle_sort(c),
        _ => {}
    }
    false
}

fn handle_filter_key(k: KeyEvent, app: &mut AppState) {
    match k.code {
        KeyCode::Enter | KeyCode::Esc => app.finish_filter(),
        KeyCode::Backspace => app.pop_filter_char(),
        KeyCode::Char(c) if k.modifiers.is_empty() || k.modifiers == KeyModifiers::SHIFT => {
            app.push_filter_char(c)
        }
        _ => {}
    }
}

fn board_idx_from_char(c: char, app: &AppState) -> Option<usize> {
    let idx = match c {
        '1'..='9' => (c as u8 - b'1') as usize,
        '0' => 9,
        '-' => 10,
        _ => return None,
    };
    if idx < app.boards.len() {
        Some(idx)
    } else {
        None
    }
}

fn ensure_loaded(app: &mut AppState, tx: &mpsc::Sender<Msg>) {
    let b = app.current_board();
    if app.status.contains_key(&b) {
        return;
    }
    app.set_status(b, Status::Loading);
    spawn_fetch(b, tx.clone());
}

fn spawn_fetch(board: Board, tx: mpsc::Sender<Msg>) {
    thread::spawn(move || {
        let result = board::fetch(board);
        let _ = tx.send((board, result));
    });
}
