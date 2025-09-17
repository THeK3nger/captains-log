use anyhow::Result;
use clap::CommandFactory;
use clap::Parser;
use colored::*;

mod cli;
mod config;
mod database;
mod export;
mod journal;

use cli::Commands;
use config::Config;
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

    /// Journal category for quick entries
    #[arg(long, global = true)]
    journal: Option<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let config = Config::load()?;
    let db = Database::new(&config)?;
    let journal = Journal::new(db);

    if config.display.colors_enabled {
        colored::control::set_override(true);
    } else {
        colored::control::set_override(false);
    }

    match cli.command {
        Some(command) => {
            cli::handle_command(command, &journal, &config, cli.journal.as_deref())?;
        }
        None => {
            if let Some(content) = cli.content {
                let id = journal.create_entry(None, &content, cli.journal.as_deref())?;
                println!("{}", format!("Entry {} added successfully", id).green());
            } else {
                Cli::command().print_help()?;
            }
        }
    }

    Ok(())
}
