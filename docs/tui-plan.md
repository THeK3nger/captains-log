# `cl tui` — Plan

A full-screen terminal interface for browsing and editing the journal: listing,
searching, filtering, creating, editing, deleting and moving entries, plus
calendar, stats, and tag/journal management. It is a front end over the same
`Journal` API the CLI and `cl serve` use. No new storage or schema.

## Goals and non-goals

**In scope**: everything you do day to day: list, show, search, filter
(journal / tag / date), new, edit, delete, move, tags, calendar, stats,
journal management, audio playback.

**Stays CLI-only**: `import`, `export`, `models`, `config set`, `record`. Recording
reads Enter from stdin to stop, which fights with a TUI that owns the keyboard.
It could be done later by suspending the TUI, but it is not worth it for v1.

## Groundwork already done

These refactors are merged into the CLI code and change no CLI output (checked
byte-for-byte against the previous binary, colored, in both date modes):

| Module | What the TUI gets |
|---|---|
| `src/cli/editor.rs` | `compose_new_entry(journal, config, journal, tags) -> Option<id>` and `edit_entry_in_editor(journal, config, id) -> bool`: the `$EDITOR` + YAML frontmatter round-trip, without printing. `parse_entry_body` is public and tested. |
| `src/cli/display.rs` | Uncolored display helpers: `entry_date`, `format_date` (safe with bad strftime patterns), `stardate_parts` (head / dimmed tail), `format_tags`, `content_preview`, `match_ranges`, `split_matches` (text segments flagged as search hits), `PREVIEW_CHARS`, `SUMMARY_DATE_FORMAT`. |
| `src/cli/commands/stats.rs` | `pub struct Stats` with public fields, `pub fn compute`, and `load_stats(journal, filters, tz) -> Option<Stats>`. |
| `src/cli/commands/edit.rs` | `audio_file_to_remove` is public (safe resolution of an entry's audio file for deletion). |

Found along the way: an untouched `cl new` editor template (`# `) was saved as an
entry with content `#` instead of being cancelled. Fixed in `parse_entry_body`.

Data access needs no changes. The TUI calls `Journal` directly:
`list_entries_filtered_limited`, `count_entries`, `search_entries`, `get_entry`,
`delete_entry`, `move_entry`, `add_tags` / `remove_tags` / `list_tags` /
`rename_tag`, `list_entries_for_month_filtered`, and the journal functions in
`journal/journals.rs`. Filters come from `dateparser::parse_entry_filters`, so
relative dates ("last week") work in the TUI filter box for free.

## Dependencies

- `ratatui` (with its default `crossterm` backend)
- `tui-textarea`: optional, only if quick in-TUI editing is added (phase 4)

Both are small next to axum/tokio. Consider a `tui` cargo feature (on by
default) if binary size ever matters.

## Architecture

```
src/tui/
├── mod.rs        # run(): terminal setup/restore, panic hook, event loop
├── app.rs        # App state + update(Action) — pure, unit-tested
├── action.rs     # Action enum; key → Action mapping (keymap)
├── markdown.rs   # pulldown-cmark → ratatui Text (styled lines)
└── ui/
    ├── list.rs     # entry list + status line
    ├── preview.rs  # entry detail pane
    ├── sidebar.rs  # journals (with colors) and tags
    ├── popup.rs    # confirm / input / picker dialogs
    ├── calendar.rs
    ├── stats.rs
    └── help.rs
```

- **State/update/draw split**: `App` holds the view (filters, selection,
  loaded page of entries, focused pane, open popup, status message).
  Key events become `Action`s, `App::update(action)` changes state and may
  return an `Effect` (`OpenEditor(id)`, `NewEntry`, `PlayAudio(id)`,
  `Quit`). `draw(&App, frame)` only renders. `update` is tested without a terminal.
- **Editor and audio effects**: leave the alternate screen and disable raw
  mode, call `editor::edit_entry_in_editor` / `compose_new_entry` (or the
  playback tool), then restore the screen, reload, and show the result in the
  status line. Errors go to the status line, never to stderr.
- **Terminal safety**: a guard restores the terminal on drop, and a panic hook
  restores it before printing the panic.
- **Loading**: page through `list_entries_filtered_limited` and
  `count_entries`. Never `list_entries()`, which loads the whole DB. Reload
  after every write.
- **Markdown**: a second renderer in `tui/markdown.rs`, walking the same
  pulldown-cmark events as `cli/formatting.rs` but producing `ratatui::text::Line`s
  (headings, bold/italic/strike, lists, quotes, code, links as underlined text).
  Wrapping comes from ratatui's `Paragraph::wrap`, so `wrap_text` isn't needed.
  `ansi-to-tui` on the existing output is a fallback, but it drags in OSC 8
  link sequences and doesn't wrap well.
- **Config**: honors `display.stardate_mode`, `display.date_format`,
  `display.colors_enabled` (monochrome theme), `editor.command`, timezone.
  `-d/--database` works like for any command.

## Layout

```
┌ Journals ──────┐┌ Entries (124 · Work · #pricing · since last week) ─────┐
│ All        124 ││ [131] 2026-11-06 21:40  Call with Ana  #pricing #urgent│
│ Personal    80 ││ [128] 2026-11-04 09:12  Sprint notes ...               │
│ Work        44 ││ ...                                                    │
├ Tags ──────────┤├────────────────────────────────────────────────────────┤
│ pricing     12 ││ # Call with Ana                                        │
│ urgent       3 ││ Talked about **pricing** for Q1 ...                    │
└────────────────┘└────────────────────────────────────────────────────────┘
 / search  f filter  n new  e edit  d delete  m move  t tags  c calendar  ? help
```

The list and preview are stacked on narrow terminals and side by side on wide
ones (≥ 120 columns). The sidebar can be toggled.

## Keymap (initial)

| Key | Action |
|---|---|
| `j`/`k`, ↑/↓, PgUp/PgDn, `g`/`G` | Move in the list |
| `Tab` / `Shift-Tab` | Cycle focus: sidebar ↔ list ↔ preview |
| `Enter` | Full-screen entry view (`Esc` back) |
| `/` | Search (live, highlighted with `split_matches`) |
| `f` | Date filter (since / until, relative dates allowed) |
| `Esc` | Clear search / filters / close popup |
| `n` | New entry in `$EDITOR` (in the selected journal) |
| `e` | Edit selected entry in `$EDITOR` |
| `d` | Delete (confirm; second confirm for audio file) |
| `m` | Move to journal (picker) |
| `t` | Add/remove tags on the entry |
| `p` | Play audio (if 🎤) |
| `c` | Calendar view |
| `s` | Stats view (for current filters) |
| `J` | Journals management |
| `?` | Help overlay |
| `q` | Quit |

## Phases

| Phase | Content | Est. size |
|---|---|---|
| 1. Skeleton | `cl tui` subcommand, terminal guard + panic hook, event loop, `App`/`Action`, entry list with paging, preview pane, status line, quit | ~600 LOC |
| 2. Core | Sidebar (journals with colors, tags), search, date filter, markdown renderer, full-screen view, help overlay | ~700 LOC |
| 3. Writes | New/edit through `$EDITOR`, delete with confirmation (+ audio), move picker, tag add/remove | ~400 LOC |
| 4. Extras | Calendar (select a day → filter list), stats screen, journal management (add/rename/set color/delete), audio playback, optional in-TUI quick entry with `tui-textarea` | ~700 LOC |

Phases 1–3 make a usable v1 (~1,700 LOC, roughly 3–5 focused days).
Phase 4 adds about 2 more days.

## Testing

- Unit tests on `App::update` (navigation, filter changes, selection kept after
  reload, popups) using an in-memory database via `Journal::new`.
- Render snapshots with ratatui's `TestBackend` for the main screens.
- `tui/markdown.rs` unit tests (one per construct).
- Manual testing always with `-d` on a test database, and with `HOME` pointed
  at a scratch folder if config settings need changing (config is global, not
  per database).

## Open questions

- In-TUI editing (phase 4) or `$EDITOR` only?
- Should `cl` with no subcommand open the TUI when stdout is a terminal?
- Mouse support (click to select, scroll)? Cheap with crossterm, but it stops the
  terminal's own text selection from working.
