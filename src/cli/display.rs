//! Plain-text pieces of how an entry is shown: dates, stardates, tags,
//! one-line previews and search-match positions.
//!
//! Nothing here adds terminal colors, so the same helpers serve the colored
//! CLI output (`commands::list`) and other front ends such as a TUI, which
//! apply their own styling.

use crate::cli::stardate::Stardate;
use crate::journal::{Entry, truncate_with_ellipsis};
use crate::time::to_local;
use chrono::{DateTime, FixedOffset};
use chrono_tz::Tz;

/// Width of a one-line content preview, in chars.
pub const PREVIEW_CHARS: usize = 40;

/// Timestamp format used in one-line entry summaries.
pub const SUMMARY_DATE_FORMAT: &str = "%Y-%m-%d %H:%M";

/// Format a timestamp with a user-supplied strftime pattern, falling back to the
/// default pattern if it is invalid (chrono's `to_string` would panic on it).
pub fn format_date(date: &DateTime<FixedOffset>, pattern: &str) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    if write!(out, "{}", date.format(pattern)).is_err() {
        out = date.format("%Y-%m-%d %H:%M:%S").to_string();
    }
    out
}

/// The entry's timestamp in local time, formatted with `pattern`.
pub fn entry_date(entry: &Entry, timezone: Option<Tz>, pattern: &str) -> String {
    format_date(&to_local(&entry.timestamp, timezone), pattern)
}

/// A stardate with five decimals, split into the part shown normally and the
/// last two digits, which are shown dimmed.
pub fn stardate_parts(entry: &Entry) -> (String, String) {
    let stardate = format!("{:.5}", entry.timestamp.to_stardate());
    let chars: Vec<char> = stardate.chars().collect();
    if chars.len() < 2 {
        return (stardate, String::new());
    }
    let split = chars.len() - 2;
    (
        chars[..split].iter().collect(),
        chars[split..].iter().collect(),
    )
}

/// `#a #b #c`
pub fn format_tags(tags: &[String]) -> String {
    tags.iter()
        .map(|tag| format!("#{tag}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Case-insensitive fold that keeps one char per char, so indices stay aligned with the input.
fn fold_char(c: char) -> char {
    let mut lower = c.to_lowercase();
    match (lower.next(), lower.next()) {
        (Some(single), None) => single,
        _ => c,
    }
}

/// Char ranges `[start, end)` of non-overlapping case-insensitive matches of `query`.
pub fn match_ranges(chars: &[char], query: &str) -> Vec<(usize, usize)> {
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

/// Split `text` into segments, each flagged with whether it matches `query`
/// (case-insensitively). Concatenating the segments gives back `text`.
pub fn split_matches(text: &str, query: &str) -> Vec<(String, bool)> {
    let chars: Vec<char> = text.chars().collect();
    let mut segments = Vec::new();
    let mut cursor = 0;
    for (start, end) in match_ranges(&chars, query) {
        if start > cursor {
            segments.push((chars[cursor..start].iter().collect(), false));
        }
        segments.push((chars[start..end].iter().collect(), true));
        cursor = end;
    }
    if cursor < chars.len() {
        segments.push((chars[cursor..].iter().collect(), false));
    }
    segments
}

/// One-line preview of `content`; with a query, the window moves so the first match is visible.
pub fn content_preview(content: &str, query: Option<&str>) -> String {
    let flat = content.replace('\n', " ");
    let chars: Vec<char> = flat.chars().collect();
    let first_match = query.and_then(|q| match_ranges(&chars, q).first().copied());
    let Some((match_start, match_end)) = first_match else {
        return truncate_with_ellipsis(&flat, PREVIEW_CHARS);
    };
    if match_end <= PREVIEW_CHARS {
        return truncate_with_ellipsis(&flat, PREVIEW_CHARS);
    }

    let start = match_start.saturating_sub(10);
    let end = (start + PREVIEW_CHARS.max(match_end - match_start + 10)).min(chars.len());
    let window: String = chars[start..end].iter().collect();
    let suffix = if end < chars.len() { "..." } else { "" };
    let prefix = if start > 0 { "..." } else { "" };
    format!("{prefix}{}{suffix}", window.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_matches_round_trips_and_flags_matches() {
        let segments = split_matches("Call ana, then ANA again", "ana");
        assert_eq!(
            segments,
            vec![
                ("Call ".to_string(), false),
                ("ana".to_string(), true),
                (", then ".to_string(), false),
                ("ANA".to_string(), true),
                (" again".to_string(), false),
            ]
        );
        assert!(split_matches("abc", "").iter().all(|(_, m)| !m));
    }

    #[test]
    fn preview_moves_to_a_late_match() {
        let content = format!("{} needle tail", "word ".repeat(20));
        let preview = content_preview(&content, Some("needle"));
        assert!(preview.starts_with("..."));
        assert!(preview.contains("needle"));
        assert_eq!(content_preview("short", Some("needle")), "short");
    }

    #[test]
    fn invalid_date_pattern_falls_back() {
        let date = DateTime::parse_from_rfc3339("2025-09-09T10:00:00+00:00").unwrap();
        assert_eq!(format_date(&date, "%Q"), "2025-09-09 10:00:00");
        assert_eq!(format_date(&date, "%Y"), "2025");
    }
}
