# Captain's Log - Development Notes

## Project Overview
A terminal-based journaling application written in Rust with SQLite storage.

## Current Status
✅ **Phase 1 Complete** - Core functionality implemented and tested

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
- [x] Quick entry creation from command line
- [x] Entry listing with timestamps
- [x] Individual entry viewing
- [x] Text-based search across content
- [x] Entry deletion
- [x] SQLite storage with proper schema

## Next Phase Features
- [ ] Calendar view for entries by date
- [ ] External editor integration (Neovim)
- [ ] Rich terminal formatting
- [ ] Date-based filtering
- [ ] Entry editing capabilities

## Testing Notes
All basic functionality has been manually tested:
1. Entry creation works correctly
2. Database persistence confirmed
3. Search functionality operational
4. List and show commands working
5. CLI help system functional