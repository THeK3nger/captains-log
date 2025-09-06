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

        let entry_iter = stmt.query_map([id], |row| Entry::from_row(row))?;

        for entry in entry_iter {
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

        let entry_iter = stmt.query_map([], |row| Entry::from_row(row))?;

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

        let entry_iter = stmt.query_map([&search_pattern], |row| Entry::from_row(row))?;

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
}
