use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use crate::{markdown, tui};

#[derive(Debug, Parser)]
#[command(
    name = "opx",
    version,
    about = "Operate a Markdown runbook from the terminal"
)]
pub struct Cli {
    /// Markdown runbook to open in the TUI. This is the default operation.
    file: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run a non-interactive operation.
    Cli {
        #[command(subcommand)]
        command: CliCommand,
    },
}

#[derive(Debug, Subcommand)]
enum CliCommand {
    Status {
        file: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Next {
        file: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Check {
        file: PathBuf,
        index: usize,
        #[arg(long)]
        json: bool,
    },
    Uncheck {
        file: PathBuf,
        index: usize,
        #[arg(long)]
        json: bool,
    },
    Copy {
        file: PathBuf,
        index: usize,
        #[arg(long)]
        json: bool,
    },
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    match (cli.file, cli.command) {
        (Some(file), None) => tui::run(&file),
        (None, Some(Command::Cli { command })) => run_cli(command),
        (Some(_), Some(_)) => anyhow::bail!("a Markdown file cannot be combined with a subcommand"),
        (None, None) => anyhow::bail!("provide a Markdown file or use `opx cli <command>`"),
    }
}

fn run_cli(command: CliCommand) -> Result<()> {
    match command {
        CliCommand::Status { file, json } => {
            let document = markdown::load(&file)?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({"total": document.tasks.len(), "completed": document.completed_count(), "progress": document.progress(), "tasks": document.tasks})
                );
            } else {
                println!(
                    "{}/{} ({:.0}%)",
                    document.completed_count(),
                    document.tasks.len(),
                    document.progress() * 100.0
                );
            }
            Ok(())
        }
        CliCommand::Next { file, json } => {
            let document = markdown::load(&file)?;
            let task = document.next_unchecked(None).context("no unchecked task")?;
            print_task(task, json)
        }
        CliCommand::Check { file, index, json } => set_checked(&file, index, true, json),
        CliCommand::Uncheck { file, index, json } => set_checked(&file, index, false, json),
        CliCommand::Copy { file, index, json } => {
            let document = markdown::load(&file)?;
            let task = document
                .tasks
                .get(index)
                .context("task index out of range")?;
            let command = task
                .command
                .as_deref()
                .context("task has no associated code block")?;
            let mut clipboard = arboard::Clipboard::new().context("failed to access clipboard")?;
            clipboard
                .set_text(command.to_owned())
                .context("failed to copy command")?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({"index": task.index, "copied": true})
                );
            } else {
                println!("Copied task {} command to clipboard.", task.index);
            }
            Ok(())
        }
    }
}

fn set_checked(file: &PathBuf, index: usize, checked: bool, json: bool) -> Result<()> {
    let document = markdown::load(file)?;
    let updated = document.with_checked(index, checked)?;
    markdown::save(file, &updated)?;
    let task = updated
        .tasks
        .get(index)
        .context("task index out of range")?;
    print_task(task, json)
}

fn print_task(task: &crate::document::Task, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string(task)?);
    } else {
        println!("{}: {}", task.index, task.title);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn markdown_file_is_the_default_operation() {
        let cli = Cli::try_parse_from(["opx", "runbook.md"]).unwrap();
        assert!(cli.file.is_some());
        assert!(cli.command.is_none());
    }

    #[test]
    fn non_interactive_commands_are_under_cli() {
        let cli = Cli::try_parse_from(["opx", "cli", "status", "runbook.md", "--json"]).unwrap();
        assert!(matches!(cli.command, Some(Command::Cli { .. })));
        assert!(cli.file.is_none());
    }
}
