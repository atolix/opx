use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Terminal,
};
use std::{
    io::stdout,
    path::Path,
    process::{Command, Output},
};

use crate::{document::Document, markdown};

enum Overlay {
    Confirm { command: String },
    Result { output: String },
}

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
    let mut overlay = None;
    loop {
        terminal.draw(|frame| {
            draw(
                frame,
                document,
                selected,
                detail,
                &status_message,
                overlay.as_ref(),
            )
        })?;
        if !event::poll(std::time::Duration::from_millis(250))? {
            continue;
        }
        if let Event::Key(key) = event::read()? {
            status_message.clear();
            let ctrl_c =
                key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL);
            if ctrl_c {
                if overlay.is_some() {
                    overlay = None;
                    status_message = "Dialog closed".into();
                    continue;
                }
                break;
            }
            if let Some(current_overlay) = overlay.take() {
                match (current_overlay, key.code) {
                    (Overlay::Confirm { command }, KeyCode::Char('y')) => {
                        match run_shell_command(&command) {
                            Ok(output) => {
                                let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
                                let stderr = String::from_utf8_lossy(&output.stderr);
                                if !stderr.is_empty() {
                                    if !text.is_empty() {
                                        text.push('\n');
                                    }
                                    text.push_str("stderr:\n");
                                    text.push_str(&stderr);
                                }
                                if text.is_empty() {
                                    text = "(no output)".into();
                                }
                                let status = output
                                    .status
                                    .code()
                                    .map_or_else(|| "unknown".to_string(), |code| code.to_string());
                                overlay = Some(Overlay::Result {
                                    output: format!("exit status: {status}\n\n{text}"),
                                });
                            }
                            Err(error) => {
                                overlay = Some(Overlay::Result {
                                    output: error.to_string(),
                                });
                            }
                        }
                    }
                    (
                        Overlay::Confirm { .. },
                        KeyCode::Char('n') | KeyCode::Char('q') | KeyCode::Esc,
                    ) => {
                        status_message = "Command execution cancelled".into();
                    }
                    (Overlay::Result { .. }, KeyCode::Char('q')) => {}
                    (other, _) => overlay = Some(other),
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
                        let command = document.tasks[selected]
                            .command
                            .clone()
                            .expect("command presence checked above");
                        overlay = Some(Overlay::Confirm { command });
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

#[cfg(unix)]
fn run_shell_command(command: &str) -> Result<Output> {
    Ok(Command::new("sh").arg("-c").arg(command).output()?)
}

#[cfg(windows)]
fn run_shell_command(command: &str) -> Result<Output> {
    Ok(Command::new("cmd").args(["/C", command]).output()?)
}

fn draw(
    frame: &mut ratatui::Frame,
    document: &Document,
    selected: usize,
    detail: bool,
    status_message: &str,
    overlay: Option<&Overlay>,
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
    } else {
        format!("{status_message}   |   j/k move   space/x toggle   r run   q quit")
    };
    frame.render_widget(Paragraph::new(footer), chunks[2]);
    if let Some(overlay) = overlay {
        draw_overlay(frame, overlay);
    }
}

fn draw_overlay(frame: &mut ratatui::Frame, overlay: &Overlay) {
    let area = centered_rect(64, 45, frame.area());
    frame.render_widget(Clear, area);
    let outer = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(5),
            Constraint::Length(1),
        ])
        .split(inner);
    let (message, code) = match overlay {
        Overlay::Confirm { command } => ("Run this command?", command.as_str()),
        Overlay::Result { output } => ("", output.as_str()),
    };
    frame.render_widget(
        Paragraph::new(message)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: false }),
        chunks[0],
    );
    frame.render_widget(
        Paragraph::new(code)
            .block(Block::default().borders(Borders::ALL))
            .wrap(Wrap { trim: false }),
        chunks[1],
    );
    frame.render_widget(
        Paragraph::new(match overlay {
            Overlay::Confirm { .. } => "y execute   n / q / Esc cancel",
            Overlay::Result { .. } => "q return",
        })
        .alignment(Alignment::Center),
        chunks[2],
    );
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
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
