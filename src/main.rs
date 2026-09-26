mod cli;
mod document;
mod markdown;
mod tui;

use anyhow::Result;

fn main() -> Result<()> {
    cli::run()
}
