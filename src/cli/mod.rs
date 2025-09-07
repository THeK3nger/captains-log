use crate::journal::Journal;
use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum Commands {
    /// List all entries
    List,

    /// Show a specific entry by ID
    Show {
        /// Entry ID to show
        id: i64,
    },

    /// Search entries
    Search {
        /// Search query
        query: String,
    },

    /// Delete an entry
    Delete {
        /// Entry ID to delete
        id: i64,
    },
}

pub fn handle_command(command: Commands, journal: &Journal) -> Result<()> {
    match command {
        Commands::List => {
            let entries = journal.list_entries()?;
            if entries.is_empty() {
                println!("No entries found");
            } else {
                for entry in entries {
                    println!(
                        "[{}] {} - {}",
                        entry.id,
                        entry.created_at.format("%Y-%m-%d %H:%M"),
                        entry.title.as_deref().unwrap_or("Untitled")
                    );
                }
            }
        }
        Commands::Show { id } => match journal.get_entry(id)? {
            Some(entry) => {
                println!("ID: {}", entry.id);
                println!("Date: {}", entry.created_at.format("%Y-%m-%d %H:%M:%S"));
                if let Some(title) = &entry.title {
                    println!("Title: {}", title);
                }
                println!("\n{}", entry.content);
            }
            None => println!("Entry not found"),
        },
        Commands::Search { query } => {
            let entries = journal.search_entries(&query)?;
            if entries.is_empty() {
                println!("No entries found matching '{}'", query);
            } else {
                println!("Found {} entries:", entries.len());
                for entry in entries {
                    println!(
                        "[{}] {} - {}",
                        entry.id,
                        entry.created_at.format("%Y-%m-%d %H:%M"),
                        entry.title.as_deref().unwrap_or("Untitled")
                    );
                }
            }
        }
        Commands::Delete { id } => {
            if journal.delete_entry(id)? {
                println!("Entry {} deleted", id);
            } else {
                println!("Entry {} not found", id);
            }
        }
    }

    Ok(())
}
