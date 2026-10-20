# Database Model

This document outlines the database model for CaptainLog, detailing the structure and relationships of the various entities involved in the application.

## Database Schema

The database currently contains one table:

```sql
CREATE TABLE IF NOT EXISTS entries (
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

CREATE INDEX IF NOT EXISTS idx_entries_timestamp
    ON entries(timestamp);
CREATE INDEX IF NOT EXISTS idx_entries_created_at
    ON entries(created_at);
CREATE INDEX IF NOT EXISTS idx_entries_journal_timestamp
    ON entries(journal, timestamp);
```

## Schema Migrations

CaptainLog applies pending schema migrations automatically when it opens the database. Migrations are applied in order, and SQLite's `PRAGMA user_version` records the latest completed schema version.

Each migration and its version update run in the same transaction. If a migration fails, the database remains at the last successfully applied version. CaptainLog also rejects databases whose schema version is newer than the application supports, preventing an older application from opening an incompatible database.
