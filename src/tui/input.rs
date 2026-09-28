use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::Path;

use crate::{document::Document, markdown};

use super::{
    command::{copy_selected, execute_overlay_command},
    state::{AppState, Overlay},
};

pub(crate) fn handle_key(
    state: &mut AppState,
    document: &mut Document,
    path: &Path,
    key: KeyEvent,
) -> Result<bool> {
    state.status_message.clear();
    let ctrl_c = key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL);
    if ctrl_c {
        if state.overlay.is_some() {
            state.overlay = None;
            state.status_message = "Dialog closed".into();
            return Ok(false);
        }
        return Ok(true);
    }

    if let Some(current_overlay) = state.overlay.take() {
        match (current_overlay, key.code) {
            (Overlay::Confirm { command }, KeyCode::Char('y')) => {
                state.overlay = Some(execute_overlay_command(&command));
            }
            (Overlay::Confirm { .. }, KeyCode::Char('n') | KeyCode::Char('q') | KeyCode::Esc) => {
                state.status_message = "Command execution cancelled".into();
            }
            (Overlay::Result { .. }, KeyCode::Char('q')) => {}
            (Overlay::Result { output, scroll }, KeyCode::Down | KeyCode::Char('j')) => {
                state.overlay = Some(Overlay::Result {
                    output,
                    scroll: scroll.saturating_add(1),
                });
            }
            (Overlay::Result { output, scroll }, KeyCode::Up | KeyCode::Char('k')) => {
                state.overlay = Some(Overlay::Result {
                    output,
                    scroll: scroll.saturating_sub(1),
                });
            }
            (other, _) => state.overlay = Some(other),
        }
        return Ok(false);
    }

    match key.code {
        KeyCode::Char('q') => Ok(true),
        KeyCode::Down | KeyCode::Char('j') => {
            state.selected = (state.selected + 1).min(document.tasks.len().saturating_sub(1));
            Ok(false)
        }
        KeyCode::Up | KeyCode::Char('k') => {
            state.selected = state.selected.saturating_sub(1);
            Ok(false)
        }
        KeyCode::Char('n') => {
            if let Some(task) = document.next_unchecked(Some(state.selected)) {
                state.selected = task.index;
            }
            Ok(false)
        }
        KeyCode::Char(' ') | KeyCode::Char('x') => {
            if let Some(task) = document.tasks.get(state.selected) {
                let updated = document.with_checked(state.selected, !task.checked)?;
                markdown::save(path, &updated)?;
                *document = updated;
            }
            Ok(false)
        }
        KeyCode::Char('y') => {
            match copy_selected(document, state.selected) {
                Ok(Some(title)) => state.status_message = format!("Copied command: {title}"),
                Ok(None) => {
                    state.status_message = "Copy skipped: selected task has no command".into()
                }
                Err(error) => state.status_message = format!("Copy failed: {error}"),
            }
            Ok(false)
        }
        KeyCode::Char('r') => {
            if let Some(command) = document
                .tasks
                .get(state.selected)
                .and_then(|task| task.command.clone())
            {
                state.overlay = Some(Overlay::Confirm { command });
            } else {
                state.status_message = "Run skipped: selected task has no command".into();
            }
            Ok(false)
        }
        KeyCode::Enter => {
            state.detail = !state.detail;
            Ok(false)
        }
        _ => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn command_execution_flows_through_confirmation_and_result_dialogs() {
        let mut document =
            markdown::parse("- [ ] print a message\n\n```sh\nprintf 'hello from test'\n```\n")
                .unwrap();
        let mut state = AppState::default();
        let path = Path::new("runbook.md");

        assert!(!handle_key(
            &mut state,
            &mut document,
            path,
            KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE)
        )
        .unwrap());
        assert!(matches!(state.overlay, Some(Overlay::Confirm { .. })));

        assert!(!handle_key(
            &mut state,
            &mut document,
            path,
            KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE)
        )
        .unwrap());
        assert!(matches!(state.overlay,
            Some(Overlay::Result { ref output, scroll: 0 }) if output.contains("hello from test")));

        handle_key(
            &mut state,
            &mut document,
            path,
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
        )
        .unwrap();
        assert!(matches!(
            state.overlay,
            Some(Overlay::Result { scroll: 1, .. })
        ));

        handle_key(
            &mut state,
            &mut document,
            path,
            KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE),
        )
        .unwrap();
        assert!(state.overlay.is_none());
    }

    #[test]
    fn q_cancels_confirmation_without_running_a_command() {
        let mut document =
            markdown::parse("- [ ] print a message\n\n```sh\necho should-not-run\n```\n").unwrap();
        let mut state = AppState::default();
        let path = Path::new("runbook.md");
        state.overlay = Some(Overlay::Confirm {
            command: "echo should-not-run".into(),
        });

        handle_key(
            &mut state,
            &mut document,
            path,
            KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE),
        )
        .unwrap();
        assert!(state.overlay.is_none());
        assert_eq!(state.status_message, "Command execution cancelled");
    }
}
