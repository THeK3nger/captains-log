# Database Model

This document outlines the database model for CaptainLog, detailing the structure and relationships of the various entities involved in the application.

## Database Schema

The database contains the `entries` table, plus `tags` and `entry_tags` for tagging (see below).

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

## Tags

```sql
CREATE TABLE tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE COLLATE NOCASE
);
CREATE TABLE entry_tags (
    entry_id INTEGER NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
    tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (entry_id, tag_id)
);
CREATE INDEX idx_entry_tags_tag ON entry_tags(tag_id, entry_id);
```

- Tag names are stored lowercase, without the `#`. A valid name starts with a letter and contains only letters, digits, `-` and `_`.
- An entry's tags are the `#tags` written in its title and content, plus any *extra* tags added explicitly (`--tag`, `cl tags add`, the frontmatter `tags:` list in the editor). Saving an entry recomputes the inline part from the text, so the text is the source of truth for inline tags.
- `#tag` is recognized at the start of the text or after whitespace or an opening bracket/quote, and must start with a letter. `#123`, `# Heading`, URL fragments and anything in code spans or fenced code blocks are ignored.
- Tags no entry uses any more are deleted. Cascading deletes need `PRAGMA foreign_keys = ON`, which is set on every connection.
- Entries that existed before the migration are not scanned: they get their inline tags the next time they are saved, or all at once with `cl tags reindex`.

## Schema Migrations

CaptainLog applies pending schema migrations automatically when it opens the database. Migrations are applied in order, and SQLite's `PRAGMA user_version` records the latest completed schema version.

Each migration and its version update run in the same transaction. If a migration fails, the database remains at the last successfully applied version. CaptainLog also rejects databases whose schema version is newer than the application supports, preventing an older application from opening an incompatible database.
