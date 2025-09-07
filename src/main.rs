use anyhow::Result;
use clap::CommandFactory;
use clap::Parser;
use colored::*;

mod cli;
mod database;
mod journal;

use cli::Commands;
use database::Database;
use journal::Journal;

#[derive(Parser)]
#[command(name = "cl")]
#[command(about = "Captain's Log - A terminal journaling application")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Quick entry content (when no subcommand is provided)
    content: Option<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let db = Database::new()?;
    let journal = Journal::new(db);

    match cli.command {
        Some(command) => {
            cli::handle_command(command, &journal)?;
        }
        None => {
            if let Some(content) = cli.content {
                let id = journal.create_entry(None, &content)?;
                println!("{}", format!("Entry {} added successfully", id).green());
            } else {
                Cli::command().print_help()?;
            }
        }
    }

    Ok(())
}
