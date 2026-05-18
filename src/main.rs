mod aa;
mod arena;
mod board;
mod coding_agents;
mod rsc;
mod ui;

use std::io;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
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
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    Ok(Terminal::new(backend)?)
}

fn teardown_terminal(terminal: &mut Term) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

fn run(terminal: &mut Term) -> Result<()> {
    let mut app = AppState::new();
    let (tx, rx) = mpsc::channel::<Msg>();

    for &board in app.boards {
        app.set_status(board, Status::Loading);
        spawn_fetch(board, tx.clone());
    }

    let tick = Duration::from_millis(120);
    let mut last_draw = Instant::now() - tick;

    loop {
        while let Ok((board, result)) = rx.try_recv() {
            match result {
                Ok(data) => app.set_status(board, Status::Loaded(data)),
                Err(e) => app.set_status(board, Status::Error(format!("{e:#}"))),
            }
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
            Event::Mouse(m) => handle_mouse(m, &mut app, &tx),
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
            KeyCode::Char('?') | KeyCode::Char('h') | KeyCode::Esc | KeyCode::Char('q') => {
                app.close_help()
            }
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
        KeyCode::Char('?') | KeyCode::Char('h') => app.toggle_help(),
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
        KeyCode::Char('y') => {
            if let Some(name) = app.selected_name() {
                copy_to_clipboard(&name);
            }
        }
        KeyCode::Char('/') => app.begin_filter(),
        KeyCode::Down | KeyCode::Char('j') => app.move_down(),
        KeyCode::Up | KeyCode::Char('k') => app.move_up(),
        KeyCode::PageDown => app.move_page_down(),
        KeyCode::PageUp => app.move_page_up(),
        KeyCode::Home => app.move_to_top(),
        KeyCode::End => app.move_to_bottom(),
        KeyCode::Char('g') => app.move_to_top(),
        KeyCode::Char('G') => app.move_to_bottom(),
        KeyCode::Char(c @ ('1'..='9' | '0' | '-' | '=')) => {
            if let Some(idx) = Board::from_shortcut(c) {
                app.select_board(idx);
                ensure_loaded(app, tx);
            }
        }
        KeyCode::Char(c @ ('a' | 'c' | 'i' | 'n' | 'p' | 's' | 't' | 'u' | 'v')) => {
            app.cycle_sort(c);
        }
        KeyCode::Char(_) => {}
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

fn handle_mouse(m: MouseEvent, app: &mut AppState, tx: &mpsc::Sender<Msg>) {
    match m.kind {
        MouseEventKind::ScrollUp => app.move_up(),
        MouseEventKind::ScrollDown => app.move_down(),
        MouseEventKind::Down(MouseButton::Left) if m.row < 3 => {
            // Tab bar is rows 0-2 (3 lines with borders)
            let tab_count = app.boards.len();
            let width = crossterm::terminal::size().map(|(w, _)| w).unwrap_or(80) as usize;
            if width > 0 && tab_count > 0 {
                let idx = m.column as usize * tab_count / width;
                if idx < tab_count {
                    app.select_board(idx);
                    ensure_loaded(app, tx);
                }
            }
        }
        _ => {}
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

fn copy_to_clipboard(text: &str) {
    use std::io::Write;
    let encoded = base64_encode(text.as_bytes());
    let _ = write!(io::stdout(), "\x1b]52;c;{encoded}\x07");
    let _ = io::stdout().flush();
}

fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((triple >> 18) & 0x3F) as usize] as char);
        out.push(TABLE[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(triple & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_encode_basic() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"hello"), "aGVsbG8=");
    }
}
