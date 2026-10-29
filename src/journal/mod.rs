mod journals;
mod tags;

use std::fmt;

use crate::database::Database;
use crate::time::{days_in_month, next_day_start_utc, start_of_day_utc};
use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, Utc};
use chrono_tz::Tz;
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};

pub use journals::JournalInfo;
pub use tags::normalize_tag;
use tags::{extract_tags, rename_in_text};

/// Entries joined with their journal, so filters can use `journals.name`.
const ENTRY_FROM: &str = " FROM entries JOIN journals ON journals.id = entries.journal_id";

const ENTRY_SELECT: &str = r#"
    SELECT entries.id AS id, entries.timestamp AS timestamp, entries.title AS title,
           entries.content AS content, entries.audio_path AS audio_path,
           entries.image_paths AS image_paths, journals.name AS journal,
           entries.created_at AS created_at, entries.updated_at AS updated_at
    FROM entries JOIN journals ON journals.id = entries.journal_id
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
    /// Normalized tag names, sorted. Inline `#tags` plus explicitly added ones.
    #[serde(default)]
    pub tags: Vec<String>,
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
            journal: row.get("journal")?,
            tags: Vec::new(),
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    }

    /// Tags that are not written as `#tag` in the title or content.
    pub fn extra_tags(&self) -> Vec<String> {
        let inline = inline_tags(self.title.as_deref(), &self.content);
        self.tags
            .iter()
            .filter(|tag| !inline.contains(tag))
            .cloned()
            .collect()
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
    tags: &'a [String],
}

impl<'a> NewEntry<'a> {
    pub fn new(content: &'a str) -> Self {
        Self {
            content,
            title: None,
            journal: None,
            timestamp: None,
            audio_path: None,
            tags: &[],
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

    /// Tags besides the `#tags` written in the title and content.
    pub fn tags(mut self, tags: &'a [String]) -> Self {
        self.tags = tags;
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
    /// Entries must have all of these (normalized) tags.
    pub tags: Vec<String>,
}

impl EntryFilters {
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

/// A tag and the number of entries that have it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagCount {
    pub name: String,
    pub count: usize,
}

/// Distinct tags written inline in an entry's title and content.
fn inline_tags(title: Option<&str>, content: &str) -> Vec<String> {
    let mut tags = title.map(extract_tags).unwrap_or_default();
    for tag in extract_tags(content) {
        if !tags.contains(&tag) {
            tags.push(tag);
        }
    }
    tags
}

/// Inline tags of the text plus `extra` (validated and normalized), sorted and deduplicated.
fn resolve_tags(title: Option<&str>, content: &str, extra: &[String]) -> Result<Vec<String>> {
    let mut tags = inline_tags(title, content);
    for raw in extra {
        let tag = normalize_tag(raw).with_context(|| {
            format!("Invalid tag '{raw}': tags start with a letter and contain only letters, digits, '-' and '_'")
        })?;
        tags.push(tag);
    }
    tags.sort();
    tags.dedup();
    Ok(tags)
}

/// Replace the tags of an entry with `tags` (already normalized), creating missing ones.
fn write_entry_tags(conn: &Connection, entry_id: i64, tags: &[String]) -> Result<()> {
    conn.execute("DELETE FROM entry_tags WHERE entry_id = ?1", [entry_id])?;
    for tag in tags {
        conn.execute("INSERT OR IGNORE INTO tags (name) VALUES (?1)", [tag])?;
        conn.execute(
            "INSERT INTO entry_tags (entry_id, tag_id)
             SELECT ?1, id FROM tags WHERE name = ?2",
            params![entry_id, tag],
        )?;
    }
    Ok(())
}

fn delete_unused_tags(conn: &Connection) -> Result<()> {
    conn.execute(
        "DELETE FROM tags WHERE id NOT IN (SELECT tag_id FROM entry_tags)",
        [],
    )?;
    Ok(())
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
        let tags = resolve_tags(entry.title, entry.content, entry.tags)?;
        let tx = conn.unchecked_transaction()?;
        let journal_id = journals::ensure_journal(&tx, entry.journal.unwrap_or(DEFAULT_JOURNAL))?;

        tx.execute(
            "INSERT INTO entries (timestamp, title, content, journal_id, audio_path, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                timestamp,
                entry.title,
                entry.content,
                journal_id,
                entry.audio_path,
                now,
                now
            ],
        )?;

        let id = tx.last_insert_rowid();
        write_entry_tags(&tx, id, &tags)?;
        tx.commit()?;

        Ok(id)
    }

    pub fn get_entry(&self, id: i64) -> Result<Option<Entry>> {
        let conn = self.db.connection();
        let mut stmt = conn.prepare(&format!("{ENTRY_SELECT} WHERE entries.id = ?1"))?;

        let mut entries: Vec<Entry> = stmt
            .query_row([id], Entry::from_row)
            .optional()?
            .into_iter()
            .collect();
        self.load_tags(&mut entries)?;

        Ok(entries.pop())
    }

    pub fn list_entries(&self) -> Result<Vec<Entry>> {
        self.list_entries_with_order(SortOrder::NewestFirst)
    }

    pub fn list_entries_with_order(&self, order: SortOrder) -> Result<Vec<Entry>> {
        let query = format!(
            "{ENTRY_SELECT} ORDER BY entries.timestamp {}",
            order.as_sql()
        );

        self.collect_entries(&query, [])
    }

    /// Entries whose title or content contain `query` (case-insensitive, literal),
    /// narrowed by `filters`, newest first.
    pub fn search_entries(&self, query: &str, filters: &EntryFilters) -> Result<Vec<Entry>> {
        let query = query.to_lowercase();
        let mut entries = self.list_entries_filtered(filters)?;
        entries.retain(|entry| {
            entry.content.to_lowercase().contains(&query)
                || entry
                    .title
                    .as_deref()
                    .is_some_and(|title| title.to_lowercase().contains(&query))
        });
        Ok(entries)
    }

    pub fn delete_entry(&self, id: i64) -> Result<bool> {
        let conn = self.db.connection();

        let tx = conn.unchecked_transaction()?;
        let rows_affected = tx.execute("DELETE FROM entries WHERE id = ?1", [id])?;
        delete_unused_tags(&tx)?;
        tx.commit()?;

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
    /// * `extra_tags` - Tags to keep besides the `#tags` written in the title and content.
    ///   The entry's tags are replaced by these plus the inline ones.
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
        extra_tags: &[String],
    ) -> Result<bool> {
        let conn = self.db.connection();
        let now = Utc::now();
        let tags = resolve_tags(title, content, extra_tags)?;
        let tx = conn.unchecked_transaction()?;
        let journal_id = journals::ensure_journal(&tx, journal)?;

        let rows_affected = tx.execute(
            "UPDATE entries SET title = ?1, content = ?2, journal_id = ?3, timestamp = ?4, updated_at = ?5 WHERE id = ?6",
            params![title, content, journal_id, timestamp, now, id],
        )?;
        if rows_affected > 0 {
            write_entry_tags(&tx, id, &tags)?;
            delete_unused_tags(&tx)?;
        }
        tx.commit()?;

        Ok(rows_affected > 0)
    }

    /// Move an entry to `new_journal`, creating the journal if needed.
    /// Returns false (and creates nothing) if the entry does not exist.
    pub fn move_entry(&self, id: i64, new_journal: &str) -> Result<bool> {
        let conn = self.db.connection();
        let now = Utc::now();
        let tx = conn.unchecked_transaction()?;
        let journal_id = journals::ensure_journal(&tx, new_journal)?;

        let rows_affected = tx.execute(
            "UPDATE entries SET journal_id = ?1, updated_at = ?2 WHERE id = ?3",
            params![journal_id, now, id],
        )?;
        if rows_affected > 0 {
            tx.commit()?;
        }

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
        self.list_entries_filtered_limited(filters, order, None)
    }

    /// Like `list_entries_filtered_with_order`, but returns at most `limit` entries
    /// (the first ones in `order`), applied in SQL.
    pub fn list_entries_filtered_limited(
        &self,
        filters: &EntryFilters,
        order: SortOrder,
        limit: Option<usize>,
    ) -> Result<Vec<Entry>> {
        let (where_clause, mut params) = self.where_clause(filters)?;
        let mut query = format!(
            "{ENTRY_SELECT}{where_clause} ORDER BY entries.timestamp {}",
            order.as_sql()
        );
        if let Some(limit) = limit {
            query.push_str(" LIMIT ?");
            params.push(Box::new(i64::try_from(limit).unwrap_or(i64::MAX)));
        }

        self.collect_entries(&query, rusqlite::params_from_iter(params))
    }

    /// Number of entries matching `filters`.
    pub fn count_entries(&self, filters: &EntryFilters) -> Result<usize> {
        let (where_clause, params) = self.where_clause(filters)?;
        let count: i64 = self.db.connection().query_row(
            &format!("SELECT COUNT(*){ENTRY_FROM}{where_clause}"),
            rusqlite::params_from_iter(params),
            |row| row.get(0),
        )?;
        Ok(usize::try_from(count).unwrap_or_default())
    }

    /// SQL `WHERE` clause (with leading space, empty when unfiltered) and its parameters.
    fn where_clause(
        &self,
        filters: &EntryFilters,
    ) -> Result<(String, Vec<Box<dyn rusqlite::ToSql>>)> {
        let (start, end) = filters.timestamp_bounds(self.timezone)?;
        let mut conditions = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(start) = start {
            conditions.push("entries.timestamp >= ?");
            params.push(Box::new(start));
        }
        if let Some(end) = end {
            conditions.push("entries.timestamp < ?");
            params.push(Box::new(end));
        }
        if let Some(journal) = filters.journal.as_deref() {
            // Case-insensitive: the column is COLLATE NOCASE
            conditions.push("journals.name = ?");
            params.push(Box::new(journal.to_string()));
        }
        for tag in &filters.tags {
            conditions.push(
                "EXISTS (SELECT 1 FROM entry_tags et JOIN tags t ON t.id = et.tag_id
                         WHERE et.entry_id = entries.id AND t.name = ?)",
            );
            params.push(Box::new(tag.clone()));
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", conditions.join(" AND "))
        };
        Ok((where_clause, params))
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
        self.load_tags(&mut entries)?;

        Ok(entries)
    }

    /// Fill in `tags` for `entries` with a few queries instead of one per entry.
    fn load_tags(&self, entries: &mut [Entry]) -> Result<()> {
        // Stay well below SQLite's limit on bound parameters.
        const CHUNK: usize = 500;
        let conn = self.db.connection();

        for chunk in entries.chunks_mut(CHUNK) {
            let placeholders = vec!["?"; chunk.len()].join(",");
            let mut stmt = conn.prepare(&format!(
                "SELECT et.entry_id, t.name FROM entry_tags et
                 JOIN tags t ON t.id = et.tag_id
                 WHERE et.entry_id IN ({placeholders})
                 ORDER BY t.name"
            ))?;
            let ids = chunk.iter().map(|entry| entry.id);
            let rows = stmt.query_map(rusqlite::params_from_iter(ids), |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?;
            for row in rows {
                let (entry_id, name) = row?;
                if let Some(entry) = chunk.iter_mut().find(|entry| entry.id == entry_id) {
                    entry.tags.push(name);
                }
            }
        }
        Ok(())
    }

    /// All tags in use with their entry counts, most used first.
    pub fn list_tags(&self) -> Result<Vec<TagCount>> {
        let mut stmt = self.db.connection().prepare(
            "SELECT t.name, COUNT(*) AS uses FROM tags t
             JOIN entry_tags et ON et.tag_id = t.id
             GROUP BY t.id ORDER BY uses DESC, t.name ASC",
        )?;
        let tags = stmt
            .query_map([], |row| {
                Ok(TagCount {
                    name: row.get(0)?,
                    count: usize::try_from(row.get::<_, i64>(1)?).unwrap_or_default(),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(tags)
    }

    /// Add explicit tags to an entry. Returns false if the entry does not exist.
    pub fn add_tags(&self, id: i64, tags: &[String]) -> Result<bool> {
        let Some(entry) = self.get_entry(id)? else {
            return Ok(false);
        };
        let mut extra = entry.extra_tags();
        extra.extend_from_slice(tags);
        self.update_entry_with_metadata(
            id,
            entry.title.as_deref(),
            &entry.content,
            &entry.journal,
            entry.timestamp,
            &extra,
        )?;
        Ok(true)
    }

    /// Remove explicit tags from an entry. Tags written as `#tag` in the text
    /// can only be removed by editing the text. Returns false if the entry does not exist.
    pub fn remove_tags(&self, id: i64, tags: &[String]) -> Result<bool> {
        let Some(entry) = self.get_entry(id)? else {
            return Ok(false);
        };
        let inline = inline_tags(entry.title.as_deref(), &entry.content);
        let mut remove = Vec::new();
        for raw in tags {
            let tag = normalize_tag(raw).with_context(|| format!("Invalid tag '{raw}'"))?;
            if inline.contains(&tag) {
                anyhow::bail!(
                    "#{tag} is written in the text of entry {id}; edit the entry to remove it"
                );
            }
            if !entry.tags.contains(&tag) {
                anyhow::bail!("Entry {id} does not have the tag #{tag}");
            }
            remove.push(tag);
        }
        let extra: Vec<String> = entry
            .extra_tags()
            .into_iter()
            .filter(|tag| !remove.contains(tag))
            .collect();
        self.update_entry_with_metadata(
            id,
            entry.title.as_deref(),
            &entry.content,
            &entry.journal,
            entry.timestamp,
            &extra,
        )?;
        Ok(true)
    }

    /// Recompute every entry's inline tags from its text, keeping explicit
    /// tags. Returns the number of entries whose tags changed.
    pub fn reindex_tags(&self) -> Result<usize> {
        let entries = self.list_entries()?;
        let tx = self.db.connection().unchecked_transaction()?;
        let mut changed = 0;

        for entry in &entries {
            let tags = resolve_tags(entry.title.as_deref(), &entry.content, &entry.extra_tags())?;
            if tags != entry.tags {
                write_entry_tags(&tx, entry.id, &tags)?;
                changed += 1;
            }
        }
        delete_unused_tags(&tx)?;
        tx.commit()?;

        Ok(changed)
    }

    /// Rename a tag everywhere, rewriting `#old` in entry texts too. If `new`
    /// already exists the two are merged. Returns the number of entries changed.
    pub fn rename_tag(&self, old: &str, new: &str) -> Result<usize> {
        let old = normalize_tag(old).with_context(|| format!("Invalid tag '{old}'"))?;
        let new = normalize_tag(new).with_context(|| format!("Invalid tag '{new}'"))?;

        let filters = EntryFilters {
            tags: vec![old.clone()],
            ..EntryFilters::default()
        };
        let entries = self.list_entries_filtered(&filters)?;
        let tx = self.db.connection().unchecked_transaction()?;
        let now = Utc::now();

        for entry in &entries {
            let title = entry
                .title
                .as_deref()
                .map(|t| rename_in_text(t, &old, &new));
            let content = rename_in_text(&entry.content, &old, &new);
            let mut tags: Vec<String> = entry
                .tags
                .iter()
                .map(|tag| {
                    if *tag == old {
                        new.clone()
                    } else {
                        tag.clone()
                    }
                })
                .collect();
            tags.sort();
            tags.dedup();

            tx.execute(
                "UPDATE entries SET title = ?1, content = ?2, updated_at = ?3 WHERE id = ?4",
                params![title, content, now, entry.id],
            )?;
            write_entry_tags(&tx, entry.id, &tags)?;
        }
        delete_unused_tags(&tx)?;
        tx.commit()?;

        Ok(entries.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn journal() -> Journal {
        Journal::new(Database::new_with_path(":memory:").unwrap())
    }

    fn tag_filter(tags: &[&str]) -> EntryFilters {
        EntryFilters {
            tags: tags.iter().map(|tag| tag.to_string()).collect(),
            ..EntryFilters::default()
        }
    }

    #[test]
    fn inline_and_explicit_tags_are_stored_sorted() {
        let j = journal();
        let extra = vec!["#Urgent".to_string(), "pricing".to_string()];
        let id = j
            .create_entry(
                NewEntry::new("Met Ana about #pricing and #Q4")
                    .title(Some("Sync #work"))
                    .tags(&extra),
            )
            .unwrap();

        let entry = j.get_entry(id).unwrap().unwrap();
        assert_eq!(entry.tags, ["pricing", "q4", "urgent", "work"]);
        assert_eq!(entry.extra_tags(), ["urgent"]);
    }

    #[test]
    fn invalid_explicit_tag_is_rejected_and_nothing_is_saved() {
        let j = journal();
        let bad = vec!["two words".to_string()];
        assert!(j.create_entry(NewEntry::new("x").tags(&bad)).is_err());
        assert!(j.list_entries().unwrap().is_empty());
    }

    #[test]
    fn tag_filters_require_all_tags() {
        let j = journal();
        j.create_entry(NewEntry::new("#a #b one")).unwrap();
        j.create_entry(NewEntry::new("#a two")).unwrap();
        j.create_entry(NewEntry::new("none")).unwrap();

        let both = j.list_entries_filtered(&tag_filter(&["a", "b"])).unwrap();
        assert_eq!(both.len(), 1);
        assert_eq!(j.count_entries(&tag_filter(&["a"])).unwrap(), 2);
        assert_eq!(j.count_entries(&tag_filter(&["A"])).unwrap(), 2);
        assert_eq!(j.count_entries(&tag_filter(&["zzz"])).unwrap(), 0);
    }

    #[test]
    fn update_recomputes_inline_tags_and_keeps_extras() {
        let j = journal();
        let extra = vec!["keep".to_string()];
        let id = j
            .create_entry(NewEntry::new("#old text").tags(&extra))
            .unwrap();
        let entry = j.get_entry(id).unwrap().unwrap();

        j.update_entry_with_metadata(
            id,
            None,
            "#new text",
            &entry.journal,
            entry.timestamp,
            &entry.extra_tags(),
        )
        .unwrap();

        let entry = j.get_entry(id).unwrap().unwrap();
        assert_eq!(entry.tags, ["keep", "new"]);
        assert!(j.list_tags().unwrap().iter().all(|t| t.name != "old"));
    }

    #[test]
    fn deleting_an_entry_drops_its_links_and_unused_tags() {
        let j = journal();
        let a = j.create_entry(NewEntry::new("#shared #solo")).unwrap();
        j.create_entry(NewEntry::new("#shared")).unwrap();

        assert!(j.delete_entry(a).unwrap());

        assert_eq!(
            j.list_tags().unwrap(),
            [TagCount {
                name: "shared".into(),
                count: 1
            }]
        );
    }

    #[test]
    fn add_and_remove_explicit_tags() {
        let j = journal();
        let id = j.create_entry(NewEntry::new("#inline text")).unwrap();

        assert!(j.add_tags(id, &["extra".to_string()]).unwrap());
        assert_eq!(j.get_entry(id).unwrap().unwrap().tags, ["extra", "inline"]);

        let err = j.remove_tags(id, &["inline".to_string()]).unwrap_err();
        assert!(err.to_string().contains("edit the entry"), "{err}");
        let err = j.remove_tags(id, &["nope".to_string()]).unwrap_err();
        assert!(err.to_string().contains("does not have"), "{err}");

        assert!(j.remove_tags(id, &["extra".to_string()]).unwrap());
        assert_eq!(j.get_entry(id).unwrap().unwrap().tags, ["inline"]);
        assert!(!j.add_tags(999, &["x".to_string()]).unwrap());
    }

    #[test]
    fn rename_rewrites_text_and_merges_with_existing_tags() {
        let j = journal();
        let extra = vec!["draft".to_string()];
        let a = j
            .create_entry(NewEntry::new("about #draft and #final").tags(&extra))
            .unwrap();
        let b = j
            .create_entry(NewEntry::new("only #draft here").title(Some("#draft")))
            .unwrap();
        let other = j.create_entry(NewEntry::new("#other")).unwrap();

        assert_eq!(j.rename_tag("Draft", "final").unwrap(), 2);

        let a = j.get_entry(a).unwrap().unwrap();
        assert_eq!(a.content, "about #final and #final");
        assert_eq!(a.tags, ["final"]);
        let b = j.get_entry(b).unwrap().unwrap();
        assert_eq!(b.content, "only #final here");
        assert_eq!(b.title.as_deref(), Some("#final"));
        assert_eq!(b.tags, ["final"]);
        assert_eq!(j.get_entry(other).unwrap().unwrap().tags, ["other"]);
        assert_eq!(
            j.list_tags()
                .unwrap()
                .iter()
                .map(|t| t.count)
                .collect::<Vec<_>>(),
            [2, 1]
        );
    }

    #[test]
    fn reindex_tags_entries_that_predate_tagging_and_keeps_extras() {
        let j = journal();
        let extra = vec!["keep".to_string()];
        let tagged = j
            .create_entry(NewEntry::new("#fresh").tags(&extra))
            .unwrap();
        let old = j
            .create_entry(NewEntry::new("written before #tags"))
            .unwrap();
        // Simulate entries saved before tagging existed
        j.db.connection()
            .execute("DELETE FROM entry_tags WHERE entry_id = ?1", [old])
            .unwrap();

        assert_eq!(j.reindex_tags().unwrap(), 1);
        assert_eq!(j.get_entry(old).unwrap().unwrap().tags, ["tags"]);
        assert_eq!(
            j.get_entry(tagged).unwrap().unwrap().tags,
            ["fresh", "keep"]
        );
        assert_eq!(j.reindex_tags().unwrap(), 0);
    }

    #[test]
    fn many_entries_load_their_tags() {
        let j = journal();
        for i in 0..1200 {
            j.create_entry(NewEntry::new(&format!("entry {i} #t{}", i % 3)))
                .unwrap();
        }
        let entries = j.list_entries().unwrap();
        assert_eq!(entries.len(), 1200);
        assert!(entries.iter().all(|entry| entry.tags.len() == 1));
    }

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
