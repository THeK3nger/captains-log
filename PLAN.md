# Captain's Log - Rust Journaling Application

## Overview
A terminal-based journaling application written in Rust that stores entries in SQLite with support for text, audio recordings, and images.

## Core Features

### Entry Creation
- Quick entry via command: `cl "This is an entry of my journal"`
- Interactive mode for longer entries using default editor (Neovim)
- Optional title specification
- Automatic timestamp generation
- Support for multimedia content (text, audio, images)

### Data Storage
- SQLite database backend
- Entry schema:
  - `id` (PRIMARY KEY)
  - `timestamp` (DATETIME)
  - `title` (TEXT, optional)
  - `content` (TEXT)
  - `audio_path` (TEXT, optional)
  - `image_paths` (TEXT, JSON array, optional)
  - `created_at` (DATETIME)
  - `updated_at` (DATETIME)

### Entry Management
- List all entries with pagination
- Calendar view showing entries by date
- Date-based filtering and queries
- Full-text search across entry content
- Entry editing and deletion

### User Interface
- Terminal-based interface using libraries like `clap` for CLI parsing
- Rich formatting with `crossterm` or similar for enhanced terminal output
- Calendar visualization for date-based browsing

## Technical Architecture

### Dependencies
- `clap` - Command-line argument parsing ✅ Implemented
- `rusqlite` - SQLite database interface ✅ Implemented
- `chrono` - Date and time handling ✅ Implemented
- `serde` - Serialization/deserialization ✅ Implemented
- `colored` - Terminal color formatting ✅ Implemented
- `directories` - User directory management ✅ Implemented
- `anyhow` - Error handling ✅ Implemented

### Project Structure
```
src/
├── main.rs              # CLI entry point
├── lib.rs               # Library exports
├── cli/
│   ├── mod.rs
│   ├── commands.rs      # Command definitions
│   └── args.rs          # Argument parsing
├── database/
│   ├── mod.rs
│   ├── connection.rs    # Database connection
│   ├── migrations.rs    # Schema migrations
│   └── models.rs        # Entry model
├── journal/
│   ├── mod.rs
│   ├── entry.rs         # Entry operations
│   ├── search.rs        # Search functionality
│   └── calendar.rs      # Calendar view
├── media/
│   ├── mod.rs
│   ├── audio.rs         # Audio handling
│   └── images.rs        # Image handling
└── utils/
    ├── mod.rs
    ├── editor.rs        # External editor integration
    └── config.rs        # Configuration management
```

## Command Interface

### Basic Commands ✅ Implemented
- `cl "Entry content"` - Quick entry creation ✅
- `cl list` - List all entries ✅
- `cl list --date 2025-01-01` - List entries from specific date ✅
- `cl list --since 2025-01-01 --until 2025-12-31` - Date range filtering ✅
- `cl show <id>` - Show specific entry ✅
- `cl edit <id>` - Edit entry in external editor ✅
- `cl calendar` - Calendar view ✅
- `cl calendar --year 2024 --month 12` - Calendar for specific month ✅
- `cl search <query>` - Search entries ✅
- `cl delete <id>` - Delete entry ✅

### Advanced Commands
- `cl --date "2024-01-15" "Entry content"` - Entry with specific date
- `cl --audio <path>` - Attach audio file
- `cl --image <path>` - Attach image file
- `cl export --format <json|markdown>` - Export entries

## Database Schema

```sql
CREATE TABLE entries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
    title TEXT,
    content TEXT NOT NULL,
    audio_path TEXT,
    image_paths TEXT, -- JSON array of image file paths
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_entries_timestamp ON entries(timestamp);
CREATE INDEX idx_entries_title ON entries(title);
CREATE VIRTUAL TABLE entries_fts USING fts5(content, title);
```

## Future Enhancements

### Hook System
- Pre-entry hooks (validation, formatting)
- Post-entry hooks (notifications, backups, integrations)
- Configuration via `~/.config/captainlog/hooks.toml`
- Hook types:
  - Shell commands
  - HTTP webhooks  
  - Custom Rust plugins

### Additional Features
- Entry templates
- Tag system
- Encryption for sensitive entries
- Cloud sync capabilities
- Import/export from other journaling apps
- Rich text formatting support
- Entry linking and references

## Configuration
- Configuration file: `~/.config/captainlog/config.toml`
- Database location: `~/.local/share/captainlog/journal.db`
- Media files: `~/.local/share/captainlog/media/`
- Settings:
  - Default editor
  - Date format preferences
  - Calendar view options
  - Search preferences

## Development Phases

### Phase 1: Core Functionality ✅ COMPLETED
1. ✅ CLI structure and argument parsing with clap
2. ✅ SQLite database setup and basic CRUD operations
3. ✅ Text entry creation and retrieval
4. ✅ Basic listing and search

### Phase 2: Enhanced Interface ✅ COMPLETED
1. ✅ Calendar view implementation
2. ✅ Rich terminal formatting with colors
3. ✅ External editor integration (respects $EDITOR env var)
4. ✅ Date filtering and queries (--date, --since, --until)
5. ✅ Entry editing functionality

**Current Status**: Phase 2 is fully implemented and tested. The application now supports:
- Quick entry creation: `cl "Your journal entry"`
- Entry listing: `cl list` (with optional date filtering)
- Entry viewing: `cl show <id>` (with enhanced formatting)
- Entry searching: `cl search <query>` (with enhanced formatting)
- Entry deletion: `cl delete <id>`
- Entry editing: `cl edit <id>` (opens external editor)
- Calendar view: `cl calendar` (shows entries by date)
- Date-based filtering: `cl list --since 2025-01-01 --until 2025-12-31`
- Rich terminal colors and improved visual hierarchy
- Database stored in `~/.local/share/captains-log/journal.db`

### Phase 3: Multimedia Support
1. Audio file attachment and playback
2. Image file attachment and viewing
3. File management and cleanup

### Phase 4: Advanced Features
1. Full-text search with FTS5
2. Entry templates
3. Export functionality
4. Hook system foundation

## Testing Strategy
- Unit tests for core functionality
- Integration tests for database operations
- CLI command testing with temporary databases
- Cross-platform compatibility testing