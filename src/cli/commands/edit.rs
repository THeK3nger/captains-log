use super::list::print_entry;
use crate::audio::{get_audio_directory, get_audio_full_path};
use crate::cli::dateparser::parse_tags;
use crate::cli::display::format_tags;
use crate::cli::editor::{compose_new_entry, edit_entry_in_editor};
use crate::config::Config;
use crate::journal::{Journal, NewEntry};
use anyhow::{Context, Result};
use colored::*;
use std::fs;
use std::path::{Path, PathBuf};

/// `cl new`: create an entry from inline content, or open the editor if there is none.
pub fn create_entry(
    journal: &Journal,
    config: &Config,
    content: Vec<String>,
    journal_category: Option<&str>,
    tags: &[String],
) -> Result<()> {
    let tags = parse_tags(tags)?;
    if content.is_empty() {
        return new_entry(journal, journal_category, &tags, config);
    }

    let entry_content = content.join(" ");
    let id = journal.create_entry(
        NewEntry::new(&entry_content)
            .journal(journal_category)
            .tags(&tags),
    )?;
    println!("{}", format!("Entry {} added successfully", id).green());
    print_entry_tags(journal, id)?;
    Ok(())
}

/// Tell the user which tags an entry ended up with (inline `#tags` included).
fn print_entry_tags(journal: &Journal, id: i64) -> Result<()> {
    if let Some(entry) = journal.get_entry(id)?
        && !entry.tags.is_empty()
    {
        println!("{}: {}", "Tags".cyan(), format_tags(&entry.tags));
    }
    Ok(())
}

/// Print `prompt` and read a y/N answer from stdin. EOF or anything but y/yes is "no".
fn confirm(prompt: ColoredString) -> Result<bool> {
    print!("{prompt}");
    std::io::Write::flush(&mut std::io::stdout())?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let input = input.trim().to_lowercase();
    Ok(input == "y" || input == "yes")
}

/// Resolve an entry's audio file, refusing anything outside the audio directory
/// (the path comes from the database, so don't trust it blindly).
/// `None` if the file doesn't exist.
pub fn audio_file_to_remove(db_path: &Path, relative_path: &str) -> Result<Option<PathBuf>> {
    let full_path = get_audio_full_path(db_path, relative_path)?;
    if !full_path.exists() {
        return Ok(None);
    }

    let audio_dir = get_audio_directory(db_path)?.canonicalize()?;
    let full_path = full_path.canonicalize()?;
    if !full_path.starts_with(&audio_dir) {
        anyhow::bail!(
            "Refusing to delete {}: it is outside the audio directory",
            full_path.display()
        );
    }
    Ok(Some(full_path))
}

/// `cl delete`. Asks for confirmation unless `yes` is set. If the entry has an
/// audio recording, also removes the file (asking first, unless `yes`), unless
/// `keep_audio` is set.
pub fn delete_entry(
    journal: &Journal,
    config: &Config,
    db_path: &Path,
    id: i64,
    yes: bool,
    keep_audio: bool,
) -> Result<()> {
    let Some(entry) = journal.get_entry(id)? else {
        anyhow::bail!("Entry {} not found", id);
    };

    if !yes {
        // Show the entry to be deleted
        println!("{}", "Entry to be deleted:".yellow().bold());
        println!();
        print_entry(&entry, config);
        println!();

        let prompt = "Are you sure you want to delete this entry? (y/N): "
            .red()
            .bold();
        if !confirm(prompt)? {
            println!("{}", "Deletion cancelled".yellow());
            return Ok(());
        }
    }

    // Decide about the audio file before deleting the entry, so a "no" or a
    // problem with the path doesn't leave a half-finished delete behind.
    let mut audio_to_remove = None;
    let mut audio_note = None;
    if let Some(relative_path) = entry.audio_path.as_deref() {
        if keep_audio {
            audio_note = Some(format!("Audio file kept: {relative_path}"));
        } else {
            match audio_file_to_remove(db_path, relative_path)? {
                None => {
                    audio_note = Some(format!(
                        "Audio file not found, nothing to remove: {relative_path}"
                    ));
                }
                Some(path) => {
                    let prompt = format!("Also delete the audio file {relative_path}? (y/N): ");
                    if yes || confirm(prompt.red().bold())? {
                        audio_to_remove = Some(path);
                    } else {
                        audio_note = Some(format!("Audio file kept: {relative_path}"));
                    }
                }
            }
        }
    }

    if !journal.delete_entry(id)? {
        println!("{}", format!("Failed to delete entry {}", id).red());
        return Ok(());
    }
    println!("{}", format!("Entry {} deleted", id).green());

    if let Some(path) = audio_to_remove {
        fs::remove_file(&path).with_context(|| {
            format!(
                "Entry {id} was deleted, but the audio file {} could not be removed",
                path.display()
            )
        })?;
        println!("{}", "Audio file deleted".green());
    } else if let Some(note) = audio_note {
        println!("{}", note.bright_black());
    }

    Ok(())
}

pub fn move_entry(journal: &Journal, id: i64, target_journal: &str) -> Result<()> {
    let Some(entry) = journal.get_entry(id)? else {
        anyhow::bail!("Entry {} not found", id);
    };

    if journal.move_entry(id, target_journal)? {
        println!(
            "{}",
            format!(
                "Entry {} moved from '{}' to '{}'",
                id, entry.journal, target_journal
            )
            .green()
        );
    } else {
        println!("{}", format!("Failed to move entry {}", id).red());
    }

    Ok(())
}

/// `cl new` without content: write the entry in the editor.
fn new_entry(
    journal: &Journal,
    journal_category: Option<&str>,
    tags: &[String],
    config: &Config,
) -> Result<()> {
    match compose_new_entry(journal, config, journal_category, tags)? {
        Some(id) => {
            println!("{}", format!("Entry {} created successfully", id).green());
            print_entry_tags(journal, id)?;
        }
        None => println!(
            "{}",
            "Entry creation cancelled - no content provided".yellow()
        ),
    }
    Ok(())
}

pub fn edit_entry(journal: &Journal, id: i64, config: &Config) -> Result<()> {
    if edit_entry_in_editor(journal, config, id)? {
        println!("{}", format!("Entry {} updated successfully", id).green());
        print_entry_tags(journal, id)?;
    } else {
        println!("{}", format!("Failed to update entry {}", id).red());
    }
    Ok(())
}
