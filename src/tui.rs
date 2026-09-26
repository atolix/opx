use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Terminal,
};
use std::{io::stdout, path::Path};

use crate::markdown;

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

fn app_loop(document: &mut crate::document::Document, path: &Path) -> Result<()> {
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;
    let mut selected = 0usize;
    let mut detail = false;
    loop {
        terminal.draw(|frame| draw(frame, document, selected, detail))?;
        if !event::poll(std::time::Duration::from_millis(250))? {
            continue;
        }
        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') => break,
                KeyCode::Down | KeyCode::Char('j') => {
                    selected = (selected + 1).min(document.tasks.len().saturating_sub(1))
                }
                KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
                KeyCode::Char('n') => {
                    if let Some(task) = document.next_unchecked(Some(selected)) {
                        selected = task.index;
                    }
                }
                KeyCode::Char(' ') => {
                    if let Some(task) = document.tasks.get(selected) {
                        let updated = document.with_checked(selected, !task.checked)?;
                        markdown::save(path, &updated)?;
                        *document = updated;
                    }
                }
                KeyCode::Char('y') => {
                    if let Some(task) = document.tasks.get(selected) {
                        if let Some(command) = &task.command {
                            let mut clipboard = arboard::Clipboard::new()?;
                            clipboard.set_text(command.clone())?;
                        }
                    }
                }
                KeyCode::Enter => detail = !detail,
                _ => {}
            }
        }
    }
    Ok(())
}

fn draw(
    frame: &mut ratatui::Frame,
    document: &crate::document::Document,
    selected: usize,
    detail: bool,
) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(2)])
        .split(area);
    let title = format!(
        "opx  {} / {}  {:.0}%",
        document.completed_count(),
        document.tasks.len(),
        document.progress() * 100.0
    );
    let mut lines = vec![Line::from(Span::styled(
        title,
        Style::default().add_modifier(Modifier::BOLD),
    ))];
    let mut last_section: Option<&str> = None;
    for task in &document.tasks {
        if (!task.section.is_empty()).then_some(task.section.as_str()) != last_section {
            if !task.section.is_empty() {
                lines.push(Line::from(Span::styled(
                    task.section.clone(),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )));
            }
            last_section = (!task.section.is_empty()).then_some(task.section.as_str());
        }
        let marker = if task.checked {
            "✓"
        } else if task.index == selected {
            ">"
        } else {
            "○"
        };
        lines.push(Line::from(vec![Span::raw(format!(
            " {} {}",
            marker, task.title
        ))]));
        if detail && task.index == selected {
            if let Some(command) = &task.command {
                lines.push(Line::from(Span::styled(
                    format!("   {}", command),
                    Style::default().fg(Color::Yellow),
                )));
            }
        }
    }
    let help = "j/k or arrows move   space toggle   y copy   enter detail   n next   q quit";
    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title("Runbook"))
            .wrap(Wrap { trim: false }),
        chunks[0],
    );
    frame.render_widget(Paragraph::new(help), chunks[1]);
}
