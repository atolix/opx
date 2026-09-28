#[path = "tui_command.rs"]
mod tui_command;
#[path = "tui_input.rs"]
mod tui_input;
#[path = "tui_state.rs"]
mod tui_state;
#[path = "tui_view.rs"]
mod tui_view;

use anyhow::Result;
use crossterm::{
    event::{self, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{io::stdout, path::Path};

use crate::{document::Document, markdown};
use tui_input::handle_key;
use tui_state::AppState;
use tui_view::draw;

pub fn run(path: &Path) -> Result<()> {
    let mut document = markdown::load(path)?;
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen)?;
    let result = app_loop(&mut document, path);
    disable_raw_mode()?;
    execute!(out, LeaveAlternateScreen)?;
    result
}

fn app_loop(document: &mut Document, path: &Path) -> Result<()> {
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;
    let mut state = AppState::default();

    loop {
        terminal.draw(|frame| draw(frame, document, &state))?;
        if !event::poll(std::time::Duration::from_millis(250))? {
            continue;
        }
        if let Event::Key(key) = event::read()? {
            if handle_key(&mut state, document, path, key)? {
                break;
            }
        }
    }
    Ok(())
}
