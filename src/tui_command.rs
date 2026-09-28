use anyhow::Result;
use std::process::{Command, Output};

use crate::document::Document;

use super::tui_state::Overlay;

pub(crate) fn execute_overlay_command(command: &str) -> Overlay {
    match run_shell_command(command) {
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
            Overlay::Result {
                output: format!("exit status: {status}\n\n{text}"),
                scroll: 0,
            }
        }
        Err(error) => Overlay::Result {
            output: error.to_string(),
            scroll: 0,
        },
    }
}

pub(crate) fn copy_selected(document: &Document, selected: usize) -> Result<Option<String>> {
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
