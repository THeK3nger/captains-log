use crate::cli::dateparser::parse_tags;
use crate::journal::Journal;
use anyhow::Result;
use clap::Subcommand;
use colored::*;

#[derive(Subcommand)]
pub enum TagsAction {
    /// List tags with the number of entries using them (default)
    List,

    /// Add tags to an entry
    Add {
        /// Entry ID
        id: i64,
        /// Tags to add
        #[arg(required = true)]
        tags: Vec<String>,
    },

    /// Remove tags from an entry (tags written as #tag in the text must be removed by editing it)
    Remove {
        /// Entry ID
        id: i64,
        /// Tags to remove
        #[arg(required = true)]
        tags: Vec<String>,
    },

    /// Rescan all entry texts for #tags (for entries saved before tagging existed)
    Reindex,

    /// Rename a tag in all entries, including #tags in their text (merges if the new name exists)
    Rename {
        /// Current tag name
        old: String,
        /// New tag name
        new: String,
    },
}

pub fn handle_tags_command(journal: &Journal, action: Option<TagsAction>) -> Result<()> {
    match action.unwrap_or(TagsAction::List) {
        TagsAction::List => list_tags(journal),
        TagsAction::Add { id, tags } => {
            let tags = parse_tags(&tags)?;
            if !journal.add_tags(id, &tags)? {
                anyhow::bail!("Entry {} not found", id);
            }
            println!("{}", format!("Entry {} tagged", id).green());
            Ok(())
        }
        TagsAction::Remove { id, tags } => {
            if !journal.remove_tags(id, &tags)? {
                anyhow::bail!("Entry {} not found", id);
            }
            println!("{}", format!("Tags removed from entry {}", id).green());
            Ok(())
        }
        TagsAction::Reindex => {
            let changed = journal.reindex_tags()?;
            println!(
                "{}",
                format!("Reindexed tags: {changed} entries updated").green()
            );
            Ok(())
        }
        TagsAction::Rename { old, new } => {
            let changed = journal.rename_tag(&old, &new)?;
            if changed == 0 {
                println!("{}", format!("No entries have the tag '{old}'").yellow());
            } else {
                println!(
                    "{}",
                    format!("Renamed '{old}' to '{new}' in {changed} entries").green()
                );
            }
            Ok(())
        }
    }
}

fn list_tags(journal: &Journal) -> Result<()> {
    let tags = journal.list_tags()?;
    if tags.is_empty() {
        println!("{}", "No tags yet".yellow());
        return Ok(());
    }

    let width = tags
        .iter()
        .map(|tag| tag.name.chars().count() + 1)
        .max()
        .unwrap_or(0);
    for tag in tags {
        // Pad before coloring: escape codes would count towards the width
        let name = format!("{:<width$}", format!("#{}", tag.name));
        println!("{}  {}", name.blue().bold(), tag.count);
    }
    Ok(())
}
