pub mod formatting;
pub mod stardate;

use crate::cli::stardate::Stardate;
use crate::config::Config;
use crate::export::{ExportFilters, Exporter};
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

        /// Filter by journal category
        #[arg(long)]
        journal: Option<String>,
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

    /// Create a new entry using external editor
    New {
        /// Journal category for the new entry
        #[arg(long)]
        journal: Option<String>,
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

    /// Manage configuration
    Config {
        #[command(subcommand)]
        action: Option<ConfigAction>,
    },

    /// Export entries to various formats
    Export {
        /// Output file path
        #[arg(short, long)]
        output: Option<String>,

        /// Export format (currently only json is supported)
        #[arg(short, long, default_value = "json")]
        format: String,

        /// Show entries from specific date (YYYY-MM-DD)
        #[arg(long)]
        date: Option<String>,

        /// Show entries since date (YYYY-MM-DD)
        #[arg(long)]
        since: Option<String>,

        /// Show entries until date (YYYY-MM-DD)
        #[arg(long)]
        until: Option<String>,

        /// Filter by journal category
        #[arg(long)]
        journal: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Show current configuration
    Show,
    /// Set a configuration value
    Set {
        /// Configuration key (e.g., editor.command, database.path)
        key: String,
        /// Configuration value
        value: String,
    },
    /// Show configuration file path
    Path,
}

pub fn handle_command(
    command: Commands,
    journal: &Journal,
    config: &Config,
    global_journal: Option<&str>,
) -> Result<()> {
    match command {
        Commands::List {
            date,
            since,
            until,
            journal: list_journal,
        } => {
            let journal_filter = list_journal.as_deref().or(global_journal);
            let entries =
                if date.is_some() || since.is_some() || until.is_some() || journal_filter.is_some()
                {
                    journal.list_entries_filtered(
                        date.as_deref(),
                        since.as_deref(),
                        until.as_deref(),
                        journal_filter,
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
                print_entry(&entry, config.display.stardate_mode);
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
            edit_entry(journal, id, config)?;
        }
        Commands::New { journal: new_journal } => {
            let journal_category = new_journal.as_deref().or(global_journal);
            new_entry(journal, journal_category, config)?;
        }
        Commands::Calendar { year, month } => {
            show_calendar(journal, year, month)?;
        }
        Commands::Config { action } => {
            handle_config_command(action, config)?;
        }
        Commands::Export {
            output,
            format,
            date,
            since,
            until,
            journal: export_journal,
        } => {
            handle_export_command(
                journal,
                output,
                &format,
                date,
                since,
                until,
                export_journal.or_else(|| global_journal.map(|s| s.to_string())),
            )?;
        }
    }

    Ok(())
}

fn handle_export_command(
    journal: &Journal,
    output_path: Option<String>,
    format: &str,
    date: Option<String>,
    since: Option<String>,
    until: Option<String>,
    journal_filter: Option<String>,
) -> Result<()> {
    let filters = create_export_filters(date, since, until, journal_filter);
    let exporter = Exporter::new(journal);

    match format.to_lowercase().as_str() {
        "json" => {
            export_with_success_message(
                || exporter.export_to_json(output_path.clone(), filters),
                &output_path,
            )?;
        }
        "md" | "markdown" => {
            export_with_success_message(
                || exporter.export_to_markdown(output_path.clone(), filters),
                &output_path,
            )?;
        }
        "org" => {
            export_with_success_message(
                || exporter.export_to_org(output_path.clone(), filters),
                &output_path,
            )?;
        }
        _ => {
            return Err(anyhow::anyhow!(
                "Unsupported export format '{}'. Currently supported formats: json, markdown, org",
                format
            ));
        }
    }

    Ok(())
}

fn create_export_filters(
    date: Option<String>,
    since: Option<String>,
    until: Option<String>,
    journal_filter: Option<String>,
) -> Option<ExportFilters> {
    if date.is_some() || since.is_some() || until.is_some() || journal_filter.is_some() {
        Some(ExportFilters {
            date,
            since,
            until,
            journal: journal_filter,
        })
    } else {
        None
    }
}

fn export_with_success_message<F>(export_fn: F, output_path: &Option<String>) -> Result<()>
where
    F: FnOnce() -> Result<()>,
{
    export_fn()?;

    if let Some(path) = output_path {
        println!(
            "{}",
            format!("Entries exported successfully to {}", path).green()
        );
    } else {
        println!("{}", "Entries exported successfully to stdout".green());
    }

    Ok(())
}

fn handle_config_command(action: Option<ConfigAction>, config: &Config) -> Result<()> {
    match action {
        Some(ConfigAction::Show) | None => {
            println!("{}", "Current Configuration:".cyan().bold());
            println!("{}", "─".repeat(40).bright_blue());

            println!();
            println!("{}", "Database:".yellow().bold());
            if let Some(path) = &config.database.path {
                println!("  path: {}", path.green());
            } else {
                println!("  path: {} (default)", "auto".bright_black());
            }

            println!();
            println!("{}", "Editor:".yellow().bold());
            if let Some(command) = &config.editor.command {
                println!("  command: {}", command.green());
            } else {
                println!(
                    "  command: {} (from $EDITOR or default)",
                    "auto".bright_black()
                );
            }

            println!();
            println!("{}", "Display:".yellow().bold());
            println!(
                "  colors_enabled: {}",
                config.display.colors_enabled.to_string().green()
            );
            println!("  date_format: {}", config.display.date_format.green());
            if let Some(entries_per_page) = config.display.entries_per_page {
                println!(
                    "  entries_per_page: {}",
                    entries_per_page.to_string().green()
                );
            } else {
                println!("  entries_per_page: {} (no limit)", "auto".bright_black());
            }
        }
        Some(ConfigAction::Set { key, value }) => {
            let mut new_config = config.clone();

            match key.as_str() {
                "database.path" => {
                    new_config.database.path = Some(value.clone());
                    println!("{}", format!("Set database.path to '{}'", value).green());
                }
                "editor.command" => {
                    new_config.editor.command = Some(value.clone());
                    println!("{}", format!("Set editor.command to '{}'", value).green());
                }
                "display.colors_enabled" => {
                    let enabled: bool = value
                        .parse()
                        .context("display.colors_enabled must be 'true' or 'false'")?;
                    new_config.display.colors_enabled = enabled;
                    println!(
                        "{}",
                        format!("Set display.colors_enabled to {}", enabled).green()
                    );
                }
                "display.date_format" => {
                    new_config.display.date_format = value.clone();
                    println!(
                        "{}",
                        format!("Set display.date_format to '{}'", value).green()
                    );
                }
                "display.entries_per_page" => {
                    if value == "auto" || value == "none" {
                        new_config.display.entries_per_page = None;
                        println!(
                            "{}",
                            "Set display.entries_per_page to auto (no limit)".green()
                        );
                    } else {
                        let per_page: usize = value
                            .parse()
                            .context("display.entries_per_page must be a number or 'auto'")?;
                        new_config.display.entries_per_page = Some(per_page);
                        println!(
                            "{}",
                            format!("Set display.entries_per_page to {}", per_page).green()
                        );
                    }
                }
                _ => {
                    return Err(anyhow::anyhow!(
                        "Unknown configuration key '{}'. Available keys: database.path, editor.command, display.colors_enabled, display.date_format, display.entries_per_page",
                        key
                    ));
                }
            }

            new_config.save()?;
            println!(
                "{}",
                "Configuration saved successfully".bright_green().bold()
            );
        }
        Some(ConfigAction::Path) => {
            let config_path = Config::get_config_path()?;
            println!("{}", config_path.display());
        }
    }

    Ok(())
}

fn print_entry(entry: &Entry, stardate_mode: bool) {
    println!("{}", "─".repeat(60).bright_blue());
    println!(
        "{}: {}",
        "ID".cyan().bold(),
        entry.id.to_string().white().bold()
    );
    if stardate_mode {
        let stardate = entry.timestamp.to_stardate();
        let stardate_string = format!("{:.5}", stardate);

        // Split into head and last two characters safely
        let chars: Vec<char> = stardate_string.chars().collect();
        let (head, tail) = if chars.len() >= 2 {
            let head: String = chars[..chars.len() - 2].iter().collect();
            let tail: String = chars[chars.len() - 2..].iter().collect();
            (head, tail)
        } else {
            (stardate_string.clone(), String::new())
        };

        println!(
            "{}: {}{}",
            "Stardate".cyan().bold(),
            head.white(),
            tail.bright_black(),
        );
    } else {
        println!(
            "{}: {}",
            "Date".cyan().bold(),
            entry
                .timestamp
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
                .white()
        );
    }
    println!(
        "{}: {}",
        "Journal".cyan().bold(),
        entry.journal.magenta().bold()
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
    // Strip newlines and limit content preview to 40 chars.
    let content_preview = if entry.content.len() > 40 {
        format!("{}...", &entry.content[..40].replace('\n', " "))
    } else {
        entry.content.replace('\n', " ")
    };

    let id = format!("[{}]", entry.id).bright_blue().bold();
    let date = entry.timestamp.format("%Y-%m-%d %H:%M").to_string().white();
    let journal = format!("[{}]", entry.journal).magenta().bold();

    if let Some(title) = &entry.title {
        format!(
            "{} {} {} - {} - {}",
            id,
            date,
            journal,
            title.green().bold(),
            content_preview.normal()
        )
    } else {
        format!("{} {} {} - {}", id, date, journal, content_preview.normal())
    }
}

fn new_entry(journal: &Journal, journal_category: Option<&str>, config: &Config) -> Result<()> {
    // Create a temporary file for the new entry
    let temp_dir = env::temp_dir();
    let temp_file = temp_dir.join("captains-log-new.md");

    // Write template content to temp file
    let template_content = "# \n\n";
    fs::write(&temp_file, template_content)?;

    // Get editor from config
    let editor = config.get_editor_command();

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
    } else {
        let first_line = lines[0].trim();
        if first_line.is_empty() && lines.len() == 1 {
            // If the only line is empty, treat as empty content
            (None, String::new())
        } else if first_line.starts_with("# ") && lines.len() == 1 {
            // If only title is present
            (
                Some(first_line.strip_prefix("# ").unwrap().trim()),
                String::new(),
            )
        } else if first_line.starts_with("# ") && lines.len() > 1 {
            // If title and content are present
            let title = first_line.strip_prefix("# ").unwrap().trim();
            let content = lines[1..].join("\n").trim().to_string();
            (Some(title), content)
        } else {
            // No title, all content
            (None, edited_content.trim().to_string())
        }
    };

    // Check if the content is empty
    if content.is_empty() && (title.is_none() || title.as_ref().unwrap().is_empty()) {
        println!("{}", "Entry creation cancelled - no content provided".yellow());
        // Clean up temp file
        let _ = fs::remove_file(&temp_file);
        return Ok(());
    }

    // Create the entry
    let id = journal.create_entry(title, &content, journal_category)?;
    println!("{}", format!("Entry {} created successfully", id).green());

    // Clean up temp file
    let _ = fs::remove_file(&temp_file);

    Ok(())
}

fn edit_entry(journal: &Journal, id: i64, config: &Config) -> Result<()> {
    // Get the existing entry
    let entry = journal.get_entry(id)?.context("Entry not found")?;

    // Create a temporary file with the current content
    let temp_dir = env::temp_dir();
    let temp_file = temp_dir.join(format!("captains-log-edit-{}.md", id));

    // Write current content to temp file
    let current_content = if entry.title.is_some() {
        format!(
            "# {}\n\n{}",
            entry.title.as_deref().unwrap_or(""),
            entry.content
        )
    } else {
        entry.content.clone()
    };
    fs::write(&temp_file, current_content)?;

    // Get editor from config
    let editor = config.get_editor_command();

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
    } else {
        let first_line = lines[0].trim();
        if first_line.is_empty() && lines.len() == 1 {
            // If the only line is empty, treat as empty content
            // This should actually be an error.
            (None, String::new())
        } else if first_line.starts_with("# ") && lines.len() == 1 {
            // If only title is present
            (
                Some(first_line.strip_prefix("# ").unwrap().trim()),
                String::new(),
            )
        } else if first_line.starts_with("# ") && lines.len() > 1 {
            // If title and content are present
            let title = first_line.strip_prefix("# ").unwrap().trim();
            let content = lines[1..].join("\n").trim().to_string();
            (Some(title), content)
        } else {
            // No title, all content
            (None, edited_content.trim().to_string())
        }
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
