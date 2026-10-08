use super::list::print_entry;
use crate::cli::frontmatter::{format_entry_with_frontmatter, parse_frontmatter};
use crate::config::Config;
use crate::journal::{Journal, NewEntry};
use anyhow::{Context, Result};
use colored::*;
use std::env;
use std::fs;
use std::path::Path;
use std::process::{Command, ExitStatus};

/// `cl new`: create an entry from inline content, or open the editor if there is none.
pub fn create_entry(
    journal: &Journal,
    config: &Config,
    content: Vec<String>,
    journal_category: Option<&str>,
) -> Result<()> {
    if content.is_empty() {
        return new_entry(journal, journal_category, config);
    }

    let entry_content = content.join(" ");
    let id = journal.create_entry(NewEntry::new(&entry_content).journal(journal_category))?;
    println!("{}", format!("Entry {} added successfully", id).green());
    Ok(())
}

pub fn delete_entry(journal: &Journal, config: &Config, id: i64) -> Result<()> {
    let Some(entry) = journal.get_entry(id)? else {
        anyhow::bail!("Entry {} not found", id);
    };

    // Show the entry to be deleted
    println!("{}", "Entry to be deleted:".yellow().bold());
    println!();
    print_entry(&entry, config.display.stardate_mode, config.get_timezone());
    println!();

    // Ask for confirmation
    print!(
        "{}",
        "Are you sure you want to delete this entry? (y/N): "
            .red()
            .bold()
    );
    std::io::Write::flush(&mut std::io::stdout())?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let input = input.trim().to_lowercase();

    if input == "y" || input == "yes" {
        if journal.delete_entry(id)? {
            println!("{}", format!("Entry {} deleted", id).green());
        } else {
            println!("{}", format!("Failed to delete entry {}", id).red());
        }
    } else {
        println!("{}", "Deletion cancelled".yellow());
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

fn parse_entry_body(body: &str) -> (Option<String>, String) {
    let trimmed_body = body.trim();
    if trimmed_body.is_empty() {
        return (None, String::new());
    }

    let mut lines = body.lines();
    let Some(first_line) = lines.next() else {
        return (None, String::new());
    };
    let first_line = first_line.trim();

    if let Some(title) = first_line.strip_prefix("# ") {
        let content = lines.collect::<Vec<_>>().join("\n").trim().to_string();
        let title = title.trim();
        let title = (!title.is_empty()).then(|| title.to_string());
        (title, content)
    } else {
        (None, trimmed_body.to_string())
    }
}

fn new_entry(journal: &Journal, journal_category: Option<&str>, config: &Config) -> Result<()> {
    // Create a temporary file for the new entry
    let temp_dir = env::temp_dir();
    let temp_file = temp_dir.join("captains-log-new.md");

    // Write template content to temp file
    let template_content = "# \n\n";
    fs::write(&temp_file, template_content)?;

    let status = run_editor(config, &temp_file)?;

    if !status.success() {
        return Err(anyhow::anyhow!("Editor exited with error"));
    }

    // Read the edited content
    let edited_content = fs::read_to_string(&temp_file)?;
    let (title, content) = parse_entry_body(&edited_content);

    // Check if the content is empty
    if content.is_empty() && title.is_none() {
        println!(
            "{}",
            "Entry creation cancelled - no content provided".yellow()
        );
        // Clean up temp file
        let _ = fs::remove_file(&temp_file);
        return Ok(());
    }

    // Create the entry
    let id = journal.create_entry(
        NewEntry::new(&content)
            .title(title.as_deref())
            .journal(journal_category),
    )?;
    println!("{}", format!("Entry {} created successfully", id).green());

    // Clean up temp file
    let _ = fs::remove_file(&temp_file);

    Ok(())
}

pub fn edit_entry(journal: &Journal, id: i64, config: &Config) -> Result<()> {
    // Get the existing entry
    let entry = journal.get_entry(id)?.context("Entry not found")?;

    // Create a temporary file with the current content
    let temp_dir = env::temp_dir();
    let temp_file = temp_dir.join(format!("captains-log-edit-{}.md", id));

    // Format content with title if present
    let body_content = if let Some(title) = &entry.title {
        format!("# {}\n\n{}", title, entry.content)
    } else {
        entry.content.clone()
    };

    // Write current content with YAML frontmatter to temp file
    let content_with_frontmatter =
        format_entry_with_frontmatter(&entry.journal, entry.timestamp, &body_content)?;
    fs::write(&temp_file, content_with_frontmatter)?;

    let status = run_editor(config, &temp_file)?;

    if !status.success() {
        return Err(anyhow::anyhow!("Editor exited with error"));
    }

    // Read the edited content
    let edited_content = fs::read_to_string(&temp_file)?;

    // Parse frontmatter and content
    let (metadata, body) = parse_frontmatter(&edited_content).context(
        "Failed to parse entry. Make sure the YAML frontmatter is properly formatted with '---' delimiters",
    )?;
    let (title, content) = parse_entry_body(&body);

    // Update the entry with metadata
    if journal.update_entry_with_metadata(
        id,
        title.as_deref(),
        &content,
        &metadata.journal,
        metadata.timestamp,
    )? {
        println!("{}", format!("Entry {} updated successfully", id).green());
    } else {
        println!("{}", format!("Failed to update entry {}", id).red());
    }

    // Clean up temp file
    let _ = fs::remove_file(&temp_file);

    Ok(())
}
