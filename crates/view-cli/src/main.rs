mod ui;

mod app {
    pub use view_core::app::*;
}

// Listener module no longer needed, Engine handles it

use std::{
    io::{self, Stdout},
    sync::Arc,
    time::Duration,
};
use parking_lot::RwLock;

use anyhow::Result;
use crossterm::{
    event::{self, Event as CEvent, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::app::AppState;
use view_core::engine::CoreEngine;

/// RAII Guard to ensure terminal is restored on exit, even during panics.
struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalGuard {
    fn new() -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        Ok(Self { terminal })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let app_state = Arc::new(RwLock::new(AppState::new()));
    
    // Spawn the core engine which manages all background processes and state mutations
    let _action_tx = CoreEngine::spawn_background(app_state.clone());

    let mut guard = TerminalGuard::new()?;
    let mut interval = tokio::time::interval(Duration::from_millis(16));

    loop {
        interval.tick().await;

        // UI only reads state and handles input; CoreEngine mutates the rest!

        if event::poll(Duration::from_millis(0))? {
            if let CEvent::Key(key) = event::read()? {
                let mut state = app_state.write();
                if state.search_mode {
                    match (key.code, key.modifiers) {
                        (KeyCode::Esc, _) => {
                            state.clear_search_query();
                            state.end_search();
                        }
                        (KeyCode::Enter, _) => state.end_search(),
                        (KeyCode::Backspace, _) => state.pop_search_char(),
                        (KeyCode::Char('c'), KeyModifiers::CONTROL) => state.should_quit = true,
                        (KeyCode::Char(ch), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                            state.append_search_char(ch);
                        }
                        _ => {}
                    }
                } else {
                    match (key.code, key.modifiers) {
                        (KeyCode::Char('q'), _) => state.should_quit = true,
                        (KeyCode::Char('c'), KeyModifiers::CONTROL) => state.should_quit = true,
                        (KeyCode::Down, _) | (KeyCode::Char('j'), _) => state.select_next(),
                        (KeyCode::Up, _) | (KeyCode::Char('k'), _) => state.select_previous(),
                        (KeyCode::PageDown, _) => state.select_next_page(),
                        (KeyCode::PageUp, _) => state.select_previous_page(),
                        (KeyCode::Home, _) => state.select_first(),
                        (KeyCode::End, _) => state.select_last(),
                        (KeyCode::Tab, _) => state.toggle_view_mode(),
                        (KeyCode::Char('f'), _) => state.cycle_filter_mode(),
                        (KeyCode::Char('/'), _) => state.begin_search(),
                        (KeyCode::Esc, _) => state.clear_search_query(),
                        _ => {}
                    }
                }
            }
        }

        {
            let state = app_state.read();
            if state.should_quit {
                break;
            }
            guard.terminal.draw(|frame| ui::render(frame, &state))?;
        }
    }

    Ok(())
}
