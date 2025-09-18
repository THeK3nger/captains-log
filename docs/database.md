# Database Model

This document outlines the database model for CaptainLog, detailing the structure and relationships of the various entities involved in the application.

## Database Schema

The database schema consists of the following tables:

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
)
```
Yep. At the moment there is only one table.
