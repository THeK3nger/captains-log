//! Journals: named groups of entries with optional description and color.

use super::Journal;
use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};

/// Color names understood by both terminals and CSS.
pub const COLOR_NAMES: [&str; 8] = [
    "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
];

/// A journal with its metadata and the number of entries in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalInfo {
    pub name: String,
    pub description: Option<String>,
    /// One of [`COLOR_NAMES`] or `#rrggbb`, see [`normalize_color`].
    pub color: Option<String>,
    pub entry_count: usize,
}

/// Trimmed journal name; errors if it is blank.
fn normalize_name(name: &str) -> Result<&str> {
    let name = name.trim();
    if name.is_empty() {
        anyhow::bail!("Journal name cannot be empty");
    }
    Ok(name)
}

/// A color as stored: a lowercase name from [`COLOR_NAMES`], or `#rrggbb`
/// (`#rgb` is expanded). `None` for anything else.
pub fn normalize_color(color: &str) -> Option<String> {
    let color = color.trim().to_lowercase();
    if COLOR_NAMES.contains(&color.as_str()) {
        return Some(color);
    }
    let hex = color.strip_prefix('#')?;
    if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    match hex.len() {
        6 => Some(color),
        3 => Some(hex.chars().fold(String::from("#"), |mut out, c| {
            out.push(c);
            out.push(c);
            out
        })),
        _ => None,
    }
}

fn validated_color(color: &str) -> Result<String> {
    normalize_color(color).with_context(|| {
        format!(
            "Invalid color '{color}': use #rrggbb, #rgb or one of {}",
            COLOR_NAMES.join(", ")
        )
    })
}

/// Blank text means "no value".
fn non_blank(text: &str) -> Option<&str> {
    Some(text.trim()).filter(|text| !text.is_empty())
}

/// Id of the journal called `name` (ignoring case), creating it if needed.
pub(super) fn ensure_journal(conn: &Connection, name: &str) -> Result<i64> {
    let name = normalize_name(name)?;
    conn.execute("INSERT OR IGNORE INTO journals (name) VALUES (?1)", [name])?;
    let id = conn.query_row("SELECT id FROM journals WHERE name = ?1", [name], |row| {
        row.get(0)
    })?;
    Ok(id)
}

fn find_journal(conn: &Connection, name: &str) -> Result<Option<i64>> {
    let id = conn
        .query_row(
            "SELECT id FROM journals WHERE name = ?1",
            [name.trim()],
            |row| row.get(0),
        )
        .optional()?;
    Ok(id)
}

impl Journal {
    /// All journals, including empty ones, sorted by name.
    pub fn list_journals(&self) -> Result<Vec<JournalInfo>> {
        let mut stmt = self.db.connection().prepare(
            "SELECT j.name, j.description, j.color, COUNT(e.id) FROM journals j
             LEFT JOIN entries e ON e.journal_id = j.id
             GROUP BY j.id ORDER BY j.name ASC",
        )?;
        let journals = stmt
            .query_map([], |row| {
                Ok(JournalInfo {
                    name: row.get(0)?,
                    description: row.get(1)?,
                    color: row.get(2)?,
                    entry_count: usize::try_from(row.get::<_, i64>(3)?).unwrap_or_default(),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(journals)
    }

    /// Create an empty journal. Errors if one with the same name (ignoring case) exists.
    pub fn create_journal(
        &self,
        name: &str,
        description: Option<&str>,
        color: Option<&str>,
    ) -> Result<()> {
        let conn = self.db.connection();
        let name = normalize_name(name)?;
        let color = color.and_then(non_blank).map(validated_color).transpose()?;
        if find_journal(conn, name)?.is_some() {
            anyhow::bail!("Journal '{name}' already exists");
        }
        conn.execute(
            "INSERT INTO journals (name, description, color) VALUES (?1, ?2, ?3)",
            params![name, description.and_then(non_blank), color],
        )?;
        Ok(())
    }

    /// Change a journal's metadata. `None` leaves a field as it is; blank text
    /// clears it. Returns false if the journal does not exist.
    pub fn update_journal(
        &self,
        name: &str,
        description: Option<&str>,
        color: Option<&str>,
    ) -> Result<bool> {
        let conn = self.db.connection();
        let Some(id) = find_journal(conn, name)? else {
            return Ok(false);
        };
        let tx = conn.unchecked_transaction()?;
        if let Some(description) = description {
            tx.execute(
                "UPDATE journals SET description = ?1 WHERE id = ?2",
                params![non_blank(description), id],
            )?;
        }
        if let Some(color) = color {
            let color = non_blank(color).map(validated_color).transpose()?;
            tx.execute(
                "UPDATE journals SET color = ?1 WHERE id = ?2",
                params![color, id],
            )?;
        }
        tx.commit()?;
        Ok(true)
    }

    /// Rename a journal, keeping its entries and metadata. Changing only the
    /// case is allowed; renaming onto another existing journal is not.
    /// Returns false if the journal does not exist.
    pub fn rename_journal(&self, old: &str, new: &str) -> Result<bool> {
        let conn = self.db.connection();
        let new = normalize_name(new)?;
        let Some(id) = find_journal(conn, old)? else {
            return Ok(false);
        };
        if find_journal(conn, new)?.is_some_and(|other| other != id) {
            anyhow::bail!(
                "Journal '{new}' already exists; move the entries with `cl move` instead"
            );
        }
        conn.execute(
            "UPDATE journals SET name = ?1 WHERE id = ?2",
            params![new, id],
        )?;
        Ok(true)
    }

    /// Delete an empty journal. Returns false if it does not exist.
    pub fn delete_journal(&self, name: &str) -> Result<bool> {
        let conn = self.db.connection();
        let Some(id) = find_journal(conn, name)? else {
            return Ok(false);
        };
        let entries: i64 = conn.query_row(
            "SELECT COUNT(*) FROM entries WHERE journal_id = ?1",
            [id],
            |row| row.get(0),
        )?;
        if entries > 0 {
            anyhow::bail!(
                "Journal '{}' still has {entries} entries; move or delete them first",
                name.trim()
            );
        }
        conn.execute("DELETE FROM journals WHERE id = ?1", [id])?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;
    use crate::journal::{EntryFilters, NewEntry};

    fn journal() -> Journal {
        Journal::new(Database::new_with_path(":memory:").unwrap())
    }

    fn info<'a>(journals: &'a [JournalInfo], name: &str) -> &'a JournalInfo {
        journals.iter().find(|j| j.name == name).unwrap()
    }

    #[test]
    fn colors_are_normalized() {
        assert_eq!(normalize_color(" Blue "), Some("blue".into()));
        assert_eq!(normalize_color("#FF9900"), Some("#ff9900".into()));
        assert_eq!(normalize_color("#f90"), Some("#ff9900".into()));
        assert_eq!(normalize_color("#ff99"), None);
        assert_eq!(normalize_color("#gg9900"), None);
        assert_eq!(normalize_color("orange"), None);
    }

    #[test]
    fn entries_share_a_journal_regardless_of_case() {
        let j = journal();
        j.create_entry(NewEntry::new("a").journal(Some("Work")))
            .unwrap();
        let id = j
            .create_entry(NewEntry::new("b").journal(Some(" work ")))
            .unwrap();

        assert_eq!(j.get_entry(id).unwrap().unwrap().journal, "Work");
        let journals = j.list_journals().unwrap();
        assert_eq!(journals.len(), 1);
        assert_eq!(journals[0].entry_count, 2);

        let filters = EntryFilters {
            journal: Some("WORK".into()),
            ..EntryFilters::default()
        };
        assert_eq!(j.count_entries(&filters).unwrap(), 2);
    }

    #[test]
    fn blank_journal_names_are_rejected() {
        let j = journal();
        assert!(
            j.create_entry(NewEntry::new("a").journal(Some("  ")))
                .is_err()
        );
        assert!(j.create_journal("", None, None).is_err());
    }

    #[test]
    fn metadata_can_be_set_and_cleared() {
        let j = journal();
        j.create_journal("Work", Some("Day job"), Some("#0af"))
            .unwrap();
        assert!(j.create_journal("work", None, None).is_err());
        assert!(j.create_journal("Ops", None, Some("chartreuse")).is_err());

        let journals = j.list_journals().unwrap();
        let work = info(&journals, "Work");
        assert_eq!(work.description.as_deref(), Some("Day job"));
        assert_eq!(work.color.as_deref(), Some("#00aaff"));
        assert_eq!(work.entry_count, 0);

        assert!(j.update_journal("WORK", Some(""), None).unwrap());
        let journals = j.list_journals().unwrap();
        assert_eq!(info(&journals, "Work").description, None);
        assert_eq!(info(&journals, "Work").color.as_deref(), Some("#00aaff"));

        assert!(j.update_journal("Work", None, Some("bogus")).is_err());
        assert!(!j.update_journal("Missing", Some("x"), None).unwrap());
    }

    #[test]
    fn rename_keeps_entries_and_refuses_collisions() {
        let j = journal();
        let id = j
            .create_entry(NewEntry::new("a").journal(Some("work")))
            .unwrap();
        j.create_journal("Personal", None, None).unwrap();

        assert!(j.rename_journal("work", "Work").unwrap());
        assert_eq!(j.get_entry(id).unwrap().unwrap().journal, "Work");
        assert!(j.rename_journal("Work", "personal").is_err());
        assert!(!j.rename_journal("Missing", "Other").unwrap());
    }

    #[test]
    fn only_empty_journals_can_be_deleted() {
        let j = journal();
        let id = j
            .create_entry(NewEntry::new("a").journal(Some("Work")))
            .unwrap();

        assert!(j.delete_journal("Work").is_err());
        j.move_entry(id, "Personal").unwrap();
        assert!(j.delete_journal("work").unwrap());
        assert!(!j.delete_journal("Work").unwrap());
    }

    #[test]
    fn moving_a_missing_entry_creates_no_journal() {
        let j = journal();
        assert!(!j.move_entry(42, "Nowhere").unwrap());
        assert!(j.list_journals().unwrap().is_empty());
    }
}
