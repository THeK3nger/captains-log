# Captain's Log - Development Notes

## Project Overview
A terminal-based journaling application written in Rust with SQLite storage.

## Current Status
✅ **Phase 1 Complete** - Core functionality implemented and tested
✅ **Phase 2 Complete** - Advanced features implemented and tested
✅ **Configuration System** - Global configuration file support added
✅ **Journal Categories** - Support for organizing entries by journal type
✅ **Export System** - JSON, Markdown, and ORG export functionality with filtering support and optimized timestamp ordering
✅ **Import System** - ORG-journal and DayOne JSON import functionality with date and journal filtering
✅ **Database Override** - CLI parameter to override database location for any command
✅ **Quick Entry Creation** - Create entries with inline content or using external editor
✅ **Stardate Mode Integration** - Consistent stardate formatting across all entry display commands

## Build & Test Commands

### Build the project
```bash
cargo build
```

### Run the application
```bash
# Quick entry creation (inline content)
./target/debug/cl new "Your journal entry content"
./target/debug/cl new "Your journal entry content" --journal Work
./target/debug/cl new "Personal note" --journal Personal

# Create new entry (opens external editor)
./target/debug/cl new
./target/debug/cl new --journal Work

# List all entries
./target/debug/cl list

# List entries by journal category
./target/debug/cl list --journal Work
./target/debug/cl list --journal Personal

# List entries with date filtering (supports both absolute and relative dates)
./target/debug/cl list --since 2025-01-01
./target/debug/cl list --until 2025-12-31
./target/debug/cl list --date 2025-09-09

# List entries with relative date filtering
./target/debug/cl list --since "last week"
./target/debug/cl list --since "yesterday"
./target/debug/cl list --date "today"
./target/debug/cl list --since "7 days ago"
./target/debug/cl list --since "2 weeks ago"
./target/debug/cl list --until "tomorrow"

# Combine filters (date and journal)
./target/debug/cl list --journal Work --since 2025-01-01
./target/debug/cl list --journal Personal --date 2025-09-09
./target/debug/cl list --journal Work --since "last month"

# Show specific entry
./target/debug/cl show <id>

# Search entries
./target/debug/cl search "<query>"

# Edit entry (opens external editor)
./target/debug/cl edit <id>

# Delete entry
./target/debug/cl delete <id>

# Move entry to different journal
./target/debug/cl move <id> <target_journal>

# Calendar view
./target/debug/cl calendar
./target/debug/cl calendar --year 2024 --month 12
./target/debug/cl calendar --journal Work

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

# Import entries from ORG or DayOne formats
./target/debug/cl import path/to/file.org --format org
./target/debug/cl import path/to/journal.json --format dayone
./target/debug/cl import file.org --format org --journal Work
./target/debug/cl import file.org --format org --date 2025-09-09

# Audio recording and playback
./target/debug/cl record
./target/debug/cl record --journal Work
./target/debug/cl record --no-transcribe
./target/debug/cl record --max-duration 300
./target/debug/cl play <id>

# Override database location (global parameter for any command)
./target/debug/cl -d "path/to/custom.db" new "Entry with custom database"
./target/debug/cl --database "/tmp/temp.db" list
./target/debug/cl -d "backup.db" export --output backup.json --format json

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
├── audio/
│   ├── mod.rs           # Audio module root and public API
│   ├── storage.rs       # Audio file management and path handling
│   ├── platform.rs      # Platform detection and tool selection
│   ├── recording.rs     # Audio recording implementation
│   ├── playback.rs      # Audio playback implementation
│   └── transcription.rs # Whisper integration for speech-to-text
├── cli/
│   ├── mod.rs           # Command handling and help text
│   ├── dateparser.rs    # Date parsing utilities
│   ├── formatting.rs    # Markdown rendering utilities
│   ├── frontmatter.rs   # YAML frontmatter parsing and formatting
│   └── stardate.rs      # Stardate conversion system
├── config/
│   └── mod.rs           # Configuration management and file handling
├── database/
│   └── mod.rs           # SQLite connection and migrations
├── export/
│   └── mod.rs           # Export functionality (JSON, Markdown, and ORG formats)
├── import/
│   └── mod.rs           # Import functionality (ORG-journal and DayOne formats)
└── journal/
    └── mod.rs           # Entry model and CRUD operations
```

## Database
- Default Location: `~/.local/share/captains-log/journal.db`
- Configurable via `database.path` setting
- Override per-command with `-d` or `--database` parameter
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
  - `audio.whisper_command` - Path to whisper.cpp binary (auto-detected by default)
  - `audio.whisper_model` - Whisper model to use (default: "base.en")
  - `audio.recording_tool` - Custom recording tool command (auto-detected by default)
  - `audio.playback_tool` - Custom playback tool command (auto-detected by default)
  - `audio.max_recording_seconds` - Maximum recording duration (default: 600)
  - `audio.sample_rate` - Audio sample rate in Hz (default: 16000)

## Date Filtering
All date-based filters (--date, --since, --until) support both absolute and relative date formats:

### Absolute Dates
- Standard format: `YYYY-MM-DD` (e.g., `2025-01-15`)

### Relative Dates
- **Simple**: `today`, `yesterday`, `tomorrow`
- **Week-based**: `this week`, `last week`, `next week`
- **Month-based**: `last month`, `next month`
- **Year-based**: `last year`, `next year`
- **Days offset**: `X days ago`, `X days from now` (e.g., `7 days ago`)
- **Weeks offset**: `X weeks ago`, `X weeks from now` (e.g., `2 weeks ago`)

### Examples
```bash
# Show entries from yesterday
./target/debug/cl list --since yesterday

# Show entries from the last week
./target/debug/cl list --since "last week"

# Show entries from 7 days ago until today
./target/debug/cl list --since "7 days ago"

# Export entries from last month
./target/debug/cl export --output last_month.json --since "last month" --format json
```

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
- [x] Relative date parsing (e.g., "yesterday", "last week", "7 days ago", "this week")
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
- [x] Global CLI parameter `-d`/`--database` to override database location
- [x] Works with all commands (list, search, edit, export, etc.)
- [x] Maintains backward compatibility with config and default database
- [x] Automatic directory creation for custom database paths

### Quick Entry Creation
- [x] New `new` command for creating entries
- [x] Inline content creation via command arguments (e.g., `cl new "content"`)
- [x] External editor integration for detailed entry creation
- [x] Template-based entry creation with title and content parsing
- [x] Support for journal category specification via --journal parameter
- [x] Empty entry cancellation (no content provided)
- [x] Consistent external editor integration using existing configuration

### Import System
- [x] ORG-journal format import support
- [x] DayOne JSON format import support
- [x] Date filtering for imports (--date parameter)
- [x] Journal category assignment for imported entries
- [x] Detailed import statistics (total, imported, skipped, errors)
- [x] Error handling and reporting for malformed entries

### Stardate Mode Integration
- [x] Configurable stardate display mode via `display.stardate_mode` setting
- [x] Consistent stardate formatting across list, search, show, and calendar commands
- [x] Stardate calculation based on Star Trek premiere date (September 8, 1966)
- [x] Visual formatting with grayed-out fractional components for readability
- [x] Seamless switching between standard timestamps and stardate format

### Audio Recording System
- [x] Audio recording with automatic transcription using Whisper
- [x] Platform-specific tool detection (sox/arecord for recording, afplay/ffplay for playback)
- [x] Record command with journal category support and optional transcription skip
- [x] Play command to replay audio from entries
- [x] Audio file storage in dedicated directory alongside database
- [x] Relative path storage in database for portability
- [x] Graceful Ctrl+C handling during recording
- [x] Configurable max recording duration
- [x] Audio indicator (🎤) in entry summaries and detailed views
- [x] Whisper.cpp integration for speech-to-text transcription
- [x] Configurable Whisper model selection
- [x] Error handling with fallback for transcription failures
- [x] 16kHz mono WAV format optimized for speech recognition

## Future Enhancement Ideas
- [ ] Tagging system for entries
- [ ] Additional export formats (CSV, XML)
- [ ] Additional import formats (Joplin, Notion, etc.)
- [ ] Full-text search improvements
- [ ] Image attachment support
- [ ] Entry templates for common journal types
- [ ] Audio compression (WAV to Opus conversion)
- [ ] Background transcription for long recordings
- [ ] Multi-language support for transcription
- [ ] Audio duration display in all views
- [ ] Batch transcription command for existing audio files

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
2. Date filtering (--date, --since, --until) works properly with absolute dates
3. Relative date parsing working correctly (today, yesterday, last week, X days ago, etc.)
4. External editor integration functional (uses $EDITOR or defaults to nvim)
5. Entry editing updates database correctly
6. Rich terminal formatting enhances user experience
7. All new commands properly documented in help system

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
1. `-d` parameter works with quick entry creation
2. `--database` parameter works with all commands (list, search, export, etc.)
3. Custom database files are created automatically with proper migrations
4. Directory structure is created for custom database paths
5. Multiple separate databases can be maintained simultaneously
6. Default database remains unaffected when using custom database override
7. Backward compatibility maintained with existing configuration system

### Quick Entry Creation Testing
1. New `new` command functionality working correctly
2. Inline content creation via command arguments working (e.g., `cl new "content"`)
3. External editor opens with template content (title placeholder)
4. Title and content parsing from editor output functional
5. Journal category specification via --journal parameter working
6. Empty entry cancellation working (exits gracefully when no content)
7. Entry creation with various content formats (title+content, content-only) operational
8. Integration with existing editor configuration working correctly
9. Database override parameter compatibility confirmed

### Import System Testing
1. ORG-journal format import functionality working correctly
2. DayOne JSON format import functionality working correctly
3. Date filtering for imports operational (--date parameter)
4. Journal category assignment for imported entries working
5. Import statistics display (total, imported, skipped, errors) accurate
6. Error handling for malformed entries working correctly
7. Timestamp parsing and preservation functional
8. Experimental feature warnings displayed appropriately

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

### Audio Recording System Testing
1. Platform detection working correctly (macOS/Linux tool selection)
2. Recording tools detected in priority order (arecord → sox → ffmpeg on Linux)
3. Playback tools detected in priority order (afplay on macOS, ffplay on Linux)
4. Audio recording with Ctrl+C graceful shutdown functional
5. Audio files created with correct format (16kHz mono WAV)
6. Audio files stored in dedicated audio/ directory alongside database
7. Relative path storage in database working correctly
8. Transcription integration functional (requires whisper.cpp installation)
9. Record command creates entries with transcribed content
10. Play command successfully plays audio from entries
11. Audio indicator (🎤) displays correctly in list and summary views
12. Show command displays audio file path
13. Error handling for missing tools displays helpful installation instructions
14. Transcription fallback working when Whisper fails
15. --no-transcribe flag working correctly (audio-only entries)
16. --max-duration parameter working correctly
17. Configuration options for audio tools working correctly

- When you test implementation, always use -d parameter to do that on a test db