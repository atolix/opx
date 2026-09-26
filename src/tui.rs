use anyhow::{Context, Result};
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
use std::{
    io::stdout,
    path::Path,
    process::{Command, ExitStatus},
};

use crate::{document::Document, markdown};

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
    let mut selected = 0usize;
    let mut detail = false;
    let mut status_message = String::new();
    let mut pending_execution = None;
    loop {
        terminal.draw(|frame| {
            draw(
                frame,
                document,
                selected,
                detail,
                &status_message,
                pending_execution.is_some(),
            )
        })?;
        if !event::poll(std::time::Duration::from_millis(250))? {
            continue;
        }
        if let Event::Key(key) = event::read()? {
            status_message.clear();
            if let Some(index) = pending_execution {
                match key.code {
                    KeyCode::Char('y') => {
                        pending_execution = None;
                        match execute_selected(&mut terminal, document, index) {
                            Ok(status) if status.success() => {
                                status_message = "Command finished successfully".into()
                            }
                            Ok(status) => {
                                status_message = format!(
                                    "Command exited with status {}",
                                    status
                                        .code()
                                        .map_or_else(|| "unknown".into(), |code| code.to_string())
                                )
                            }
                            Err(error) => status_message = format!("Command failed: {error}"),
                        }
                    }
                    KeyCode::Char('n') | KeyCode::Esc => {
                        pending_execution = None;
                        status_message = "Command execution cancelled".into();
                    }
                    _ => {}
                }
                continue;
            }
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
                KeyCode::Char(' ') | KeyCode::Char('x') => {
                    if let Some(task) = document.tasks.get(selected) {
                        let updated = document.with_checked(selected, !task.checked)?;
                        markdown::save(path, &updated)?;
                        *document = updated;
                    }
                }
                KeyCode::Char('y') => match copy_selected(document, selected) {
                    Ok(Some(title)) => status_message = format!("Copied command: {title}"),
                    Ok(None) => {
                        status_message = "Copy skipped: selected task has no command".into()
                    }
                    Err(error) => status_message = format!("Copy failed: {error}"),
                },
                KeyCode::Char('r') => {
                    if document
                        .tasks
                        .get(selected)
                        .and_then(|task| task.command.as_ref())
                        .is_some()
                    {
                        pending_execution = Some(selected);
                        status_message = "Run command? Press y to execute, n to cancel".into();
                    } else {
                        status_message = "Run skipped: selected task has no command".into();
                    }
                }
                KeyCode::Enter => detail = !detail,
                _ => {}
            }
        }
    }
    Ok(())
}

fn copy_selected(document: &Document, selected: usize) -> Result<Option<String>> {
    let Some(task) = document.tasks.get(selected) else {
        return Ok(None);
    };
    let Some(command) = &task.command else {
        return Ok(None);
    };
    let mut clipboard = arboard::Clipboard::new()?;
    clipboard.set_text(command.clone())?;
    Ok(Some(task.title.clone()))
}

fn execute_selected(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    document: &Document,
    selected: usize,
) -> Result<ExitStatus> {
    let command = document
        .tasks
        .get(selected)
        .and_then(|task| task.command.as_deref())
        .context("selected task has no associated code block")?;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    let result = run_shell_command(command);
    enable_raw_mode()?;
    execute!(terminal.backend_mut(), EnterAlternateScreen)?;
    terminal.clear()?;
    result
}

#[cfg(unix)]
fn run_shell_command(command: &str) -> Result<ExitStatus> {
    Ok(Command::new("sh").arg("-c").arg(command).status()?)
}

#[cfg(windows)]
fn run_shell_command(command: &str) -> Result<ExitStatus> {
    Ok(Command::new("cmd").args(["/C", command]).status()?)
}

fn draw(
    frame: &mut ratatui::Frame,
    document: &Document,
    selected: usize,
    detail: bool,
    status_message: &str,
    pending_execution: bool,
) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(2),
        ])
        .split(area);
    let progress = document.progress();
    let progress_percent = (progress * 100.0).round().clamp(0.0, 100.0) as u16;
    let bar_width = chunks[0].width.saturating_sub(7) as usize;
    let filled_width = bar_width * progress_percent as usize / 100;
    let progress_bar = format!(
        "{}{} {:>3}%",
        "█".repeat(filled_width),
        "░".repeat(bar_width.saturating_sub(filled_width)),
        progress_percent
    );
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            progress_bar,
            Style::default().fg(Color::Rgb(170, 150, 255)),
        )))
        .block(Block::default().borders(Borders::ALL).title(format!(
            "opx  Progress {} / {}",
            document.completed_count(),
            document.tasks.len()
        ))),
        chunks[0],
    );

    if detail {
        let panes = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(64), Constraint::Percentage(36)])
            .split(chunks[1]);
        frame.render_widget(
            Paragraph::new(task_lines(document, selected)).block(runbook_block()),
            panes[0],
        );
        frame.render_widget(
            Paragraph::new(detail_lines(document, selected))
                .block(Block::default().borders(Borders::ALL).title("Detail"))
                .wrap(Wrap { trim: false }),
            panes[1],
        );
    } else {
        frame.render_widget(
            Paragraph::new(task_lines(document, selected)).block(runbook_block()),
            chunks[1],
        );
    }

    let footer = if status_message.is_empty() {
        "j/k move   space/x toggle   y copy   r run   enter detail   n next   q quit".to_string()
    } else if pending_execution {
        format!("{status_message}")
    } else {
        format!("{status_message}   |   j/k move   space/x toggle   r run   q quit")
    };
    frame.render_widget(Paragraph::new(footer), chunks[2]);
}

fn runbook_block() -> Block<'static> {
    Block::default().borders(Borders::ALL).title("Runbook")
}

fn task_lines(document: &Document, selected: usize) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let mut last_path: Vec<String> = Vec::new();
    for task in &document.tasks {
        let common = last_path
            .iter()
            .zip(&task.section_path)
            .take_while(|(left, right)| left == right)
            .count();
        for (depth, heading) in task.section_path.iter().enumerate().skip(common) {
            lines.push(Line::from(Span::styled(
                format!("{}{}", "  ".repeat(depth), heading),
                Style::default()
                    .fg(if depth == 0 { Color::Cyan } else { Color::Blue })
                    .add_modifier(Modifier::BOLD),
            )));
        }
        last_path = task.section_path.clone();

        let task_style = if task.index == selected {
            Style::default()
                .fg(Color::Rgb(170, 150, 255))
                .add_modifier(Modifier::BOLD)
        } else if task.checked {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default().fg(Color::Gray)
        };
        let marker = if task.checked { "✓" } else { "○" };
        let indent = "  ".repeat(task.section_path.len());
        let title_style = if task.checked {
            task_style.add_modifier(Modifier::CROSSED_OUT)
        } else {
            task_style
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{indent}{marker} "), task_style),
            Span::styled(task.title.clone(), title_style),
        ]));
    }
    lines
}

fn detail_lines(document: &Document, selected: usize) -> Vec<Line<'static>> {
    let Some(task) = document.tasks.get(selected) else {
        return vec![Line::from("No task selected")];
    };
    let mut lines = vec![
        Line::from(Span::styled(
            task.title.clone(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(format!(
            "Status: {}",
            if task.checked { "checked" } else { "unchecked" }
        )),
        Line::from(format!(
            "Section: {}",
            if task.section_path.is_empty() {
                "(root)".to_string()
            } else {
                task.section_path.join(" > ")
            }
        )),
    ];
    if let Some(language) = &task.language {
        lines.push(Line::from(format!("Language: {language}")));
    }
    if let Some(details) = &task.details {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Details",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )));
        lines.extend(details.lines().map(|line| Line::from(format!("  {line}"))));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Command",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));
    if let Some(command) = &task.command {
        lines.extend(command.lines().map(|line| Line::from(format!("  {line}"))));
    } else {
        lines.push(Line::from("  (none)"));
    }
    lines
}
