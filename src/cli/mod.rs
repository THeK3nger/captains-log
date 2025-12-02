pub mod dateparser;
pub mod formatting;
pub mod frontmatter;
pub mod stardate;

use crate::cli::formatting::{get_wrap_width, wrap_text};
use crate::cli::frontmatter::{format_entry_with_frontmatter, parse_frontmatter};
use crate::cli::stardate::Stardate;
use crate::config::Config;
use crate::export::{ExportFilters, Exporter};
use crate::import::Importer;
use crate::journal::{Entry, Journal};
use anyhow::{Context, Result};
use chrono::{Datelike, Local, NaiveDate};
use clap::Subcommand;
use colored::*;
use dateparser::parse_relative_date;
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

    /// Move an entry to a different journal
    Move {
        /// Entry ID to move
        id: i64,
        /// Target journal name
        journal: String,
    },

    /// Edit an existing entry
    Edit {
        /// Entry ID to edit
        id: i64,
    },

    /// Create a new entry
    New {
        /// Journal category for the new entry
        #[arg(long)]
        journal: Option<String>,

        /// Quick entry content (if provided, creates entry directly without opening editor)
        content: Vec<String>,
    },

    /// Display calendar view of entries
    Calendar {
        /// Year to display (default: current year)
        #[arg(long)]
        year: Option<i32>,

        /// Month to display (1-12, default: current month)
        #[arg(long)]
        month: Option<u32>,

        /// Filter by journal category
        #[arg(long)]
        journal: Option<String>,
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

    /// Import entries from various formats
    Import {
        /// Path to file to import
        path: String,

        /// Import format (supported formats: org, dayone)
        #[arg(short, long, default_value = "org")]
        format: String,

        /// Filter by specific date (YYYY-MM-DD) - only import entries from this date
        #[arg(long)]
        date: Option<String>,

        /// Target journal category for imported entries
        #[arg(long)]
        journal: Option<String>,
    },

    /// Record audio and create a new journal entry with transcription
    Record {
        /// Journal category for the new entry
        #[arg(long)]
        journal: Option<String>,

        /// Skip transcription (audio only)
        #[arg(long)]
        no_transcribe: bool,

        /// Maximum recording duration in seconds (default: 600)
        #[arg(long)]
        max_duration: Option<u64>,
    },

    /// Play audio from an existing entry
    Play {
        /// Entry ID to play audio from
        id: i64,
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
    db_path: &std::path::Path,
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

            // Parse date filters using .map().transpose() pattern
            let date_filter = date
                .as_deref()
                .map(parse_relative_date)
                .transpose()
                .map_err(|e| anyhow::anyhow!("Invalid date: {}", e))?;
            let since_filter = since
                .as_deref()
                .map(parse_relative_date)
                .transpose()
                .map_err(|e| anyhow::anyhow!("Invalid since date: {}", e))?;
            let until_filter = until
                .as_deref()
                .map(parse_relative_date)
                .transpose()
                .map_err(|e| anyhow::anyhow!("Invalid until date: {}", e))?;

            let entries = if date_filter.is_some()
                || since_filter.is_some()
                || until_filter.is_some()
                || journal_filter.is_some()
            {
                journal.list_entries_filtered(
                    date_filter.as_ref(),
                    since_filter.as_ref(),
                    until_filter.as_ref(),
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
                    println!(
                        "{}",
                        format_entry_summary(&entry, config.display.stardate_mode)
                    );
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
                    println!(
                        "{}",
                        format_entry_summary(&entry, config.display.stardate_mode)
                    );
                }
            }
        }
        Commands::Delete { id } => {
            match journal.get_entry(id)? {
                Some(entry) => {
                    // Show the entry to be deleted
                    println!("{}", "Entry to be deleted:".yellow().bold());
                    println!();
                    print_entry(&entry, config.display.stardate_mode);
                    println!();

                    // Ask for confirmation
                    print!(
                        "{}",
                        "Are you sure you want to delete this entry? (y/N): "
                            .red()
                            .bold()
                    );
                    std::io::Write::flush(&mut std::io::stdout())?;

                    let mut input = String::new();
                    std::io::stdin().read_line(&mut input)?;
                    let input = input.trim().to_lowercase();

                    if input == "y" || input == "yes" {
                        if journal.delete_entry(id)? {
                            println!("{}", format!("Entry {} deleted", id).green());
                        } else {
                            println!("{}", format!("Failed to delete entry {}", id).red());
                        }
                    } else {
                        println!("{}", "Deletion cancelled".yellow());
                    }
                }
                None => {
                    println!("{}", format!("Entry {} not found", id).red());
                }
            }
        }
        Commands::Move {
            id,
            journal: target_journal,
        } => match journal.get_entry(id)? {
            Some(entry) => {
                let old_journal = &entry.journal;
                if journal.move_entry(id, &target_journal)? {
                    println!(
                        "{}",
                        format!(
                            "Entry {} moved from '{}' to '{}'",
                            id, old_journal, target_journal
                        )
                        .green()
                    );
                } else {
                    println!("{}", format!("Failed to move entry {}", id).red());
                }
            }
            None => {
                println!("{}", format!("Entry {} not found", id).red());
            }
        },
        Commands::Edit { id } => {
            edit_entry(journal, id, config)?;
        }
        Commands::New {
            journal: new_journal,
            content,
        } => {
            let journal_category = new_journal.as_deref().or(global_journal);
            if content.is_empty() {
                // No content provided - open editor
                new_entry(journal, journal_category, config)?;
            } else {
                // Content provided - create entry directly
                let entry_content = content.join(" ");
                let id = journal.create_entry(None, &entry_content, journal_category)?;
                println!("{}", format!("Entry {} added successfully", id).green());
            }
        }
        Commands::Calendar {
            year,
            month,
            journal: calendar_journal,
        } => {
            let journal_filter = calendar_journal.as_deref().or(global_journal);
            show_calendar(journal, year, month, journal_filter, config)?;
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
                export_journal.or_else(|| global_journal.map(str::to_string)),
            )?;
        }
        Commands::Import {
            path,
            format,
            date,
            journal: import_journal,
        } => {
            handle_import_command(
                journal,
                &path,
                &format,
                date,
                import_journal.or_else(|| global_journal.map(str::to_string)),
            )?;
        }

        Commands::Record {
            journal: record_journal,
            no_transcribe,
            max_duration,
        } => {
            handle_record_command(
                journal,
                config,
                db_path,
                record_journal.or_else(|| global_journal.map(str::to_string)),
                no_transcribe,
                max_duration,
            )?;
        }

        Commands::Play { id } => {
            handle_play_command(journal, config, db_path, id)?;
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
            exporter.export_to_json(output_path.clone(), filters)?;
        }
        "md" | "markdown" => {
            exporter.export_to_markdown(output_path.clone(), filters)?;
        }
        "org" => {
            exporter.export_to_org(output_path.clone(), filters)?;
        }
        _ => {
            return Err(anyhow::anyhow!(
                "Unsupported export format '{}'. Currently supported formats: json, markdown, org",
                format
            ));
        }
    }

    // Print success message
    if let Some(path) = &output_path {
        println!(
            "{}",
            format!("Entries exported successfully to {}", path).green()
        );
    } else {
        println!("{}", "Entries exported successfully to stdout".green());
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

fn handle_import_command(
    journal: &Journal,
    file_path: &str,
    format: &str,
    date: Option<String>,
    journal_category: Option<String>,
) -> Result<()> {
    // Parse date filter if provided
    let filter_date = date
        .as_deref()
        .map(parse_relative_date)
        .transpose()
        .map_err(|e| anyhow::anyhow!("Invalid date filter: {}", e))?;

    let importer = Importer::new(journal);

    match format.to_lowercase().as_str() {
        "org" => {
            println!(
                "{} {}",
                "[EXPERIMENTAL]".yellow(),
                "org-journal import is still very VERY experimental."
            );
            println!("{}", format!("Importing from {}...", file_path).cyan());

            let stats =
                importer.import_from_org(file_path, journal_category.as_deref(), filter_date)?;

            // Display results
            println!();
            println!("{}", "Import completed!".green().bold());
            println!("  Total entries found: {}", stats.total);
            println!(
                "  Successfully imported: {}",
                stats.imported.to_string().green()
            );

            if stats.skipped > 0 {
                println!("  Skipped: {}", stats.skipped.to_string().yellow());
            }

            if !stats.errors.is_empty() {
                println!();
                println!("{}", "Errors encountered:".red().bold());
                for error in &stats.errors {
                    println!("  - {}", error.red());
                }
            }
        }
        "dayone" => {
            println!(
                "{} {}",
                "[EXPERIMENTAL]".yellow(),
                "DayOne JSON import is still experimental."
            );
            println!("{}", format!("Importing from {}...", file_path).cyan());

            let stats =
                importer.import_from_dayone(file_path, journal_category.as_deref(), filter_date)?;

            // Display results
            println!();
            println!("{}", "Import completed!".green().bold());
            println!("  Total entries found: {}", stats.total);
            println!(
                "  Successfully imported: {}",
                stats.imported.to_string().green()
            );

            if stats.skipped > 0 {
                println!("  Skipped: {}", stats.skipped.to_string().yellow());
            }

            if !stats.errors.is_empty() {
                println!();
                println!("{}", "Errors encountered:".red().bold());
                for error in &stats.errors {
                    println!("  - {}", error.red());
                }
            }
        }
        _ => {
            return Err(anyhow::anyhow!(
                "Unsupported import format '{}'. Currently supported formats: org, dayone",
                format
            ));
        }
    }

    Ok(())
}

fn handle_record_command(
    journal_obj: &Journal,
    config: &Config,
    db_path: &std::path::Path,
    journal_category: Option<String>,
    no_transcribe: bool,
    max_duration: Option<u64>,
) -> Result<()> {
    use crate::audio::{
        ensure_audio_directory_exists, generate_audio_filename, get_audio_directory, record_audio,
        transcribe_audio,
    };

    // Ensure audio directory exists
    ensure_audio_directory_exists(db_path)?;

    // Generate filename
    let audio_dir = get_audio_directory(db_path)?;
    let filename = generate_audio_filename();
    let full_path = audio_dir.join(&filename);

    // Get max duration from config or parameter
    let max_duration_secs = max_duration.unwrap_or(config.audio.max_recording_seconds);

    // Record audio
    let duration = record_audio(config, &full_path, max_duration_secs)?;

    // Transcribe audio (unless skipped)
    let transcription = if no_transcribe {
        println!("{}", "Skipping transcription (--no-transcribe flag)".yellow());
        "[Audio entry - no transcription]".to_string()
    } else {
        match transcribe_audio(config, &full_path) {
            Ok(text) => {
                // Display transcription to user
                println!();
                println!("{}", "─── Transcription ───".cyan().bold());
                println!("{}", text);
                println!("{}", "─────────────────────".cyan().bold());
                println!();
                text
            }
            Err(e) => {
                println!("{}", format!("Warning: Transcription failed: {}", e).yellow());
                println!("{}", "Saving entry with audio only...".yellow());
                "[Transcription failed - audio only]".to_string()
            }
        }
    };

    println!("{}", "📝 Creating journal entry...".cyan());

    // Create entry with audio
    // Store relative path: audio/filename.wav
    let relative_path = format!("audio/{}", filename);

    let entry_id = journal_obj.create_entry_with_audio(
        None, // No title
        &transcription,
        journal_category.as_deref(),
        Some(&relative_path),
    )?;

    println!(
        "{}",
        format!("✓ Entry {} created successfully with audio attached", entry_id).green()
    );
    println!("  {}: {}", "Duration".cyan(), format!("{:.1}s", duration.as_secs_f64()));
    println!("  {}: {}", "Audio".cyan(), relative_path.green());

    Ok(())
}

fn handle_play_command(journal_obj: &Journal, config: &Config, db_path: &std::path::Path, id: i64) -> Result<()> {
    use crate::audio::{get_audio_full_path, play_audio};
    use colored::Colorize;

    // Get entry
    let entry = journal_obj
        .get_entry(id)?
        .ok_or_else(|| anyhow::anyhow!("Entry {} not found", id))?;

    // Check if entry has audio
    let audio_path = entry
        .audio_path
        .ok_or_else(|| anyhow::anyhow!("Entry {} has no audio recording", id))?;

    // Get full path
    let full_path = get_audio_full_path(db_path, &audio_path)?;

    // Check if file exists
    if !full_path.exists() {
        return Err(anyhow::anyhow!(
            "Audio file not found at {}. It may have been moved or deleted.",
            full_path.display()
        ));
    }

    println!("{}", format!("🎵 Playing audio from entry {}...", id).cyan());
    println!("  {}: {}", "Audio".cyan(), audio_path.green());

    // Play audio
    play_audio(config, &full_path)?;

    println!("{}", "✓ Playback complete".green());

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
            println!(
                "  stardate_mode: {}",
                config.display.stardate_mode.to_string().green()
            );
            if let Some(entries_per_page) = config.display.entries_per_page {
                println!(
                    "  entries_per_page: {}",
                    entries_per_page.to_string().green()
                );
            } else {
                println!("  entries_per_page: {} (no limit)", "auto".bright_black());
            }

            println!();
            println!("{}", "Audio:".yellow().bold());
            if let Some(whisper_command) = &config.audio.whisper_command {
                println!("  whisper_command: {}", whisper_command.green());
            } else {
                println!("  whisper_command: {} (auto-detect)", "auto".bright_black());
            }
            println!("  whisper_model: {}", config.audio.whisper_model.green());
            if let Some(recording_tool) = &config.audio.recording_tool {
                println!("  recording_tool: {}", recording_tool.green());
            } else {
                println!("  recording_tool: {} (auto-detect)", "auto".bright_black());
            }
            if let Some(playback_tool) = &config.audio.playback_tool {
                println!("  playback_tool: {}", playback_tool.green());
            } else {
                println!("  playback_tool: {} (auto-detect)", "auto".bright_black());
            }
            println!(
                "  max_recording_seconds: {}",
                config.audio.max_recording_seconds.to_string().green()
            );
            println!(
                "  sample_rate: {}",
                config.audio.sample_rate.to_string().green()
            );
        }
        Some(ConfigAction::Set { key, value }) => {
            let mut new_config = config.clone();

            match key.as_str() {
                "database.path" => {
                    new_config.database.path = Some(value);
                    println!("{}", format!("Set database.path to '{}'", new_config.database.path.as_ref().unwrap()).green());
                }
                "editor.command" => {
                    new_config.editor.command = Some(value);
                    println!("{}", format!("Set editor.command to '{}'", new_config.editor.command.as_ref().unwrap()).green());
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
                    new_config.display.date_format = value;
                    println!(
                        "{}",
                        format!("Set display.date_format to '{}'", new_config.display.date_format).green()
                    );
                }
                "display.stardate_mode" => {
                    let enabled: bool = value
                        .parse()
                        .context("display.stardate_mode must be 'true' or 'false'")?;
                    new_config.display.stardate_mode = enabled;
                    println!(
                        "{}",
                        format!("Set display.stardate_mode to {}", enabled).green()
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
                "audio.whisper_command" => {
                    new_config.audio.whisper_command = Some(value.clone());
                    println!(
                        "{}",
                        format!("Set audio.whisper_command to '{}'", value).green()
                    );
                }
                "audio.whisper_model" => {
                    new_config.audio.whisper_model = value.clone();
                    println!(
                        "{}",
                        format!("Set audio.whisper_model to '{}'", value).green()
                    );
                }
                "audio.recording_tool" => {
                    new_config.audio.recording_tool = Some(value.clone());
                    println!(
                        "{}",
                        format!("Set audio.recording_tool to '{}'", value).green()
                    );
                }
                "audio.playback_tool" => {
                    new_config.audio.playback_tool = Some(value.clone());
                    println!(
                        "{}",
                        format!("Set audio.playback_tool to '{}'", value).green()
                    );
                }
                "audio.max_recording_seconds" => {
                    let seconds: u64 = value
                        .parse()
                        .context("audio.max_recording_seconds must be a number")?;
                    new_config.audio.max_recording_seconds = seconds;
                    println!(
                        "{}",
                        format!("Set audio.max_recording_seconds to {}", seconds).green()
                    );
                }
                "audio.sample_rate" => {
                    let rate: u32 = value
                        .parse()
                        .context("audio.sample_rate must be a number")?;
                    new_config.audio.sample_rate = rate;
                    println!(
                        "{}",
                        format!("Set audio.sample_rate to {}", rate).green()
                    );
                }
                _ => {
                    return Err(anyhow::anyhow!(
                        "Unknown configuration key '{}'. Available keys: database.path, editor.command, display.colors_enabled, display.date_format, display.stardate_mode, display.entries_per_page, audio.whisper_command, audio.whisper_model, audio.recording_tool, audio.playback_tool, audio.max_recording_seconds, audio.sample_rate",
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
    let width = get_wrap_width();
    println!("{}", "─".repeat(width as usize).bright_blue());
    println!(
        "{}: {}",
        "ID".cyan().bold(),
        entry.id.to_string().white().bold()
    );
    if stardate_mode {
        let stardate = entry.timestamp.to_stardate();

        let stardate_string = format_stardate(stardate);

        println!("{}: {}", "Stardate".cyan().bold(), stardate_string);
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

    // Display audio info if available
    if let Some(audio_path) = &entry.audio_path {
        println!("{}: {}", "Audio".cyan().bold(), audio_path.green());
    }

    let content = render_markdown(&entry.content);
    let wrapped_content = wrap_text(&content, width);

    println!("{}", "─".repeat(width as usize).bright_blue());
    println!();
    println!("{}", wrapped_content);
    println!();
    println!("{}", "─".repeat(width as usize).bright_blue());
}

fn format_entry_summary(entry: &Entry, stardate_mode: bool) -> String {
    // Strip newlines and limit content preview to 40 chars.
    let content_preview = if entry.content.len() > 40 {
        format!("{}...", &entry.content[..40].replace('\n', " "))
    } else {
        entry.content.replace('\n', " ")
    };

    let id = format!("[{}]", entry.id).bright_blue().bold();

    let date = if stardate_mode {
        let stardate = entry.timestamp.to_stardate();
        format_stardate(stardate)
    } else {
        entry
            .timestamp
            .format("%Y-%m-%d %H:%M")
            .to_string()
            .white()
            .to_string()
    };

    let journal = format!("[{}]", entry.journal).magenta().bold();

    // Add audio indicator if entry has audio
    let audio_indicator = if entry.audio_path.is_some() {
        " 🎤"
    } else {
        ""
    };

    if let Some(title) = &entry.title {
        format!(
            "{} {} {} - {} - {}{}",
            id,
            date,
            journal,
            title.green().bold(),
            content_preview.normal(),
            audio_indicator
        )
    } else {
        format!("{} {} {} - {}{}", id, date, journal, content_preview.normal(), audio_indicator)
    }
}

fn format_stardate(stardate: f64) -> String {
    let stardate_string = format!("{:.5}", stardate);

    // Split into head and last two characters safely
    let chars: Vec<char> = stardate_string.chars().collect();
    let (head, tail) = if chars.len() >= 2 {
        let head: String = chars[..chars.len() - 2].iter().collect();
        let tail: String = chars[chars.len() - 2..].iter().collect();
        (head, tail)
    } else {
        (stardate_string, String::new())
    };

    format!("{}{}", head.white(), tail.bright_black())
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
        println!(
            "{}",
            "Entry creation cancelled - no content provided".yellow()
        );
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

    // Format content with title if present
    let body_content = if let Some(title) = &entry.title {
        format!("# {}\n\n{}", title, entry.content)
    } else {
        entry.content.clone()
    };

    // Write current content with YAML frontmatter to temp file
    let content_with_frontmatter =
        format_entry_with_frontmatter(&entry.journal, entry.timestamp, &body_content)?;
    fs::write(&temp_file, content_with_frontmatter)?;

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

    // Parse frontmatter and content
    let (metadata, body) = parse_frontmatter(&edited_content).context(
        "Failed to parse entry. Make sure the YAML frontmatter is properly formatted with '---' delimiters",
    )?;

    let lines: Vec<&str> = body.lines().collect();

    // Parse title and content from the body
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
            (None, body.trim().to_string())
        }
    };

    // Update the entry with metadata
    if journal.update_entry_with_metadata(
        id,
        title,
        &content,
        &metadata.journal,
        metadata.timestamp,
    )? {
        println!("{}", format!("Entry {} updated successfully", id).green());
    } else {
        println!("{}", format!("Failed to update entry {}", id).red());
    }

    // Clean up temp file
    let _ = fs::remove_file(&temp_file);

    Ok(())
}

fn show_calendar(
    journal: &Journal,
    year: Option<i32>,
    month: Option<u32>,
    journal_filter: Option<&str>,
    config: &Config,
) -> Result<()> {
    let now = Local::now();
    let year = year.unwrap_or(now.year());
    let month = month.unwrap_or(now.month());

    // Validate month
    if !(1..=12).contains(&month) {
        return Err(anyhow::anyhow!("Month must be between 1 and 12"));
    }

    // Get entries for the month
    let entries = journal.list_entries_for_month_filtered(year, month, journal_filter)?;

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
            println!(
                "{}",
                format_entry_summary(&entry, config.display.stardate_mode)
            );
        }
    }

    Ok(())
}
