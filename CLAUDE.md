# Captain's Log - Development Notes

## Project Overview
A terminal-based journaling application written in Rust with SQLite storage.

## Current Status
✅ **Phase 1 Complete** - Core functionality implemented and tested
✅ **Phase 2 Complete** - Advanced features implemented and tested
✅ **Configuration System** - Global configuration file support added
✅ **Journal Categories** - Support for organizing entries by journal type
✅ **Export System** - JSON, Markdown, and ORG export functionality with filtering support and optimized timestamp ordering
✅ **Database Override** - CLI parameter to override database location for any command
✅ **External Editor Entry Creation** - New command to create entries directly using external editor
✅ **Stardate Mode Integration** - Consistent stardate formatting across all entry display commands

## Build & Test Commands

### Build the project
```bash
cargo build
```

### Run the application
```bash
# Quick entry creation
./target/debug/cl "Your journal entry content"

# Quick entry creation with journal category
./target/debug/cl "Your journal entry content" --journal Work
./target/debug/cl "Personal note" --journal Personal

# List all entries
./target/debug/cl list

# List entries by journal category
./target/debug/cl list --journal Work
./target/debug/cl list --journal Personal

# Show specific entry
./target/debug/cl show <id>

# Search entries
./target/debug/cl search "<query>"

# Delete entry
./target/debug/cl delete <id>

# Move entry to different journal
./target/debug/cl move <id> <target_journal>

# Create new entry (opens external editor)
./target/debug/cl new
./target/debug/cl new --journal Work

# Edit entry (opens external editor)
./target/debug/cl edit <id>

# List entries with date filtering
./target/debug/cl list --since 2025-01-01
./target/debug/cl list --until 2025-12-31
./target/debug/cl list --date 2025-09-09

# Combine filters (date and journal)
./target/debug/cl list --journal Work --since 2025-01-01
./target/debug/cl list --journal Personal --date 2025-09-09

# Calendar view
./target/debug/cl calendar
./target/debug/cl calendar --year 2024 --month 12

# Configuration management
./target/debug/cl config show
./target/debug/cl config set editor.command "code --wait"
./target/debug/cl config set database.path "/custom/path/journal.db"
./target/debug/cl config set display.stardate_mode true
./target/debug/cl config path

# Export entries to JSON, Markdown, or ORG format
./target/debug/cl export --output entries.json --format json
./target/debug/cl export --output entries.md --format markdown
./target/debug/cl export --output entries.org --format org
./target/debug/cl export --output work_entries.json --journal Work --format json
./target/debug/cl export --output work_entries.md --journal Work --format markdown
./target/debug/cl export --output work_entries.org --journal Work --format org
./target/debug/cl export --output recent.json --since 2025-09-01 --format json
./target/debug/cl export --output recent.md --since 2025-09-01 --format markdown
./target/debug/cl export --output filtered.org --journal Personal --since 2025-09-01 --until 2025-09-30 --format org

# Move entries between journals
./target/debug/cl move 123 Work
./target/debug/cl move 456 Personal
./target/debug/cl move 789 Projects

# Override database location (global parameter for any command)
./target/debug/cl -f "path/to/custom.db" "Entry with custom database"
./target/debug/cl --file "/tmp/temp.db" list
./target/debug/cl -f "backup.db" export --output backup.json --format json

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
│   ├── mod.rs           # Command handling and help text
│   ├── formatting.rs    # Markdown rendering utilities
│   └── stardate.rs      # Stardate conversion system
├── config/
│   └── mod.rs           # Configuration management and file handling
├── database/
│   └── mod.rs           # SQLite connection and migrations
├── export/
│   └── mod.rs           # Export functionality (JSON and ORG formats)
└── journal/
    └── mod.rs           # Entry model and CRUD operations
```

## Database
- Default Location: `~/.local/share/captains-log/journal.db`
- Configurable via `database.path` setting
- Override per-command with `-f` or `--file` parameter
- Schema: entries table with id, timestamp, title, content, audio_path, image_paths, journal, created_at, updated_at
- Automatic migrations on first run
- Journal field defaults to "Personal" for backward compatibility

## Configuration
- Location: `~/.config/captains-log/config.json` (Linux/macOS) or `%APPDATA%\captains-log\config.json` (Windows)
- JSON format with automatic creation of defaults
- Available settings:
  - `database.path` - Custom database location
  - `editor.command` - Custom editor for entry editing
  - `display.colors_enabled` - Enable/disable colored output
  - `display.date_format` - Custom date format string
  - `display.stardate_mode` - Enable/disable stardate display format
  - `display.entries_per_page` - Pagination limit

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
- [x] New entry creation using external editor

### Configuration System
- [x] Global configuration file support
- [x] JSON-based configuration with automatic defaults
- [x] Configurable database location
- [x] Configurable editor command
- [x] Display settings (colors, date format, pagination)
- [x] Configuration management commands (show, set, path)

### Journal Categories
- [x] Support for organizing entries by journal type (Personal, Work, etc.)
- [x] Journal field added to database schema with "Personal" default
- [x] --journal parameter for quick entry creation
- [x] --journal filter for list command
- [x] Journal type displayed in entry summaries and detailed views
- [x] Backward compatibility with existing entries

### Export System
- [x] JSON export functionality with structured output format
- [x] Markdown export functionality with date-based grouping
- [x] ORG-mode export functionality with org-journal compatible format
- [x] Export filtering support (date, since, until, journal)
- [x] Export metadata (version, export timestamp)
- [x] CLI integration with comprehensive options
- [x] Error handling for unsupported formats
- [x] Optimized timestamp ordering (oldest to newest) in exports
- [x] Efficient database queries with configurable sort order
- [x] Fixed ORG export chronological date grouping (uses NaiveDate keys instead of string sorting)

### Database Override
- [x] Global CLI parameter `-f`/`--file` to override database location
- [x] Works with all commands (list, search, edit, export, etc.)
- [x] Maintains backward compatibility with config and default database
- [x] Automatic directory creation for custom database paths

### External Editor Entry Creation
- [x] New `new` command for creating entries using external editor
- [x] Template-based entry creation with title and content parsing
- [x] Support for journal category specification via --journal parameter
- [x] Empty entry cancellation (no content provided)
- [x] Consistent external editor integration using existing configuration

### Stardate Mode Integration
- [x] Configurable stardate display mode via `display.stardate_mode` setting
- [x] Consistent stardate formatting across list, search, show, and calendar commands
- [x] Stardate calculation based on Star Trek premiere date (September 8, 1966)
- [x] Visual formatting with grayed-out fractional components for readability
- [x] Seamless switching between standard timestamps and stardate format

## Future Enhancement Ideas
- [ ] Tagging system for entries
- [ ] Additional export formats (CSV, XML)
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

### Configuration System Testing
1. Configuration file creation with sensible defaults
2. Configuration viewing with `config show` command
3. Setting individual configuration values with `config set`
4. Database path configuration working correctly
5. Editor command configuration functional
6. Display settings (colors, date format, pagination) operational
7. Error handling for invalid configuration keys
8. Configuration persistence across application restarts

### Journal Categories Testing
1. Journal field added to database schema with automatic migration
2. Default journal value "Personal" applied to new and existing entries
3. Quick entry creation with --journal parameter working
4. List command filtering by journal category functional
5. Journal type displayed in both summary and detailed entry views
6. Combined filtering (date + journal) operational
7. Backward compatibility maintained for existing entries

### Export System Testing
1. JSON export functionality working correctly with proper format
2. Markdown export functionality working with date-based grouping and proper formatting
3. ORG-mode export functionality working with org-journal compatible format
4. Export filtering by journal category operational
5. Export filtering by date ranges (since, until, date) functional
6. Combined filters (journal + date) working properly
7. Export metadata (version, timestamp) included in output
8. Error handling for unsupported formats working correctly
9. CLI help documentation comprehensive and accurate
10. File creation and directory handling working properly
11. Timestamp ordering optimized (oldest to newest) in all export formats
12. Database query performance improved with configurable sort order
13. ORG export chronological date grouping fixed (proper date-based sorting)

### Database Override Testing
1. `-f` parameter works with quick entry creation
2. `--file` parameter works with all commands (list, search, export, etc.)
3. Custom database files are created automatically with proper migrations
4. Directory structure is created for custom database paths
5. Multiple separate databases can be maintained simultaneously
6. Default database remains unaffected when using custom database override
7. Backward compatibility maintained with existing configuration system

### External Editor Entry Creation Testing
1. New `new` command functionality working correctly
2. External editor opens with template content (title placeholder)
3. Title and content parsing from editor output functional
4. Journal category specification via --journal parameter working
5. Empty entry cancellation working (exits gracefully when no content)
6. Entry creation with various content formats (title+content, content-only) operational
7. Integration with existing editor configuration working correctly
8. Database override parameter compatibility confirmed

### Stardate Mode Integration Testing
1. Stardate mode configuration setting and management working correctly
2. List command displays stardates when stardate_mode is enabled
3. Search command displays stardates in search results when enabled
4. Calendar command displays stardates in entry listings when enabled
5. Show command maintains existing stardate functionality
6. Consistent visual formatting (grayed-out fractional components) across all commands
7. Seamless switching between standard timestamps and stardate display
8. Configuration persistence across application restarts
9. Backward compatibility maintained with existing timestamp functionality
- When you test implementation, always use -f parameter to do that on a test db