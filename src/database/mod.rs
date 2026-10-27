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
    // 2: tags. Names are unique ignoring case; links go away with their entry.
    "CREATE TABLE tags (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL UNIQUE COLLATE NOCASE
    );
    CREATE TABLE entry_tags (
        entry_id INTEGER NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
        tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
        PRIMARY KEY (entry_id, tag_id)
    );
    CREATE INDEX idx_entry_tags_tag ON entry_tags(tag_id, entry_id);",
    // 3: journals become a table with optional metadata, and entries point to
    // it by id. Names are unique ignoring case, so "work" and "Work" merge into
    // whichever was used first. Missing or blank journals become "Personal".
    // SQLite cannot change a column into a foreign key, so `entries` is rebuilt
    // (foreign keys are off during migrations, see `migrate`).
    "CREATE TABLE journals (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL UNIQUE COLLATE NOCASE,
        description TEXT,
        color TEXT,
        created_at DATETIME DEFAULT CURRENT_TIMESTAMP
    );
    INSERT OR IGNORE INTO journals (name)
        SELECT COALESCE(NULLIF(TRIM(journal), ''), 'Personal') AS name
        FROM entries GROUP BY name ORDER BY MIN(id);
    CREATE TABLE entries_new (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
        title TEXT,
        content TEXT NOT NULL,
        audio_path TEXT,
        image_paths TEXT,
        journal_id INTEGER NOT NULL REFERENCES journals(id),
        created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
        updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
    );
    INSERT INTO entries_new (id, timestamp, title, content, audio_path, image_paths, journal_id, created_at, updated_at)
        SELECT e.id, e.timestamp, e.title, e.content, e.audio_path, e.image_paths, j.id, e.created_at, e.updated_at
        FROM entries e
        JOIN journals j ON j.name = COALESCE(NULLIF(TRIM(e.journal), ''), 'Personal');
    DROP TABLE entries;
    ALTER TABLE entries_new RENAME TO entries;
    CREATE INDEX idx_entries_timestamp ON entries(timestamp);
    CREATE INDEX idx_entries_created_at ON entries(created_at);
    CREATE INDEX idx_entries_journal_timestamp ON entries(journal_id, timestamp);",
];

/// Bring the database up to the latest schema version. Each migration runs in
/// its own transaction together with its version bump, so a failure leaves the
/// database at the last fully applied version.
///
/// Foreign keys are switched off while migrating so tables can be rebuilt
/// (dropping `entries` would otherwise cascade into `entry_tags`); each
/// migration is checked for dangling references before it commits.
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
    if current == latest {
        return Ok(());
    }

    // Cannot be changed inside a transaction, so it is toggled around them.
    let foreign_keys: bool = conn.pragma_query_value(None, "foreign_keys", |row| row.get(0))?;
    conn.pragma_update(None, "foreign_keys", false)?;
    let result = apply_migrations(conn, current);
    conn.pragma_update(None, "foreign_keys", foreign_keys)?;
    result
}

fn apply_migrations(conn: &mut Connection, current: u32) -> Result<()> {
    for (index, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let version = index as u32 + 1;
        let tx = conn.transaction()?;
        tx.execute_batch(sql)
            .with_context(|| format!("Failed to apply database migration {version}"))?;
        let violations: i64 =
            tx.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
                row.get(0)
            })?;
        if violations > 0 {
            anyhow::bail!(
                "Database migration {version} left {violations} broken references; nothing was changed"
            );
        }
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
        // Off by default in SQLite; needed for ON DELETE CASCADE on entry_tags.
        conn.pragma_update(None, "foreign_keys", true)?;

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
        conn.execute("INSERT INTO journals (name) VALUES ('Personal')", [])
            .unwrap();
        conn.execute(
            "INSERT INTO entries (content, journal_id) VALUES ('x', 1)",
            [],
        )
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
    fn journals_migration_keeps_entries_and_their_tags() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", true).unwrap();
        conn.execute_batch(MIGRATIONS[0]).unwrap();
        conn.execute_batch(MIGRATIONS[1]).unwrap();
        conn.pragma_update(None, "user_version", 2).unwrap();
        conn.execute_batch(
            "INSERT INTO entries (id, content, journal) VALUES
                (1, 'a', 'Work'), (2, 'b', 'work'), (3, 'c', NULL),
                (4, 'd', '  '), (5, 'e', 'Personal'), (6, 'f', 'Ship Ops');
             INSERT INTO tags (id, name) VALUES (1, 'pricing');
             INSERT INTO entry_tags (entry_id, tag_id) VALUES (1, 1), (6, 1);",
        )
        .unwrap();

        migrate(&mut conn).unwrap();

        assert_eq!(version(&conn), MIGRATIONS.len() as u32);
        let journals: Vec<(i64, String)> = conn
            .prepare(
                "SELECT e.id, j.name FROM entries e
                 JOIN journals j ON j.id = e.journal_id ORDER BY e.id",
            )
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(
            journals,
            vec![
                (1, "Work".to_string()),
                (2, "Work".to_string()),
                (3, "Personal".to_string()),
                (4, "Personal".to_string()),
                (5, "Personal".to_string()),
                (6, "Ship Ops".to_string()),
            ]
        );
        let tagged: i64 = conn
            .query_row("SELECT COUNT(*) FROM entry_tags", [], |row| row.get(0))
            .unwrap();
        assert_eq!(tagged, 2, "rebuilding entries must not drop its tags");
        let foreign_keys: bool = conn
            .pragma_query_value(None, "foreign_keys", |row| row.get(0))
            .unwrap();
        assert!(foreign_keys, "foreign keys are switched back on");

        // Cascades still point at the rebuilt table
        conn.execute("DELETE FROM entries WHERE id = 1", [])
            .unwrap();
        let tagged: i64 = conn
            .query_row("SELECT COUNT(*) FROM entry_tags", [], |row| row.get(0))
            .unwrap();
        assert_eq!(tagged, 1);
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
