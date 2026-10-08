pub mod commands;
pub mod dateparser;
pub mod formatting;
pub mod frontmatter;
pub mod stardate;

use crate::config::Config;
use crate::journal::Journal;
use anyhow::Result;
use clap::Subcommand;
use commands::audio::{handle_play_command, handle_record_command};
use commands::config::{ConfigAction, handle_config_command};
use commands::edit::{create_entry, delete_entry, edit_entry, move_entry};
use commands::import_export::{ExportArgs, ImportArgs, export, import};
use commands::list::{ListArgs, list_entries, search_entries, show_calendar, show_entry};

#[derive(Subcommand)]
pub enum Commands {
    /// List all entries
    List(ListArgs),

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

        /// Don't ask for confirmation (also deletes the attached audio file)
        #[arg(short, long)]
        yes: bool,

        /// Keep the attached audio file instead of deleting it
        #[arg(long)]
        keep_audio: bool,
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
    Export(ExportArgs),

    /// Import entries from various formats
    Import(ImportArgs),

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

    /// Start the LCARS web interface (read-only)
    Serve {
        /// Port to listen on
        #[arg(short, long, default_value_t = 4343)]
        port: u16,
    },
}

pub fn handle_command(
    command: Commands,
    journal: &Journal,
    config: &Config,
    db_path: &std::path::Path,
    global_journal: Option<&str>,
) -> Result<()> {
    match command {
        Commands::List(args) => list_entries(journal, config, args, global_journal),
        Commands::Show { id } => show_entry(journal, config, id),
        Commands::Search { query } => search_entries(journal, config, &query),
        Commands::Delete {
            id,
            yes,
            keep_audio,
        } => delete_entry(journal, config, db_path, id, yes, keep_audio),
        Commands::Move {
            id,
            journal: target_journal,
        } => move_entry(journal, id, &target_journal),
        Commands::Edit { id } => edit_entry(journal, id, config),
        Commands::New {
            journal: new_journal,
            content,
        } => create_entry(
            journal,
            config,
            content,
            new_journal.as_deref().or(global_journal),
        ),
        Commands::Calendar {
            year,
            month,
            journal: calendar_journal,
        } => show_calendar(
            journal,
            year,
            month,
            calendar_journal.as_deref().or(global_journal),
            config,
        ),
        Commands::Config { action } => handle_config_command(action, config),
        Commands::Export(args) => export(journal, config, args, global_journal),
        Commands::Import(args) => import(journal, config, args, global_journal),
        Commands::Record {
            journal: record_journal,
            no_transcribe,
            max_duration,
        } => handle_record_command(
            journal,
            config,
            db_path,
            record_journal.or_else(|| global_journal.map(str::to_string)),
            no_transcribe,
            max_duration,
        ),
        Commands::Play { id } => handle_play_command(journal, config, db_path, id),
        Commands::Serve { .. } => {
            // handled in main.rs before handle_command is called
            Ok(())
        }
    }
}
