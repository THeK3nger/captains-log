# Captain's Log - Development Notes

## Project Overview
A terminal-based journaling application written in Rust with SQLite storage.

## Current Status
✅ **Phase 1 Complete** - Core functionality implemented and tested  
✅ **Phase 2 Complete** - Advanced features implemented and tested

## Build & Test Commands

### Build the project
```bash
cargo build
```

### Run the application
```bash
# Quick entry creation
./target/debug/cl "Your journal entry content"

# List all entries
./target/debug/cl list

# Show specific entry
./target/debug/cl show <id>

# Search entries
./target/debug/cl search "<query>"

# Delete entry  
./target/debug/cl delete <id>

# Edit entry (opens external editor)
./target/debug/cl edit <id>

# List entries with date filtering
./target/debug/cl list --since 2025-01-01
./target/debug/cl list --until 2025-12-31
./target/debug/cl list --date 2025-09-09

# Calendar view
./target/debug/cl calendar
./target/debug/cl calendar --year 2024 --month 12

# Show help
./target/debug/cl --help
```

### Development Commands
```bash
# Build and run tests (when implemented)
cargo test

# Check for linting issues
cargo clippy

# Format code
cargo fmt
```

## Project Structure
```
src/
├── main.rs              # CLI entry point and argument parsing
├── cli/
│   └── mod.rs           # Command handling and help text
├── database/
│   └── mod.rs           # SQLite connection and migrations
└── journal/
    └── mod.rs           # Entry model and CRUD operations
```

## Database
- Location: `~/.local/share/captains-log/journal.db`  
- Schema: entries table with id, timestamp, title, content, audio_path, image_paths, created_at, updated_at
- Automatic migrations on first run

## Features Implemented

### Phase 1 - Core Features
- [x] Quick entry creation from command line
- [x] Entry listing with timestamps
- [x] Individual entry viewing
- [x] Text-based search across content
- [x] Entry deletion
- [x] SQLite storage with proper schema

### Phase 2 - Advanced Features
- [x] Calendar view for entries by date
- [x] External editor integration (respects $EDITOR environment variable, defaults to nvim)
- [x] Rich terminal formatting with colors and improved layout
- [x] Date-based filtering (--date, --since, --until options for list command)
- [x] Entry editing capabilities

## Future Enhancement Ideas
- [ ] Tagging system for entries
- [ ] Export functionality (markdown, JSON)
- [ ] Import from other journal formats
- [ ] Full-text search improvements
- [ ] Attachment support (images, files)

## Testing Notes
All functionality has been manually tested:

### Phase 1 Testing
1. Entry creation works correctly
2. Database persistence confirmed
3. Search functionality operational
4. List and show commands working
5. CLI help system functional

### Phase 2 Testing
1. Calendar view displays correctly with entry indicators
2. Date filtering (--date, --since, --until) works properly
3. External editor integration functional (uses $EDITOR or defaults to nvim)
4. Entry editing updates database correctly
5. Rich terminal formatting enhances user experience
6. All new commands properly documented in help system