use crate::cli::dateparser::parse_entry_filters;
use crate::cli::display::{
    SUMMARY_DATE_FORMAT, content_preview, entry_date, format_tags, split_matches, stardate_parts,
};
use crate::cli::formatting::{get_wrap_width, wrap_text};
use crate::config::{Config, DEFAULT_LIST_LIMIT};
use crate::journal::{Entry, Journal, SortOrder};
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

    /// Only entries with this tag (repeat to require several tags)
    #[arg(long = "tag")]
    pub tags: Vec<String>,

    /// Maximum number of entries to show (defaults to display.entries_per_page or 20)
    #[arg(short, long)]
    pub limit: Option<usize>,

    /// Show all matching entries
    #[arg(long, conflicts_with = "limit")]
    pub all: bool,

    /// Show oldest entries first (default: newest first)
    #[arg(long)]
    pub oldest_first: bool,
}

#[derive(Args)]
pub struct SearchArgs {
    /// Search query (case-insensitive, matched literally in titles and content)
    pub query: String,

    /// Filter by journal category
    #[arg(long)]
    pub journal: Option<String>,

    /// Only entries with this tag (repeat to require several tags)
    #[arg(long = "tag")]
    pub tags: Vec<String>,

    /// Only entries since date (YYYY-MM-DD or relative, e.g. "last week")
    #[arg(long)]
    pub since: Option<String>,

    /// Only entries until date (YYYY-MM-DD or relative)
    #[arg(long)]
    pub until: Option<String>,

    /// Maximum number of results to show (newest first)
    #[arg(short, long)]
    pub limit: Option<usize>,
}

pub fn list_entries(journal: &Journal, config: &Config, args: ListArgs) -> Result<()> {
    let ListArgs {
        date,
        since,
        until,
        journal: list_journal,
        tags,
        limit,
        all,
        oldest_first,
    } = args;

    if limit == Some(0) {
        anyhow::bail!("--limit must be greater than 0");
    }

    let journal_filter = list_journal.as_deref();
    let filters = parse_entry_filters(
        date.as_deref(),
        since.as_deref(),
        until.as_deref(),
        journal_filter,
        &tags,
        config.get_timezone(),
    )?;

    let total_entries = journal.count_entries(&filters)?;
    if total_entries == 0 {
        println!("{}", "No entries found".yellow());
        return Ok(());
    }

    let effective_limit = if all {
        None
    } else {
        Some(
            limit
                .or(config.display.entries_per_page)
                .unwrap_or(DEFAULT_LIST_LIMIT),
        )
    };

    let mut entries =
        journal.list_entries_filtered_limited(&filters, SortOrder::NewestFirst, effective_limit)?;

    if oldest_first {
        entries.reverse();
    }

    let shown_entries = entries.len();
    let order_label = if oldest_first {
        "oldest to newest"
    } else {
        "newest first"
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
            format_entry_summary(
                &entry,
                config.display.stardate_mode,
                config.get_timezone(),
                None
            )
        );
    }

    Ok(())
}

pub fn show_entry(journal: &Journal, config: &Config, id: i64) -> Result<()> {
    match journal.get_entry(id)? {
        Some(entry) => {
            print_entry(&entry, config);
            Ok(())
        }
        None => anyhow::bail!("Entry {} not found", id),
    }
}

pub fn search_entries(journal: &Journal, config: &Config, args: SearchArgs) -> Result<()> {
    let SearchArgs {
        query,
        journal: journal_filter,
        tags,
        since,
        until,
        limit,
    } = args;

    if limit == Some(0) {
        anyhow::bail!("--limit must be greater than 0");
    }

    let filters = parse_entry_filters(
        None,
        since.as_deref(),
        until.as_deref(),
        journal_filter.as_deref(),
        &tags,
        config.get_timezone(),
    )?;
    let mut entries = journal.search_entries(&query, &filters)?;

    if entries.is_empty() {
        println!(
            "{}",
            format!("No entries found matching '{}'", query).yellow()
        );
        return Ok(());
    }

    let total_entries = entries.len();
    if let Some(limit) = limit {
        entries.truncate(limit);
    }

    println!(
        "{}",
        if entries.len() == total_entries {
            format!("Found {total_entries} entries matching '{query}':")
        } else {
            format!(
                "Found {total_entries} entries matching '{query}', showing latest {}:",
                entries.len()
            )
        }
        .green()
        .bold()
    );
    println!();
    for entry in entries {
        println!(
            "{}",
            format_entry_summary(
                &entry,
                config.display.stardate_mode,
                config.get_timezone(),
                Some(&query)
            )
        );
    }
    Ok(())
}

pub fn print_entry(entry: &Entry, config: &Config) {
    let stardate_mode = config.display.stardate_mode;
    let timezone = config.get_timezone();
    let width = get_wrap_width();
    println!("{}", "─".repeat(width as usize).bright_blue());
    println!(
        "{}: {}",
        "ID".cyan().bold(),
        entry.id.to_string().white().bold()
    );
    if stardate_mode {
        println!("{}: {}", "Stardate".cyan().bold(), format_stardate(entry));
    } else {
        println!(
            "{}: {}",
            "Date".cyan().bold(),
            entry_date(entry, timezone, &config.display.date_format).white()
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

    if !entry.tags.is_empty() {
        println!(
            "{}: {}",
            "Tags".cyan().bold(),
            format_tags(&entry.tags).blue()
        );
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

/// `text` with every match of `query` highlighted and the rest styled by `base`.
fn highlight_matches(text: &str, query: &str, base: impl Fn(&str) -> ColoredString) -> String {
    split_matches(text, query)
        .into_iter()
        .map(|(segment, is_match)| {
            if is_match {
                segment.black().on_yellow().bold().to_string()
            } else {
                base(&segment).to_string()
            }
        })
        .collect()
}

fn format_entry_summary(
    entry: &Entry,
    stardate_mode: bool,
    timezone: Option<Tz>,
    highlight: Option<&str>,
) -> String {
    let preview = content_preview(&entry.content, highlight);
    let content_preview = match highlight {
        Some(query) => highlight_matches(&preview, query, |s| s.normal()),
        None => preview,
    };

    let id = format!("[{}]", entry.id).bright_blue().bold();

    let date = if stardate_mode {
        format_stardate(entry)
    } else {
        entry_date(entry, timezone, SUMMARY_DATE_FORMAT)
            .white()
            .to_string()
    };

    let journal = format!("[{}]", entry.journal).magenta().bold();

    let tags = if entry.tags.is_empty() {
        String::new()
    } else {
        format!(" {}", format_tags(&entry.tags).bright_black())
    };

    // Add audio indicator if entry has audio
    let audio_indicator = if entry.audio_path.is_some() {
        " 🎤"
    } else {
        ""
    };

    if let Some(title) = &entry.title {
        format!(
            "{} {} {} - {} - {}{}{}",
            id,
            date,
            journal,
            match highlight {
                Some(query) => highlight_matches(title, query, |s| s.green().bold()),
                None => title.green().bold().to_string(),
            },
            content_preview,
            tags,
            audio_indicator
        )
    } else {
        format!(
            "{} {} {} - {}{}{}",
            id, date, journal, content_preview, tags, audio_indicator
        )
    }
}

/// A stardate with the last two digits dimmed.
fn format_stardate(entry: &Entry) -> String {
    let (head, tail) = stardate_parts(entry);
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
                format_entry_summary(&entry, config.display.stardate_mode, tz, None)
            );
        }
    }

    Ok(())
}
