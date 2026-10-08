use crate::config::Config;
use anyhow::{Context, Result};
use rusqlite::Connection;
use std::fs;

/// Schema migrations, applied in order. The index of a migration plus one is
/// the `PRAGMA user_version` the database has after it ran.
///
/// Append only: never edit or reorder a migration that has been released.
const MIGRATIONS: &[&str] = &[
    // 1: initial schema. Written with IF NOT EXISTS so it is also safe on
    // databases created before versioning existed (user_version 0).
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
    );
    CREATE INDEX IF NOT EXISTS idx_entries_timestamp ON entries(timestamp);
    CREATE INDEX IF NOT EXISTS idx_entries_created_at ON entries(created_at);
    CREATE INDEX IF NOT EXISTS idx_entries_journal_timestamp ON entries(journal, timestamp);",
];

/// Bring the database up to the latest schema version. Each migration runs in
/// its own transaction together with its version bump, so a failure leaves the
/// database at the last fully applied version.
fn migrate(conn: &mut Connection) -> Result<()> {
    let current: u32 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .context("Failed to read database schema version")?;

    let latest = MIGRATIONS.len() as u32;
    if current > latest {
        anyhow::bail!(
            "Database schema version {} is newer than this version of captains-log supports ({}). Please upgrade the application.",
            current,
            latest
        );
    }

    for (index, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let version = index as u32 + 1;
        let tx = conn.transaction()?;
        tx.execute_batch(sql)
            .with_context(|| format!("Failed to apply database migration {version}"))?;
        tx.pragma_update(None, "user_version", version)?;
        tx.commit()?;
    }

    Ok(())
}

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

        let conn = Connection::open(db_path)
            .with_context(|| format!("Failed to open database at {:?}", db_path))?;

        let mut db = Database { conn };
        db.run_migrations()?;

        Ok(db)
    }

    fn run_migrations(&mut self) -> Result<()> {
        migrate(&mut self.conn)
    }

    pub fn connection(&self) -> &Connection {
        &self.conn
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(conn: &Connection) -> u32 {
        conn.pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn fresh_database_gets_latest_version() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();

        assert_eq!(version(&conn), MIGRATIONS.len() as u32);
        conn.execute("INSERT INTO entries (content) VALUES ('x')", [])
            .unwrap();
    }

    #[test]
    fn unversioned_existing_database_keeps_its_data() {
        let mut conn = Connection::open_in_memory().unwrap();
        // Simulate a database created before versioning (user_version = 0)
        conn.execute_batch(MIGRATIONS[0]).unwrap();
        conn.execute("INSERT INTO entries (content) VALUES ('old')", [])
            .unwrap();
        assert_eq!(version(&conn), 0);

        migrate(&mut conn).unwrap();

        assert_eq!(version(&conn), MIGRATIONS.len() as u32);
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM entries", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn migrating_twice_is_a_no_op() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();
        migrate(&mut conn).unwrap();
        assert_eq!(version(&conn), MIGRATIONS.len() as u32);
    }

    #[test]
    fn newer_database_is_rejected() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", MIGRATIONS.len() as u32 + 1)
            .unwrap();

        let err = migrate(&mut conn).unwrap_err().to_string();
        assert!(err.contains("newer"), "unexpected error: {err}");
    }
}
