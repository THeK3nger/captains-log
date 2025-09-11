pub mod formatting;

use crate::journal::{Entry, Journal};
use anyhow::{Context, Result};
use chrono::{Datelike, Local, NaiveDate};
use clap::Subcommand;
use colored::*;
use formatting::render_markdown;
use std::env;
use std::fs;
use std::process::Command;

#[derive(Subcommand)]
pub enum Commands {
    /// List all entries
    List {
        /// Show entries from specific date (YYYY-MM-DD)
        #[arg(long)]
        date: Option<String>,

        /// Show entries since date (YYYY-MM-DD)
        #[arg(long)]
        since: Option<String>,

        /// Show entries until date (YYYY-MM-DD)
        #[arg(long)]
        until: Option<String>,
    },

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

    /// Edit an existing entry
    Edit {
        /// Entry ID to edit
        id: i64,
    },

    /// Display calendar view of entries
    Calendar {
        /// Year to display (default: current year)
        #[arg(long)]
        year: Option<i32>,

        /// Month to display (1-12, default: current month)
        #[arg(long)]
        month: Option<u32>,
    },
}

pub fn handle_command(command: Commands, journal: &Journal) -> Result<()> {
    match command {
        Commands::List { date, since, until } => {
            let entries = if date.is_some() || since.is_some() || until.is_some() {
                journal.list_entries_filtered(
                    date.as_deref(),
                    since.as_deref(),
                    until.as_deref(),
                )?
            } else {
                journal.list_entries()?
            };

            if entries.is_empty() {
                println!("{}", "No entries found".yellow());
            } else {
                println!(
                    "{}",
                    format!("Found {} entries:", entries.len()).green().bold()
                );
                println!();
                for entry in entries {
                    println!("{}", format_entry_summary(&entry));
                }
            }
        }
        Commands::Show { id } => match journal.get_entry(id)? {
            Some(entry) => {
                print_entry(&entry);
            }
            None => println!("{}", format!("Entry {} not found", id).red()),
        },
        Commands::Search { query } => {
            let entries = journal.search_entries(&query)?;
            if entries.is_empty() {
                println!(
                    "{}",
                    format!("No entries found matching '{}'", query).yellow()
                );
            } else {
                println!(
                    "{}",
                    format!("Found {} entries matching '{}':", entries.len(), query)
                        .green()
                        .bold()
                );
                println!();
                for entry in entries {
                    println!("{}", format_entry_summary(&entry));
                }
            }
        }
        Commands::Delete { id } => {
            if journal.delete_entry(id)? {
                println!("{}", format!("Entry {} deleted", id).green());
            } else {
                println!("{}", format!("Entry {} not found", id).red());
            }
        }
        Commands::Edit { id } => {
            edit_entry(journal, id)?;
        }
        Commands::Calendar { year, month } => {
            show_calendar(journal, year, month)?;
        }
    }

    Ok(())
}

fn print_entry(entry: &Entry) {
    println!("{}", "─".repeat(60).bright_blue());
    println!(
        "{}: {}",
        "ID".cyan().bold(),
        entry.id.to_string().white().bold()
    );
    println!(
        "{}: {}",
        "Date".cyan().bold(),
        entry
            .timestamp
            .format("%Y-%m-%d %H:%M:%S")
            .to_string()
            .white()
    );
    if let Some(title) = &entry.title {
        println!("{}: {}", "Title".cyan().bold(), title.green().bold());
    }
    println!("{}", "─".repeat(60).bright_blue());
    println!();
    println!("{}", render_markdown(&entry.content));
    println!();
    println!("{}", "─".repeat(60).bright_blue());
}

fn format_entry_summary(entry: &Entry) -> String {
    let content_preview = if entry.content.len() > 40 {
        format!("{}...", &entry.content[..40])
    } else {
        entry.content.clone()
    };

    let id = format!("[{}]", entry.id).bright_blue().bold();
    let date = entry.timestamp.format("%Y-%m-%d %H:%M").to_string().white();

    if let Some(title) = &entry.title {
        format!(
            "{} {} - {} - {}",
            id,
            date,
            title.green().bold(),
            content_preview.normal()
        )
    } else {
        format!("{} {} - {}", id, date, content_preview.normal())
    }
}

fn edit_entry(journal: &Journal, id: i64) -> Result<()> {
    // Get the existing entry
    let entry = journal.get_entry(id)?.context("Entry not found")?;

    // Create a temporary file with the current content
    let temp_dir = env::temp_dir();
    let temp_file = temp_dir.join(format!("captains-log-edit-{}.md", id));

    // Write current content to temp file
    let current_content = format!(
        "# {}\n\n{}",
        entry.title.as_deref().unwrap_or(""),
        entry.content
    );
    fs::write(&temp_file, current_content)?;

    // Get editor from environment or use default
    let editor = env::var("EDITOR").unwrap_or_else(|_| "nvim".to_string());

    // Open editor
    let status = Command::new(&editor)
        .arg(&temp_file)
        .status()
        .context("Failed to launch editor")?;

    if !status.success() {
        return Err(anyhow::anyhow!("Editor exited with error"));
    }

    // Read the edited content
    let edited_content = fs::read_to_string(&temp_file)?;
    let lines: Vec<&str> = edited_content.lines().collect();

    // Parse title and content
    let (title, content) = if lines.is_empty() {
        (None, String::new())
    } else if lines.len() == 1 {
        (None, lines[0].to_string())
    } else {
        let title = if lines[0].trim().is_empty() {
            None
        } else {
            let mut title = lines[0].trim();
            // Remove leading '#' if present
            title = title.strip_prefix('#').unwrap_or(title);
            Some(title.trim())
        };
        let content = lines[1..].join("\n").trim().to_string();
        (title, content)
    };

    // Update the entry
    if journal.update_entry(id, title, &content)? {
        println!("{}", format!("Entry {} updated successfully", id).green());
    } else {
        println!("{}", format!("Failed to update entry {}", id).red());
    }

    // Clean up temp file
    let _ = fs::remove_file(&temp_file);

    Ok(())
}

fn show_calendar(journal: &Journal, year: Option<i32>, month: Option<u32>) -> Result<()> {
    let now = Local::now();
    let year = year.unwrap_or(now.year());
    let month = month.unwrap_or(now.month());

    // Validate month
    if !(1..=12).contains(&month) {
        return Err(anyhow::anyhow!("Month must be between 1 and 12"));
    }

    // Get entries for the month
    let entries = journal.list_entries_for_month(year, month)?;

    // Create a map of day -> entry count
    let mut day_counts = std::collections::HashMap::new();
    for entry in &entries {
        let day = entry.timestamp.day();
        *day_counts.entry(day).or_insert(0) += 1;
    }

    // Print calendar header
    let month_names = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    println!();
    println!(
        "{}",
        format!("{} {}", month_names[(month - 1) as usize], year)
            .cyan()
            .bold()
    );
    println!("{}", "─".repeat(21).bright_blue());
    println!("{}", "Mo Tu We Th Fr Sa Su".white().bold());

    // Get first day of month and number of days
    let first_day = NaiveDate::from_ymd_opt(year, month, 1).context("Invalid date")?;
    let first_weekday = first_day.weekday().num_days_from_monday();

    let days_in_month = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    }
    .context("Invalid date")?
    .pred_opt()
    .context("Invalid date")?
    .day();

    // Print calendar
    for _ in 0..first_weekday {
        print!("   ");
    }

    for day in 1..=days_in_month {
        if day_counts.contains_key(&day) {
            print!("{}", format!("{:2}*", day).green().bold());
        } else {
            print!("{:2} ", day);
        }

        let current_weekday = (first_weekday + day - 1) % 7;
        if current_weekday == 6 {
            println!();
        }
    }
    println!();
    println!("{}", "─".repeat(21).bright_blue());

    // Print legend
    println!();
    println!("{} = has entries", "*".green().bold());

    // Show entries for this month
    if !entries.is_empty() {
        println!();
        println!(
            "{}",
            format!("Entries for {}/{:02}:", year, month).cyan().bold()
        );
        println!();
        for entry in entries {
            println!("{}", format_entry_summary(&entry));
        }
    }

    Ok(())
}
