use crate::cli::dateparser::parse_entry_filters;
use crate::cli::formatting::{get_wrap_width, wrap_text};
use crate::cli::stardate::Stardate;
use crate::config::{Config, DEFAULT_LIST_LIMIT};
use crate::journal::{Entry, Journal, SortOrder, truncate_with_ellipsis};
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
            print_entry(&entry, config.display.stardate_mode, config.get_timezone());
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

const PREVIEW_CHARS: usize = 40;

/// Case-insensitive fold that keeps one char per char, so indices stay aligned with the input.
fn fold_char(c: char) -> char {
    let mut lower = c.to_lowercase();
    match (lower.next(), lower.next()) {
        (Some(single), None) => single,
        _ => c,
    }
}

/// Char ranges `[start, end)` of non-overlapping case-insensitive matches of `query`.
fn match_ranges(chars: &[char], query: &str) -> Vec<(usize, usize)> {
    let needle: Vec<char> = query.chars().map(fold_char).collect();
    if needle.is_empty() {
        return Vec::new();
    }
    let haystack: Vec<char> = chars.iter().copied().map(fold_char).collect();

    let mut ranges = Vec::new();
    let mut i = 0;
    while i + needle.len() <= haystack.len() {
        if haystack[i..].starts_with(&needle) {
            ranges.push((i, i + needle.len()));
            i += needle.len();
        } else {
            i += 1;
        }
    }
    ranges
}

/// `text` with every match of `query` highlighted and the rest styled by `base`.
fn highlight_matches(text: &str, query: &str, base: impl Fn(&str) -> ColoredString) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut cursor = 0;
    for (start, end) in match_ranges(&chars, query) {
        if start > cursor {
            out.push_str(&base(&chars[cursor..start].iter().collect::<String>()).to_string());
        }
        out.push_str(
            &chars[start..end]
                .iter()
                .collect::<String>()
                .black()
                .on_yellow()
                .bold()
                .to_string(),
        );
        cursor = end;
    }
    if cursor < chars.len() {
        out.push_str(&base(&chars[cursor..].iter().collect::<String>()).to_string());
    }
    out
}

/// One-line preview of `content`; with a query, the window moves so the first match is visible.
fn content_preview(content: &str, query: Option<&str>) -> String {
    let flat = content.replace('\n', " ");
    let Some(first) = query.and_then(|q| {
        let chars: Vec<char> = flat.chars().collect();
        match_ranges(&chars, q)
            .first()
            .map(|&(start, end)| (start, end))
    }) else {
        return truncate_with_ellipsis(&flat, PREVIEW_CHARS);
    };
    if first.1 <= PREVIEW_CHARS {
        return truncate_with_ellipsis(&flat, PREVIEW_CHARS);
    }

    let chars: Vec<char> = flat.chars().collect();
    let start = first.0.saturating_sub(10);
    let end = (start + PREVIEW_CHARS.max(first.1 - first.0 + 10)).min(chars.len());
    let window: String = chars[start..end].iter().collect();
    let suffix = if end < chars.len() { "..." } else { "" };
    let prefix = if start > 0 { "..." } else { "" };
    format!("{prefix}{}{suffix}", window.trim())
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

/// `#a #b #c`
pub fn format_tags(tags: &[String]) -> String {
    tags.iter()
        .map(|tag| format!("#{tag}"))
        .collect::<Vec<_>>()
        .join(" ")
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
                format_entry_summary(&entry, config.display.stardate_mode, tz, None)
            );
        }
    }

    Ok(())
}
