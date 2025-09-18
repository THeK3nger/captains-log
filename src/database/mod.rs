use crate::config::Config;
use anyhow::{Context, Result};
use rusqlite::Connection;
use std::fs;

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn new(config: &Config) -> Result<Self> {
        let db_path = config.get_database_path()?;

        Self::new_with_path(&db_path)
    }

    pub fn new_with_path<P: AsRef<std::path::Path>>(db_path: P) -> Result<Self> {
        let db_path = db_path.as_ref();

        // Create directory if it doesn't exist
        if let Some(parent) = db_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(&db_path)
            .with_context(|| format!("Failed to open database at {:?}", db_path))?;

        let mut db = Database { conn };
        db.run_migrations()?;

        Ok(db)
    }

    fn run_migrations(&mut self) -> Result<()> {
        // Create entries table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS entries (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
                title TEXT,
                content TEXT NOT NULL,
                audio_path TEXT,
                image_paths TEXT,
                journal TEXT DEFAULT 'Personal',
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        // Create indexes for better performance
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_entries_timestamp ON entries(timestamp)",
            [],
        )?;

        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_entries_created_at ON entries(created_at)",
            [],
        )?;

        Ok(())
    }

    pub fn connection(&self) -> &Connection {
        &self.conn
    }
}
