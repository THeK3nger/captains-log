use crate::journal::{Journal, JournalInfo};
use anyhow::Result;
use clap::Subcommand;
use colored::*;

#[derive(Subcommand)]
pub enum JournalsAction {
    /// List journals with their entry counts, descriptions and colors (default)
    List,

    /// Create an empty journal (journals are also created by writing an entry in them)
    Add {
        /// Journal name
        name: String,
        /// What the journal is for
        #[arg(long)]
        description: Option<String>,
        /// Color: #rrggbb, #rgb, or black/red/green/yellow/blue/magenta/cyan/white
        #[arg(long)]
        color: Option<String>,
    },

    /// Change a journal's description or color (pass "" to clear one)
    Set {
        /// Journal name
        name: String,
        /// What the journal is for
        #[arg(long)]
        description: Option<String>,
        /// Color: #rrggbb, #rgb, or black/red/green/yellow/blue/magenta/cyan/white
        #[arg(long)]
        color: Option<String>,
    },

    /// Rename a journal, keeping its entries
    Rename {
        /// Current journal name
        old: String,
        /// New journal name
        new: String,
    },

    /// Delete an empty journal
    Delete {
        /// Journal name
        name: String,
    },
}

pub fn handle_journals_command(journal: &Journal, action: Option<JournalsAction>) -> Result<()> {
    match action.unwrap_or(JournalsAction::List) {
        JournalsAction::List => list_journals(journal),
        JournalsAction::Add {
            name,
            description,
            color,
        } => {
            journal.create_journal(&name, description.as_deref(), color.as_deref())?;
            println!("{}", format!("Journal '{}' created", name.trim()).green());
            Ok(())
        }
        JournalsAction::Set {
            name,
            description,
            color,
        } => {
            if description.is_none() && color.is_none() {
                anyhow::bail!("Nothing to change: pass --description and/or --color");
            }
            if !journal.update_journal(&name, description.as_deref(), color.as_deref())? {
                anyhow::bail!("Journal '{}' not found", name);
            }
            println!("{}", format!("Journal '{}' updated", name.trim()).green());
            Ok(())
        }
        JournalsAction::Rename { old, new } => {
            if !journal.rename_journal(&old, &new)? {
                anyhow::bail!("Journal '{}' not found", old);
            }
            println!(
                "{}",
                format!("Renamed journal '{}' to '{}'", old.trim(), new.trim()).green()
            );
            Ok(())
        }
        JournalsAction::Delete { name } => {
            if !journal.delete_journal(&name)? {
                anyhow::bail!("Journal '{}' not found", name);
            }
            println!("{}", format!("Journal '{}' deleted", name.trim()).green());
            Ok(())
        }
    }
}

/// Terminal color for a stored journal color (a name or `#rrggbb`).
fn terminal_color(color: &str) -> Option<Color> {
    match color.strip_prefix('#') {
        Some(hex) if hex.len() == 6 => {
            let channel = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
            Some(Color::TrueColor {
                r: channel(0)?,
                g: channel(2)?,
                b: channel(4)?,
            })
        }
        Some(_) => None,
        None => color.parse().ok(),
    }
}

fn list_journals(journal: &Journal) -> Result<()> {
    let journals = journal.list_journals()?;
    if journals.is_empty() {
        println!("{}", "No journals yet".yellow());
        return Ok(());
    }

    let width = journals
        .iter()
        .map(|j| j.name.chars().count())
        .max()
        .unwrap_or(0);
    for JournalInfo {
        name,
        description,
        color,
        entry_count,
    } in journals
    {
        // Pad before coloring: escape codes would count towards the width
        let padded = format!("{name:<width$}");
        let label = match color.as_deref().and_then(terminal_color) {
            Some(c) => padded.color(c).bold(),
            None => padded.bold(),
        };
        let count = format!(
            "{entry_count:>4} {}",
            if entry_count == 1 {
                "entry  "
            } else {
                "entries"
            }
        );
        print!("{label}  {count}");
        if let Some(description) = description {
            print!("  {}", description.dimmed());
        }
        if let Some(color) = color {
            print!("  {}", format!("[{color}]").dimmed());
        }
        println!();
    }
    Ok(())
}
