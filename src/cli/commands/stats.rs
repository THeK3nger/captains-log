use crate::cli::dateparser::parse_entry_filters;
use crate::config::Config;
use crate::journal::{Entry, Journal, SortOrder};
use crate::time::{local_date, today};
use anyhow::Result;
use chrono::{Datelike, Days, NaiveDate, Weekday};
use clap::Args;
use colored::*;
use std::collections::{BTreeSet, HashMap};

const TOP_TAGS: usize = 5;

#[derive(Args)]
pub struct StatsArgs {
    /// Only count entries since date (YYYY-MM-DD or relative, e.g. "last month")
    #[arg(long)]
    pub since: Option<String>,

    /// Only count entries until date
    #[arg(long)]
    pub until: Option<String>,

    /// Filter by journal category
    #[arg(long)]
    pub journal: Option<String>,
}

#[derive(Debug, PartialEq)]
struct Stats {
    total: usize,
    per_journal: Vec<(String, usize)>,
    current_streak: usize,
    longest_streak: usize,
    last_7_days: usize,
    last_30_days: usize,
    avg_words: usize,
    top_tags: Vec<(String, usize)>,
    busiest_weekday: Option<(Weekday, usize)>,
}

/// Sort by count descending, then name, so output is stable.
fn ranked(counts: HashMap<String, usize>) -> Vec<(String, usize)> {
    let mut ranked: Vec<_> = counts.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    ranked
}

fn streaks(days: &BTreeSet<NaiveDate>, today: NaiveDate) -> (usize, usize) {
    let mut longest = 0;
    let mut run = 0;
    let mut previous: Option<NaiveDate> = None;
    for &day in days {
        run = match previous {
            Some(prev) if prev.succ_opt() == Some(day) => run + 1,
            _ => 1,
        };
        longest = longest.max(run);
        previous = Some(day);
    }

    // A streak is still alive if the last entry was today or yesterday
    let mut current = 0;
    let mut day = if days.contains(&today) {
        Some(today)
    } else {
        today.pred_opt().filter(|d| days.contains(d))
    };
    while let Some(d) = day.filter(|d| days.contains(d)) {
        current += 1;
        day = d.pred_opt();
    }
    (current, longest)
}

fn compute(entries: &[Entry], today: NaiveDate, tz: Option<chrono_tz::Tz>) -> Stats {
    let mut journals: HashMap<String, usize> = HashMap::new();
    let mut tags: HashMap<String, usize> = HashMap::new();
    let mut weekdays: HashMap<Weekday, usize> = HashMap::new();
    let mut days = BTreeSet::new();
    let mut words = 0;

    for entry in entries {
        let date = local_date(&entry.timestamp, tz);
        days.insert(date);
        *weekdays.entry(date.weekday()).or_default() += 1;
        *journals.entry(entry.journal.clone()).or_default() += 1;
        for tag in &entry.tags {
            *tags.entry(tag.clone()).or_default() += 1;
        }
        words += entry.content.split_whitespace().count();
    }

    let within = |n: u64| {
        let from = today.checked_sub_days(Days::new(n - 1)).unwrap_or(today);
        entries
            .iter()
            .filter(|e| (from..=today).contains(&local_date(&e.timestamp, tz)))
            .count()
    };
    let (current_streak, longest_streak) = streaks(&days, today);

    Stats {
        total: entries.len(),
        per_journal: ranked(journals),
        current_streak,
        longest_streak,
        last_7_days: within(7),
        last_30_days: within(30),
        avg_words: words.checked_div(entries.len()).unwrap_or(0),
        top_tags: ranked(tags).into_iter().take(TOP_TAGS).collect(),
        busiest_weekday: weekdays
            .into_iter()
            .max_by_key(|&(day, count)| (count, std::cmp::Reverse(day.num_days_from_monday()))),
    }
}

fn plural(n: usize, unit: &str) -> String {
    format!("{n} {unit}{}", if n == 1 { "" } else { "s" })
}

pub fn show_stats(journal: &Journal, config: &Config, args: StatsArgs) -> Result<()> {
    let tz = config.get_timezone();
    let filters = parse_entry_filters(
        None,
        args.since.as_deref(),
        args.until.as_deref(),
        args.journal.as_deref(),
        &[],
        tz,
    )?;
    let entries = journal.list_entries_filtered_with_order(&filters, SortOrder::OldestFirst)?;
    if entries.is_empty() {
        println!("{}", "No entries found".yellow());
        return Ok(());
    }

    let stats = compute(&entries, today(tz), tz);
    let label = |text: &str| format!("{:<16}", text).bold();

    println!("{}", "Captain's Log statistics".cyan().bold());
    println!("{}{}", label("Entries"), stats.total);
    for (name, count) in &stats.per_journal {
        println!("  {:<14}{}", name, count);
    }
    println!("{}{}", label("Last 7 days"), stats.last_7_days);
    println!("{}{}", label("Last 30 days"), stats.last_30_days);
    println!(
        "{}{} (longest: {})",
        label("Current streak"),
        plural(stats.current_streak, "day"),
        plural(stats.longest_streak, "day")
    );
    println!(
        "{}{}",
        label("Avg. length"),
        plural(stats.avg_words, "word")
    );
    if let Some((day, count)) = stats.busiest_weekday {
        println!(
            "{}{} ({})",
            label("Busiest day"),
            day,
            if count == 1 {
                "1 entry".to_string()
            } else {
                format!("{count} entries")
            }
        );
    }
    if !stats.top_tags.is_empty() {
        println!("{}", "Top tags".bold());
        for (name, count) in &stats.top_tags {
            println!("  {:<14}{}", format!("#{name}").blue(), count);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use chrono_tz::UTC;

    fn entry(day: u32, journal: &str, content: &str, tags: &[&str]) -> Entry {
        let at = Utc.with_ymd_and_hms(2025, 9, day, 12, 0, 0).unwrap();
        Entry {
            id: i64::from(day),
            timestamp: at,
            title: None,
            content: content.to_string(),
            audio_path: None,
            image_paths: vec![],
            journal: journal.to_string(),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            created_at: at,
            updated_at: at,
        }
    }

    #[test]
    fn computes_streaks_counts_and_tags() {
        let entries = vec![
            entry(1, "Personal", "one two", &["a"]),
            entry(2, "Work", "one two three four", &["a", "b"]),
            entry(3, "Work", "x", &[]),
            entry(10, "Work", "y", &["a"]),
            entry(11, "Work", "z", &[]),
        ];
        let today = NaiveDate::from_ymd_opt(2025, 9, 12).unwrap();
        let stats = compute(&entries, today, Some(UTC));

        assert_eq!(stats.total, 5);
        assert_eq!(
            stats.per_journal,
            vec![("Work".into(), 4), ("Personal".into(), 1)]
        );
        assert_eq!(stats.longest_streak, 3);
        assert_eq!(stats.current_streak, 2); // 10th and 11th; today is empty but yesterday counts
        assert_eq!(stats.last_7_days, 2);
        assert_eq!(stats.last_30_days, 5);
        assert_eq!(stats.avg_words, 1);
        assert_eq!(stats.top_tags, vec![("a".into(), 3), ("b".into(), 1)]);
    }

    #[test]
    fn streak_is_broken_after_a_missed_day() {
        let days: BTreeSet<_> = [NaiveDate::from_ymd_opt(2025, 9, 1).unwrap()].into();
        let today = NaiveDate::from_ymd_opt(2025, 9, 3).unwrap();
        assert_eq!(streaks(&days, today), (0, 1));
    }
}
