use std::fmt;

use crate::database::Database;
use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{Row, params};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Entry {
    pub id: i64,
    pub timestamp: DateTime<Utc>,
    pub title: Option<String>,
    pub content: String,
    pub audio_path: Option<String>,
    pub image_paths: Vec<String>,
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
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    }

    pub fn get_summary(&self, summary_size: usize) -> String {
        let content_preview = if self.content.len() > summary_size {
            format!("{}...", &self.content[..summary_size])
        } else {
            self.content.clone()
        };
        let title = self.title.as_deref();
        if title.is_some() {
            format!(
                "[{}] {} - {} - {}",
                self.id,
                self.timestamp.format("%Y-%m-%d %H:%M"),
                title.unwrap(),
                content_preview
            )
        } else {
            format!(
                "[{}] {} - {}",
                self.id,
                self.timestamp.format("%Y-%m-%d %H:%M"),
                content_preview
            )
        }
    }
}

impl fmt::Display for Entry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}] {} - {}\n{}\n",
            self.id,
            self.timestamp.format("%Y-%m-%d %H:%M"),
            self.title.as_deref().unwrap_or("Untitled"),
            self.content
        )
    }
}

pub struct Journal {
    db: Database,
}

impl Journal {
    pub fn new(db: Database) -> Self {
        Journal { db }
    }

    pub fn create_entry(&self, title: Option<&str>, content: &str) -> Result<i64> {
        let conn = self.db.connection();
        let now = Utc::now();

        conn.execute(
            "INSERT INTO entries (timestamp, title, content, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![now, title, content, now, now],
        )?;

        Ok(conn.last_insert_rowid())
    }

    pub fn get_entry(&self, id: i64) -> Result<Option<Entry>> {
        let conn = self.db.connection();

        let mut stmt = conn.prepare(
            "SELECT id, timestamp, title, content, audio_path, image_paths,
                    created_at, updated_at
             FROM entries WHERE id = ?1",
        )?;

        let mut entry_iter = stmt.query_map([id], Entry::from_row)?;

        if let Some(entry) = entry_iter.next() {
            return Ok(Some(entry?));
        }

        Ok(None)
    }

    pub fn list_entries(&self) -> Result<Vec<Entry>> {
        let conn = self.db.connection();

        let mut stmt = conn.prepare(
            "SELECT id, timestamp, title, content, audio_path, image_paths,
                    created_at, updated_at
             FROM entries ORDER BY created_at DESC",
        )?;

        let entry_iter = stmt.query_map([], Entry::from_row)?;

        let mut entries = Vec::new();
        for entry in entry_iter {
            entries.push(entry?);
        }

        Ok(entries)
    }

    pub fn search_entries(&self, query: &str) -> Result<Vec<Entry>> {
        let conn = self.db.connection();
        let search_pattern = format!("%{}%", query);

        let mut stmt = conn.prepare(
            "SELECT id, timestamp, title, content, audio_path, image_paths,
                    created_at, updated_at
             FROM entries
             WHERE content LIKE ?1 OR title LIKE ?1
             ORDER BY created_at DESC",
        )?;

        let entry_iter = stmt.query_map([&search_pattern], Entry::from_row)?;

        let mut entries = Vec::new();
        for entry in entry_iter {
            entries.push(entry?);
        }

        Ok(entries)
    }

    pub fn delete_entry(&self, id: i64) -> Result<bool> {
        let conn = self.db.connection();

        let rows_affected = conn.execute("DELETE FROM entries WHERE id = ?1", [id])?;

        Ok(rows_affected > 0)
    }

    pub fn update_entry(&self, id: i64, title: Option<&str>, content: &str) -> Result<bool> {
        let conn = self.db.connection();
        let now = Utc::now();

        let rows_affected = conn.execute(
            "UPDATE entries SET title = ?1, content = ?2, updated_at = ?3 WHERE id = ?4",
            params![title, content, now, id],
        )?;

        Ok(rows_affected > 0)
    }

    pub fn list_entries_filtered(
        &self,
        date: Option<&str>,
        since: Option<&str>,
        until: Option<&str>,
    ) -> Result<Vec<Entry>> {
        let conn = self.db.connection();
        let mut query = "SELECT id, timestamp, title, content, audio_path, image_paths, created_at, updated_at FROM entries".to_string();
        let mut conditions = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(date_str) = date {
            let date = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
                .map_err(|_| anyhow::anyhow!("Invalid date format. Use YYYY-MM-DD"))?;
            conditions.push("DATE(timestamp) = ?");
            params.push(Box::new(date.to_string()));
        }

        if let Some(since_str) = since {
            let since_date = chrono::NaiveDate::parse_from_str(since_str, "%Y-%m-%d")
                .map_err(|_| anyhow::anyhow!("Invalid since date format. Use YYYY-MM-DD"))?;
            conditions.push("DATE(timestamp) >= ?");
            params.push(Box::new(since_date.to_string()));
        }

        if let Some(until_str) = until {
            let until_date = chrono::NaiveDate::parse_from_str(until_str, "%Y-%m-%d")
                .map_err(|_| anyhow::anyhow!("Invalid until date format. Use YYYY-MM-DD"))?;
            conditions.push("DATE(timestamp) <= ?");
            params.push(Box::new(until_date.to_string()));
        }

        if !conditions.is_empty() {
            query.push_str(" WHERE ");
            query.push_str(&conditions.join(" AND "));
        }

        query.push_str(" ORDER BY created_at DESC");

        let mut stmt = conn.prepare(&query)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let entry_iter = stmt.query_map(param_refs.as_slice(), Entry::from_row)?;

        let mut entries = Vec::new();
        for entry in entry_iter {
            entries.push(entry?);
        }

        Ok(entries)
    }

    pub fn list_entries_for_month(&self, year: i32, month: u32) -> Result<Vec<Entry>> {
        let conn = self.db.connection();

        let mut stmt = conn.prepare(
            "SELECT id, timestamp, title, content, audio_path, image_paths,
                    created_at, updated_at
             FROM entries
             WHERE strftime('%Y', timestamp) = ?1 AND strftime('%m', timestamp) = ?2
             ORDER BY timestamp ASC",
        )?;

        let month_str = format!("{:02}", month);
        let entry_iter = stmt.query_map([year.to_string(), month_str], Entry::from_row)?;

        let mut entries = Vec::new();
        for entry in entry_iter {
            entries.push(entry?);
        }

        Ok(entries)
    }
}
