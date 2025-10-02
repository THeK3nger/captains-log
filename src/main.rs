use anyhow::Result;
use clap::CommandFactory;
use clap::Parser;
use colored::*;
use std::cmp::min;

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

    /// Override database file location
    #[arg(short = 'f', long = "file", global = true)]
    database_file: Option<String>,
}

/// Calculate Levenshtein distance between two strings
fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    let len1 = s1.len();
    let len2 = s2.len();

    if len1 == 0 {
        return len2;
    }
    if len2 == 0 {
        return len1;
    }

    let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];

    for i in 0..=len1 {
        matrix[i][0] = i;
    }
    for j in 0..=len2 {
        matrix[0][j] = j;
    }

    for (i, c1) in s1.chars().enumerate() {
        for (j, c2) in s2.chars().enumerate() {
            let cost = if c1 == c2 { 0 } else { 1 };
            matrix[i + 1][j + 1] = min(
                min(
                    matrix[i][j + 1] + 1, // deletion
                    matrix[i + 1][j] + 1, // insertion
                ),
                matrix[i][j] + cost, // substitution
            );
        }
    }

    matrix[len1][len2]
}

/// Check if a word might be a typo of a known command
fn is_likely_command_typo(word: &str) -> bool {
    const KNOWN_COMMANDS: &[&str] = &[
        "list", "show", "search", "delete", "move", "edit", "new", "calendar", "config", "export",
    ];
    const MAX_DISTANCE: usize = 2;

    // Check if the word is similar to any known command
    for command in KNOWN_COMMANDS {
        if levenshtein_distance(word, command) <= MAX_DISTANCE {
            return true;
        }
    }

    false
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
            if let Some(content) = cli.content {
                // Check if this might be a typo of a command
                let should_create = if !content.contains(' ') && is_likely_command_typo(&content) {
                    // Single word that's similar to a known command - ask for confirmation
                    print!(
                        "{}",
                        format!(
                            "Did you mean to run a command? Entry content is '{}'. Create entry? (y/N): ",
                            content
                        )
                        .yellow()
                        .bold()
                    );
                    std::io::Write::flush(&mut std::io::stdout())?;

                    let mut input = String::new();
                    std::io::stdin().read_line(&mut input)?;
                    let input = input.trim().to_lowercase();

                    input == "y" || input == "yes"
                } else {
                    // Multi-word entry or doesn't look like a typo - proceed without confirmation
                    true
                };

                if should_create {
                    let id = journal.create_entry(None, &content, cli.journal.as_deref())?;
                    println!("{}", format!("Entry {} added successfully", id).green());
                } else {
                    println!("{}", "Entry creation cancelled".yellow());
                }
            } else {
                Cli::command().print_help()?;
            }
        }
    }

    Ok(())
}
