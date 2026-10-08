use std::fmt;

use crate::database::Database;
use crate::time::{days_in_month, next_day_start_utc, start_of_day_utc};
use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, Utc};
use chrono_tz::Tz;
use rusqlite::{OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};

const ENTRY_SELECT: &str = r#"
    SELECT id, timestamp, title, content, audio_path, image_paths,
           journal, created_at, updated_at
    FROM entries
"#;

#[derive(Debug, Serialize, Deserialize)]
pub struct Entry {
    pub id: i64,
    pub timestamp: DateTime<Utc>,
    pub title: Option<String>,
    pub content: String,
    pub audio_path: Option<String>,
    pub image_paths: Vec<String>,
    pub journal: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Entry {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        let image_paths_json: Option<String> = row.get("image_paths")?;
        let image_paths = match image_paths_json {
            Some(json) => serde_json::from_str(&json).unwrap_or_default(),
            None => Vec::new(),
        };

        Ok(Entry {
            id: row.get("id")?,
            timestamp: row.get("timestamp")?,
            title: row.get("title")?,
            content: row.get("content")?,
            audio_path: row.get("audio_path")?,
            image_paths,
            journal: row
                .get("journal")
                .unwrap_or_else(|_| DEFAULT_JOURNAL.to_string()),
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    }

    pub fn get_summary(&self, summary_size: usize) -> String {
        let content_preview = truncate_with_ellipsis(&self.content, summary_size);
        if let Some(title) = self.title.as_deref() {
            format!(
                "[{}] {} [{}] - {} - {}",
                self.id,
                self.timestamp.format("%Y-%m-%d %H:%M"),
                self.journal,
                title,
                content_preview
            )
        } else {
            format!(
                "[{}] {} [{}] - {}",
                self.id,
                self.timestamp.format("%Y-%m-%d %H:%M"),
                self.journal,
                content_preview
            )
        }
    }
}

type TimestampBounds = (Option<DateTime<Utc>>, Option<DateTime<Utc>>);

const DEFAULT_JOURNAL: &str = "Personal";

/// An entry to be inserted with [`Journal::create_entry`].
///
/// Only the content is required. The timestamp defaults to now and the
/// journal to "Personal".
#[derive(Debug, Clone, Copy)]
pub struct NewEntry<'a> {
    content: &'a str,
    title: Option<&'a str>,
    journal: Option<&'a str>,
    timestamp: Option<DateTime<Utc>>,
    audio_path: Option<&'a str>,
}

impl<'a> NewEntry<'a> {
    pub fn new(content: &'a str) -> Self {
        Self {
            content,
            title: None,
            journal: None,
            timestamp: None,
            audio_path: None,
        }
    }

    pub fn title(mut self, title: Option<&'a str>) -> Self {
        self.title = title;
        self
    }

    pub fn journal(mut self, journal: Option<&'a str>) -> Self {
        self.journal = journal;
        self
    }

    pub fn timestamp(mut self, timestamp: DateTime<Utc>) -> Self {
        self.timestamp = Some(timestamp);
        self
    }

    pub fn audio_path(mut self, audio_path: Option<&'a str>) -> Self {
        self.audio_path = audio_path;
        self
    }
}

/// Sort direction for entries, always ordered by timestamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    OldestFirst,
    NewestFirst,
}

impl SortOrder {
    fn as_sql(self) -> &'static str {
        match self {
            SortOrder::OldestFirst => "ASC",
            SortOrder::NewestFirst => "DESC",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct EntryFilters {
    pub date: Option<NaiveDate>,
    pub since: Option<NaiveDate>,
    pub until: Option<NaiveDate>,
    pub journal: Option<String>,
}

impl EntryFilters {
    pub fn is_active(&self) -> bool {
        self.date.is_some()
            || self.since.is_some()
            || self.until.is_some()
            || self.journal.is_some()
    }

    pub fn for_month(year: i32, month: u32, journal: Option<&str>) -> Result<Self> {
        let last_day = days_in_month(year, month)
            .with_context(|| format!("Invalid year/month combination: {year}-{month:02}"))?;
        let month_start = NaiveDate::from_ymd_opt(year, month, 1).expect("validated above");
        let month_end = NaiveDate::from_ymd_opt(year, month, last_day).expect("validated above");

        Ok(Self {
            since: Some(month_start),
            until: Some(month_end),
            journal: journal.map(str::to_string),
            ..Self::default()
        })
    }

    /// UTC bounds of the filter. Days are calendar days in `tz` (system local if `None`).
    fn timestamp_bounds(&self, tz: Option<Tz>) -> Result<TimestampBounds> {
        let mut start = self
            .date
            .map(|date| start_of_day_utc(date, tz))
            .transpose()?;
        let mut end = self
            .date
            .map(|date| next_day_start_utc(date, tz))
            .transpose()?;

        if let Some(since) = self.since {
            let since_start = start_of_day_utc(since, tz)?;
            start = Some(match start {
                Some(current) => current.max(since_start),
                None => since_start,
            });
        }

        if let Some(until) = self.until {
            let until_end = next_day_start_utc(until, tz)?;
            end = Some(match end {
                Some(current) => current.min(until_end),
                None => until_end,
            });
        }

        Ok((start, end))
    }
}

pub fn truncate_with_ellipsis(text: &str, max_chars: usize) -> String {
    let mut chars = text.chars();
    let preview: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{}...", preview.trim_end())
    } else {
        preview
    }
}

impl fmt::Display for Entry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}] {} [{}] - {}\n{}\n",
            self.id,
            self.timestamp.format("%Y-%m-%d %H:%M"),
            self.journal,
            self.title.as_deref().unwrap_or("Untitled"),
            self.content
        )
    }
}

pub struct Journal {
    db: Database,
    timezone: Option<Tz>,
}

impl Journal {
    pub fn new(db: Database) -> Self {
        Journal { db, timezone: None }
    }

    /// Timezone used to interpret calendar days in date filters (system local if `None`).
    pub fn with_timezone(mut self, timezone: Option<Tz>) -> Self {
        self.timezone = timezone;
        self
    }

    /// Insert a new entry and return its id.
    pub fn create_entry(&self, entry: NewEntry) -> Result<i64> {
        let conn = self.db.connection();
        let now = Utc::now();
        let timestamp = entry.timestamp.unwrap_or(now);

        conn.execute(
            "INSERT INTO entries (timestamp, title, content, journal, audio_path, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                timestamp,
                entry.title,
                entry.content,
                entry.journal.unwrap_or(DEFAULT_JOURNAL),
                entry.audio_path,
                now,
                now
            ],
        )?;

        Ok(conn.last_insert_rowid())
    }

    pub fn get_entry(&self, id: i64) -> Result<Option<Entry>> {
        let conn = self.db.connection();
        let mut stmt = conn.prepare(&format!("{ENTRY_SELECT} WHERE id = ?1"))?;

        stmt.query_row([id], Entry::from_row)
            .optional()
            .map_err(Into::into)
    }

    pub fn list_entries(&self) -> Result<Vec<Entry>> {
        self.list_entries_with_order(SortOrder::NewestFirst)
    }

    pub fn list_entries_with_order(&self, order: SortOrder) -> Result<Vec<Entry>> {
        let query = format!("{ENTRY_SELECT} ORDER BY timestamp {}", order.as_sql());

        self.collect_entries(&query, [])
    }

    pub fn search_entries(&self, query: &str) -> Result<Vec<Entry>> {
        let search_pattern = format!("%{}%", query);
        let query = format!(
            "{ENTRY_SELECT} WHERE content LIKE ?1 OR title LIKE ?1 ORDER BY created_at DESC"
        );
        self.collect_entries(&query, [&search_pattern])
    }

    pub fn delete_entry(&self, id: i64) -> Result<bool> {
        let conn = self.db.connection();

        let rows_affected = conn.execute("DELETE FROM entries WHERE id = ?1", [id])?;

        Ok(rows_affected > 0)
    }

    /// Update entry's title, content, journal, and timestamp. Returns true if the entry was found and updated.
    ///
    /// # Arguments
    /// * `id` - The ID of the entry to update.
    /// * `title` - The new title for the entry. Use `None` to clear the title.
    /// * `content` - The new content for the entry.
    /// * `journal` - The new journal for the entry.
    /// * `timestamp` - The new timestamp for the entry.
    ///
    /// # Returns
    /// * `Result<bool>` - Ok(true) if the entry was updated, Ok(false) if not found, Err on error.
    pub fn update_entry_with_metadata(
        &self,
        id: i64,
        title: Option<&str>,
        content: &str,
        journal: &str,
        timestamp: DateTime<Utc>,
    ) -> Result<bool> {
        let conn = self.db.connection();
        let now = Utc::now();

        let rows_affected = conn.execute(
            "UPDATE entries SET title = ?1, content = ?2, journal = ?3, timestamp = ?4, updated_at = ?5 WHERE id = ?6",
            params![title, content, journal, timestamp, now, id],
        )?;

        Ok(rows_affected > 0)
    }

    pub fn list_journals(&self) -> Result<Vec<String>> {
        let conn = self.db.connection();
        let mut stmt = conn.prepare("SELECT DISTINCT journal FROM entries ORDER BY journal ASC")?;
        let journals = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;
        Ok(journals)
    }

    pub fn move_entry(&self, id: i64, new_journal: &str) -> Result<bool> {
        let conn = self.db.connection();
        let now = Utc::now();

        let rows_affected = conn.execute(
            "UPDATE entries SET journal = ?1, updated_at = ?2 WHERE id = ?3",
            params![new_journal, now, id],
        )?;

        Ok(rows_affected > 0)
    }

    pub fn list_entries_filtered(&self, filters: &EntryFilters) -> Result<Vec<Entry>> {
        self.list_entries_filtered_with_order(filters, SortOrder::NewestFirst)
    }

    pub fn list_entries_filtered_with_order(
        &self,
        filters: &EntryFilters,
        order: SortOrder,
    ) -> Result<Vec<Entry>> {
        let (start, end) = filters.timestamp_bounds(self.timezone)?;
        let journal = filters.journal.as_deref();
        let mut query = ENTRY_SELECT.to_string();
        let mut conditions = Vec::new();

        if start.is_some() {
            conditions.push("timestamp >= ?");
        }
        if end.is_some() {
            conditions.push("timestamp < ?");
        }
        if journal.is_some() {
            conditions.push("journal = ?");
        }

        if filters.is_active() {
            query.push_str(" WHERE ");
            query.push_str(&conditions.join(" AND "));
        }

        query.push_str(&format!(" ORDER BY timestamp {}", order.as_sql()));

        match (start.as_ref(), end.as_ref(), journal) {
            (Some(start), Some(end), Some(journal)) => {
                self.collect_entries(&query, params![start, end, journal])
            }
            (Some(start), Some(end), None) => self.collect_entries(&query, params![start, end]),
            (Some(start), None, Some(journal)) => {
                self.collect_entries(&query, params![start, journal])
            }
            (Some(start), None, None) => self.collect_entries(&query, params![start]),
            (None, Some(end), Some(journal)) => self.collect_entries(&query, params![end, journal]),
            (None, Some(end), None) => self.collect_entries(&query, params![end]),
            (None, None, Some(journal)) => self.collect_entries(&query, params![journal]),
            (None, None, None) => self.collect_entries(&query, []),
        }
    }

    pub fn list_entries_for_month(&self, year: i32, month: u32) -> Result<Vec<Entry>> {
        self.list_entries_for_month_filtered(year, month, None)
    }

    pub fn list_entries_for_month_filtered(
        &self,
        year: i32,
        month: u32,
        journal: Option<&str>,
    ) -> Result<Vec<Entry>> {
        let filters = EntryFilters::for_month(year, month, journal)?;
        self.list_entries_filtered_with_order(&filters, SortOrder::OldestFirst)
    }

    fn collect_entries<P>(&self, query: &str, params: P) -> Result<Vec<Entry>>
    where
        P: rusqlite::Params,
    {
        let conn = self.db.connection();
        let mut stmt = conn.prepare(query)?;
        let entry_iter = stmt.query_map(params, Entry::from_row)?;

        let mut entries = Vec::new();
        for entry in entry_iter {
            entries.push(entry?);
        }

        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncation_is_unicode_safe() {
        assert_eq!(truncate_with_ellipsis("naive cafe", 5), "naive...");
        assert_eq!(truncate_with_ellipsis("caffe latte", 20), "caffe latte");
        assert_eq!(
            truncate_with_ellipsis("caffe e cornetto ☕", 17),
            "caffe e cornetto..."
        );
    }

    #[test]
    fn exact_date_creates_day_bounds() {
        let filters = EntryFilters {
            date: NaiveDate::from_ymd_opt(2026, 5, 1),
            ..EntryFilters::default()
        };
        let (start, end) = filters
            .timestamp_bounds(Some(chrono_tz::UTC))
            .expect("bounds should resolve");

        assert_eq!(
            start.expect("start bound").date_naive(),
            NaiveDate::from_ymd_opt(2026, 5, 1).expect("valid date")
        );
        assert_eq!(
            end.expect("end bound").date_naive(),
            NaiveDate::from_ymd_opt(2026, 5, 2).expect("valid date")
        );
    }

    #[test]
    fn day_bounds_follow_the_timezone() {
        let filters = EntryFilters {
            date: NaiveDate::from_ymd_opt(2026, 5, 1),
            ..EntryFilters::default()
        };
        let (start, end) = filters
            .timestamp_bounds(Some(chrono_tz::Europe::Rome))
            .expect("bounds should resolve");

        // Rome is UTC+2 in May: local midnight is 22:00 UTC the previous day
        assert_eq!(
            start.expect("start bound").to_rfc3339(),
            "2026-04-30T22:00:00+00:00"
        );
        assert_eq!(
            end.expect("end bound").to_rfc3339(),
            "2026-05-01T22:00:00+00:00"
        );
    }
}
