//! The external-editor round-trip for creating and editing entries.
//!
//! Nothing here prints: callers (the CLI commands, and later the TUI) decide how
//! to report the outcome. The functions block while the editor runs, so a TUI
//! must leave its alternate screen before calling them.

use crate::cli::dateparser::parse_tags;
use crate::cli::frontmatter::{format_entry_with_frontmatter, parse_frontmatter};
use crate::config::Config;
use crate::journal::{Journal, NewEntry};
use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

/// Write `contents` to a new temp file named `captains-log-<kind>-<time>-<pid>.md`.
///
/// The name is unique and an existing file is never overwritten, so a file
/// kept after a failed save survives later `cl new` / `cl edit` runs.
fn create_temp_file(kind: &str, contents: &str) -> Result<PathBuf> {
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let pid = std::process::id();
    for attempt in 0..100 {
        let suffix = if attempt == 0 {
            String::new()
        } else {
            format!("-{attempt}")
        };
        let path = env::temp_dir().join(format!("captains-log-{kind}-{stamp}-{pid}{suffix}.md"));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                file.write_all(contents.as_bytes())
                    .with_context(|| format!("Failed to write {}", path.display()))?;
                return Ok(path);
            }
            Err(e) if e.kind() == ErrorKind::AlreadyExists => continue,
            Err(e) => {
                return Err(e).with_context(|| format!("Failed to create {}", path.display()));
            }
        }
    }
    anyhow::bail!("Could not create a unique temp file for the editor")
}

/// Run the configured editor (which may include arguments, e.g. "code --wait") on `file`.
fn run_editor(config: &Config, file: &Path) -> Result<ExitStatus> {
    let editor = config.get_editor_command();
    let mut parts = shlex::split(&editor)
        .filter(|parts| !parts.is_empty())
        .with_context(|| format!("Invalid editor command: {editor:?}"))?
        .into_iter();
    let program = parts.next().expect("checked non-empty");

    Command::new(&program)
        .args(parts)
        .arg(file)
        .status()
        .with_context(|| format!("Failed to launch editor {editor:?}"))
}

/// Split an editor buffer into an optional `# Title` first line and the content.
pub fn parse_entry_body(body: &str) -> (Option<String>, String) {
    let trimmed_body = body.trim();
    if trimmed_body.is_empty() {
        return (None, String::new());
    }

    let mut lines = body.lines();
    let Some(first_line) = lines.next() else {
        return (None, String::new());
    };
    let first_line = first_line.trim();

    // A bare "#" is the untouched title line of the new-entry template ("# "
    // after trimming), not content. "#tag" is not a title.
    let title = match first_line.strip_prefix('#') {
        Some("") => Some(""),
        Some(rest) => rest.strip_prefix(' '),
        None => None,
    };
    if let Some(title) = title {
        let content = lines.collect::<Vec<_>>().join("\n").trim().to_string();
        let title = title.trim();
        let title = (!title.is_empty()).then(|| title.to_string());
        (title, content)
    } else {
        (None, trimmed_body.to_string())
    }
}

/// Open the editor on an empty template and create an entry from what was written.
///
/// Returns the new entry's ID, or `None` if the user left the buffer empty.
pub fn compose_new_entry(
    journal: &Journal,
    config: &Config,
    journal_category: Option<&str>,
    tags: &[String],
) -> Result<Option<i64>> {
    let temp_file = create_temp_file("new", "# \n\n")?;

    let status = run_editor(config, &temp_file)?;
    if !status.success() {
        anyhow::bail!("Editor exited with error");
    }

    let edited_content = fs::read_to_string(&temp_file)?;
    let (title, content) = parse_entry_body(&edited_content);

    if content.is_empty() && title.is_none() {
        let _ = fs::remove_file(&temp_file);
        return Ok(None);
    }

    // Only remove the temp file once the entry is saved, so a failed save
    // doesn't lose what the user wrote.
    let id = journal
        .create_entry(
            NewEntry::new(&content)
                .title(title.as_deref())
                .journal(journal_category)
                .tags(tags),
        )
        .with_context(|| {
            format!(
                "Failed to save the entry (your text is kept in {})",
                temp_file.display()
            )
        })?;
    let _ = fs::remove_file(&temp_file);
    Ok(Some(id))
}

/// Open entry `id` in the editor, with its journal, timestamp and extra tags as
/// YAML frontmatter, and save what comes back.
///
/// Returns whether the entry was updated. If the frontmatter can't be parsed,
/// the error says so and the temp file is left in place with the user's edits.
pub fn edit_entry_in_editor(journal: &Journal, config: &Config, id: i64) -> Result<bool> {
    let entry = journal.get_entry(id)?.context("Entry not found")?;

    let body_content = if let Some(title) = &entry.title {
        format!("# {}\n\n{}", title, entry.content)
    } else {
        entry.content.clone()
    };

    // Tags written as #tag in the text are not repeated in the frontmatter.
    let content_with_frontmatter = format_entry_with_frontmatter(
        &entry.journal,
        entry.timestamp,
        &entry.extra_tags(),
        &body_content,
    )?;
    let temp_file = create_temp_file(&format!("edit-{id}"), &content_with_frontmatter)?;

    let status = run_editor(config, &temp_file)?;
    if !status.success() {
        anyhow::bail!("Editor exited with error");
    }

    let edited_content = fs::read_to_string(&temp_file)?;
    let (metadata, body) = parse_frontmatter(&edited_content).with_context(|| {
        format!(
            "Failed to parse entry. Make sure the YAML frontmatter is properly formatted with '---' delimiters (your edits are kept in {})",
            temp_file.display()
        )
    })?;
    let (title, content) = parse_entry_body(&body);
    let extra_tags = parse_tags(&metadata.tags).with_context(|| {
        format!(
            "Invalid tags in the frontmatter (your edits are kept in {})",
            temp_file.display()
        )
    })?;

    let updated = journal
        .update_entry_with_metadata(
            id,
            title.as_deref(),
            &content,
            &metadata.journal,
            metadata.timestamp,
            &extra_tags,
        )
        .with_context(|| {
            format!(
                "Failed to save the entry (your edits are kept in {})",
                temp_file.display()
            )
        })?;

    let _ = fs::remove_file(&temp_file);
    Ok(updated)
}

#[cfg(test)]
mod tests {
    use super::{create_temp_file, parse_entry_body};

    #[test]
    fn temp_files_are_unique_and_never_overwritten() {
        let a = create_temp_file("test", "first").unwrap();
        let b = create_temp_file("test", "second").unwrap();
        assert_ne!(a, b);
        assert_eq!(std::fs::read_to_string(&a).unwrap(), "first");
        assert_eq!(std::fs::read_to_string(&b).unwrap(), "second");
        let _ = std::fs::remove_file(a);
        let _ = std::fs::remove_file(b);
    }

    #[test]
    fn title_line_is_split_from_content() {
        assert_eq!(
            parse_entry_body("# Hello\n\nworld\n"),
            (Some("Hello".into()), "world".into())
        );
    }

    #[test]
    fn hashtag_first_line_is_content() {
        assert_eq!(
            parse_entry_body("#work notes\nmore"),
            (None, "#work notes\nmore".into())
        );
    }

    #[test]
    fn template_with_only_a_body_has_no_title() {
        assert_eq!(parse_entry_body("# \n\nbody"), (None, "body".into()));
    }

    #[test]
    fn empty_title_and_no_title() {
        assert_eq!(parse_entry_body("# \n\n"), (None, String::new()));
        assert_eq!(
            parse_entry_body("  just text \n"),
            (None, "just text".into())
        );
        assert_eq!(parse_entry_body(""), (None, String::new()));
    }
}
