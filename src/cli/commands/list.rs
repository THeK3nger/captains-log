use crate::cli::dateparser::parse_entry_filters;
use crate::cli::formatting::{get_wrap_width, wrap_text};
use crate::cli::stardate::Stardate;
use crate::config::{Config, DEFAULT_LIST_LIMIT};
use crate::journal::{Entry, Journal, truncate_with_ellipsis};
use crate::time::{days_in_month, to_local};
use anyhow::{Context, Result};
use chrono::{Datelike, NaiveDate, Utc};
use chrono_tz::Tz;
use clap::Args;
use colored::*;

use crate::cli::formatting::render_markdown;

#[derive(Args)]
pub struct ListArgs {
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

    /// Maximum number of entries to show (defaults to display.entries_per_page or 20)
    #[arg(short, long)]
    pub limit: Option<usize>,

    /// Show all matching entries
    #[arg(long, conflicts_with = "limit")]
    pub all: bool,

    /// Show newest entries first
    #[arg(long)]
    pub newest_first: bool,
}

pub fn list_entries(
    journal: &Journal,
    config: &Config,
    args: ListArgs,
    global_journal: Option<&str>,
) -> Result<()> {
    let ListArgs {
        date,
        since,
        until,
        journal: list_journal,
        limit,
        all,
        newest_first,
    } = args;

    if limit == Some(0) {
        anyhow::bail!("--limit must be greater than 0");
    }

    let journal_filter = list_journal.as_deref().or(global_journal);
    let filters = parse_entry_filters(
        date.as_deref(),
        since.as_deref(),
        until.as_deref(),
        journal_filter,
        config.get_timezone(),
    )?;

    let mut entries = if filters.is_active() {
        journal.list_entries_filtered(&filters)?
    } else {
        journal.list_entries()?
    };

    if entries.is_empty() {
        println!("{}", "No entries found".yellow());
        return Ok(());
    }

    let total_entries = entries.len();
    let effective_limit = if all {
        None
    } else {
        Some(
            limit
                .or(config.display.entries_per_page)
                .unwrap_or(DEFAULT_LIST_LIMIT),
        )
    };

    if let Some(limit) = effective_limit {
        entries.truncate(limit);
    }

    if !newest_first {
        entries.reverse();
    }

    let shown_entries = entries.len();
    let order_label = if newest_first {
        "newest first"
    } else {
        "oldest to newest"
    };

    println!(
        "{}",
        if shown_entries == total_entries {
            format!("Found {total_entries} entries ({order_label}):")
        } else {
            format!(
                "Found {total_entries} entries, showing latest {shown_entries} ({order_label}):"
            )
        }
        .green()
        .bold()
    );
    println!();
    for entry in entries {
        println!(
            "{}",
            format_entry_summary(&entry, config.display.stardate_mode, config.get_timezone())
        );
    }

    Ok(())
}

pub fn show_entry(journal: &Journal, config: &Config, id: i64) -> Result<()> {
    match journal.get_entry(id)? {
        Some(entry) => {
            print_entry(&entry, config.display.stardate_mode, config.get_timezone());
            Ok(())
        }
        None => anyhow::bail!("Entry {} not found", id),
    }
}

pub fn search_entries(journal: &Journal, config: &Config, query: &str) -> Result<()> {
    let entries = journal.search_entries(query)?;
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
                format_entry_summary(&entry, config.display.stardate_mode, config.get_timezone())
            );
        }
    }
    Ok(())
}

pub fn print_entry(entry: &Entry, stardate_mode: bool, timezone: Option<Tz>) {
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
            to_local(&entry.timestamp, timezone)
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

fn format_entry_summary(entry: &Entry, stardate_mode: bool, timezone: Option<Tz>) -> String {
    // Strip newlines and limit content preview to 40 chars.
    let content_preview = truncate_with_ellipsis(&entry.content.replace('\n', " "), 40);

    let id = format!("[{}]", entry.id).bright_blue().bold();

    let date = if stardate_mode {
        let stardate = entry.timestamp.to_stardate();
        format_stardate(stardate)
    } else {
        to_local(&entry.timestamp, timezone)
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
        format!(
            "{} {} {} - {}{}",
            id,
            date,
            journal,
            content_preview.normal(),
            audio_indicator
        )
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

pub fn show_calendar(
    journal: &Journal,
    year: Option<i32>,
    month: Option<u32>,
    journal_filter: Option<&str>,
    config: &Config,
) -> Result<()> {
    let now = to_local(&Utc::now(), config.get_timezone());
    let year = year.unwrap_or(now.year());
    let month = month.unwrap_or(now.month());

    // Validate month
    if !(1..=12).contains(&month) {
        return Err(anyhow::anyhow!("Month must be between 1 and 12"));
    }

    // Get entries for the month
    let entries = journal.list_entries_for_month_filtered(year, month, journal_filter)?;

    // Create a map of day -> entry count
    let tz = config.get_timezone();
    let mut day_counts = std::collections::HashMap::new();
    for entry in &entries {
        let day = to_local(&entry.timestamp, tz).day();
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

    let days_in_month = days_in_month(year, month).context("Invalid date")?;

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
                format_entry_summary(&entry, config.display.stardate_mode, tz)
            );
        }
    }

    Ok(())
}
