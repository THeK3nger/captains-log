use anyhow::Result;
use clap::CommandFactory;
use clap::Parser;
use colored::*;

mod cli;
mod config;
mod database;
mod export;
mod import;
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

    /// Journal category (global option for all commands)
    #[arg(long, global = true)]
    journal: Option<String>,

    /// Override database file location
    #[arg(short = 'd', long = "database", global = true)]
    database_file: Option<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let config = Config::load()?;
    let db = if let Some(db_file) = &cli.database_file {
        Database::new_with_path(db_file)?
    } else {
        Database::new(&config)?
    };
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
            Cli::command().print_help()?;
        }
    }

    Ok(())
}
