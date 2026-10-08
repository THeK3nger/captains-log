use crate::cli::dateparser::parse_relative_date;
use crate::config::Config;
use crate::export::{ExportFilters, Exporter};
use crate::import::{ImportStats, Importer};
use crate::journal::Journal;
use anyhow::Result;
use chrono_tz::Tz;
use clap::{Args, ValueEnum};
use colored::*;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ExportFormat {
    Json,
    #[value(alias = "md")]
    Markdown,
    Org,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ImportFormat {
    /// org-journal files
    Org,
    /// DayOne JSON export
    Dayone,
}

#[derive(Args)]
pub struct ExportArgs {
    /// Output file path
    #[arg(short, long)]
    pub output: Option<String>,

    /// Export format
    #[arg(short, long, value_enum, ignore_case = true, default_value_t = ExportFormat::Json)]
    pub format: ExportFormat,

    /// Show entries from specific date (YYYY-MM-DD)
    #[arg(long)]
    pub date: Option<String>,

    /// Show entries since date (YYYY-MM-DD)
    #[arg(long)]
    pub since: Option<String>,

    /// Show entries until date (YYYY-MM-DD)
    #[arg(long)]
    pub until: Option<String>,

    /// Filter by journal category
    #[arg(long)]
    pub journal: Option<String>,
}

#[derive(Args)]
pub struct ImportArgs {
    /// Path to file to import
    pub path: String,

    /// Import format
    #[arg(short, long, value_enum, ignore_case = true, default_value_t = ImportFormat::Org)]
    pub format: ImportFormat,

    /// Filter by specific date (YYYY-MM-DD) - only import entries from this date
    #[arg(long)]
    pub date: Option<String>,

    /// Target journal category for imported entries
    #[arg(long)]
    pub journal: Option<String>,
}

pub fn export(
    journal: &Journal,
    config: &Config,
    args: ExportArgs,
    global_journal: Option<&str>,
) -> Result<()> {
    let filters = create_export_filters(
        args.date,
        args.since,
        args.until,
        args.journal.or_else(|| global_journal.map(str::to_string)),
    );
    handle_export_command(
        journal,
        args.output,
        args.format,
        filters,
        config.get_timezone(),
    )
}

pub fn import(
    journal: &Journal,
    config: &Config,
    args: ImportArgs,
    global_journal: Option<&str>,
) -> Result<()> {
    handle_import_command(
        journal,
        &args.path,
        args.format,
        args.date,
        args.journal.or_else(|| global_journal.map(str::to_string)),
        config.get_timezone(),
    )
}

fn handle_export_command(
    journal: &Journal,
    output_path: Option<String>,
    format: ExportFormat,
    filters: Option<ExportFilters>,
    timezone: Option<Tz>,
) -> Result<()> {
    let exporter = Exporter::new(journal, timezone);

    match format {
        ExportFormat::Json => exporter.export_to_json(output_path.clone(), filters)?,
        ExportFormat::Markdown => exporter.export_to_markdown(output_path.clone(), filters)?,
        ExportFormat::Org => exporter.export_to_org(output_path.clone(), filters)?,
    }

    // Only report success when writing to a file; stdout carries the payload
    if let Some(path) = &output_path {
        println!(
            "{}",
            format!("Entries exported successfully to {}", path).green()
        );
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
    format: ImportFormat,
    date: Option<String>,
    journal_category: Option<String>,
    timezone: Option<Tz>,
) -> Result<()> {
    // Parse date filter if provided
    let filter_date = date
        .as_deref()
        .map(|d| parse_relative_date(d, timezone))
        .transpose()
        .map_err(|e| anyhow::anyhow!("Invalid date filter: {}", e))?;

    let importer = Importer::new(journal, timezone);

    let notice = match format {
        ImportFormat::Org => "org-journal import is still very VERY experimental.",
        ImportFormat::Dayone => "DayOne JSON import is still experimental.",
    };
    println!("{} {}", "[EXPERIMENTAL]".yellow(), notice);
    println!("{}", format!("Importing from {}...", file_path).cyan());

    let stats = match format {
        ImportFormat::Org => {
            importer.import_from_org(file_path, journal_category.as_deref(), filter_date)?
        }
        ImportFormat::Dayone => {
            importer.import_from_dayone(file_path, journal_category.as_deref(), filter_date)?
        }
    };

    print_import_stats(&stats);

    Ok(())
}

fn print_import_stats(stats: &ImportStats) {
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
