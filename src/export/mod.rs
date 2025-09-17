use crate::journal::{Entry, Journal};
use anyhow::{Context, Result};
use pulldown_cmark::{Event, Options, Tag, TagEnd};

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct ExportData {
    pub version: String,
    pub exported_at: chrono::DateTime<chrono::Utc>,
    pub entries: Vec<Entry>,
}

pub struct Exporter<'a> {
    journal: &'a Journal,
}

impl<'a> Exporter<'a> {
    pub fn new(journal: &'a Journal) -> Self {
        Self { journal }
    }

    pub fn export_to_json(
        &self,
        output_path: Option<String>,
        filters: Option<ExportFilters>,
    ) -> Result<()> {
        let entries = if let Some(filters) = filters {
            self.journal.list_entries_filtered_with_order(
                filters.date.as_deref(),
                filters.since.as_deref(),
                filters.until.as_deref(),
                filters.journal.as_deref(),
                "timestamp",
                "ASC",
            )?
        } else {
            self.journal.list_entries_with_order("timestamp", "ASC")?
        };

        let export_data = ExportData {
            version: env!("CARGO_PKG_VERSION").to_string(),
            exported_at: chrono::Utc::now(),
            entries,
        };

        let json_content = serde_json::to_string_pretty(&export_data)
            .context("Failed to serialize entries to JSON")?;

        if output_path.is_some() {
            let path: &str = output_path.as_deref().unwrap();
            // Create directory if it doesn't exist
            if let Some(parent) = Path::new(path).parent() {
                fs::create_dir_all(parent).context("Failed to create output directory")?;
            }

            fs::write(path, json_content).context("Failed to write JSON file")?;
        } else {
            println!("{}", json_content);
        }

        Ok(())
    }

    pub fn export_to_org(
        &self,
        output_path: Option<String>,
        filters: Option<ExportFilters>,
    ) -> Result<()> {
        let entries = if let Some(filters) = filters {
            self.journal.list_entries_filtered_with_order(
                filters.date.as_deref(),
                filters.since.as_deref(),
                filters.until.as_deref(),
                filters.journal.as_deref(),
                "timestamp",
                "ASC",
            )?
        } else {
            self.journal.list_entries_with_order("timestamp", "ASC")?
        };

        let mut org_content = String::new();
        // Entries are grouped by date as in org-journal format. Example:
        // * Wednesday, 17/09/2025
        // :PROPERTIES:
        // :CREATED:  20250917
        // :END:
        // ** 19:31
        // Entry content...
        // ** 20:45
        // Another entry content...

        // Group entries by date using NaiveDate for proper chronological ordering
        use chrono::NaiveDate;
        use std::collections::BTreeMap;
        let mut grouped_entries: BTreeMap<NaiveDate, Vec<&Entry>> = BTreeMap::new();
        for entry in &entries {
            let date_key = entry.timestamp.naive_utc().date();
            grouped_entries.entry(date_key).or_default().push(entry);
        }

        for (date, entries) in grouped_entries {
            let created_date = entries
                .first()
                .map(|e| e.timestamp.format("%Y%m%d").to_string())
                .unwrap_or_default();
            let formatted_date = date.format("%A, %d/%m/%Y").to_string();
            org_content.push_str(&format!("* {}\n", formatted_date));
            org_content.push_str(&format!(
                ":PROPERTIES:\n:CREATED:  {}\n:END:\n",
                created_date
            ));
            for entry in entries {
                let time = entry.timestamp.format("%H:%M").to_string();
                if let Some(title) = &entry.title {
                    org_content.push_str(&format!("** {} {}\n", time, title));
                } else {
                    org_content.push_str(&format!("** {} \n", time));
                }
                org_content.push_str(&convert_markdown_to_org(&entry.content, 1));
            }
        }

        if output_path.is_some() {
            let path: &str = output_path.as_deref().unwrap();
            // Create directory if it doesn't exist
            if let Some(parent) = Path::new(path).parent() {
                fs::create_dir_all(parent).context("Failed to create output directory")?;
            }

            fs::write(path, org_content).context("Failed to write Org file")?;
        } else {
            println!("{}", org_content);
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct ExportFilters {
    pub date: Option<String>,
    pub since: Option<String>,
    pub until: Option<String>,
    pub journal: Option<String>,
}

/// Convert a markdown string to an org-mode formatted string.
///
/// This is a very basic converted and may not cover all markdown features or edge cases.
fn convert_markdown_to_org(markdown: &str, base_level: u32) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = pulldown_cmark::Parser::new_ext(markdown, options);
    let mut result: String = String::new();

    // Tracking formatting states
    let mut list_depth: usize = 0;
    let mut in_blockquote = false;

    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Emphasis => {
                    result.push('/');
                }
                Tag::Strong => {
                    result.push('*');
                }
                Tag::Strikethrough => {
                    result.push('+');
                }
                Tag::Heading { level, .. } => {
                    result.push_str(&"*".repeat((level as u32 + base_level) as usize));
                    result.push(' ');
                }
                Tag::List(_) => {
                    list_depth += 1;
                    result.push('\n');
                }
                Tag::Item => {
                    let indent = "  ".repeat(list_depth.saturating_sub(1));
                    result.push_str(&format!("{}• ", indent));
                }
                Tag::BlockQuote(_) => {
                    result.push_str("#+BEGIN_QUOTE \n");
                    in_blockquote = true;
                }
                Tag::CodeBlock(t) => {
                    result.push_str("#+BEGIN_SRC ");
                    if let pulldown_cmark::CodeBlockKind::Fenced(lang) = t {
                        result.push_str(&lang);
                    }
                    result.push('\n');
                }
                Tag::Link { dest_url, .. } => {
                    result.push_str(&format!("[[{}][", dest_url));
                }
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Paragraph => {
                    if !in_blockquote {
                        result.push_str("\n\n");
                    } else {
                        result.push('\n');
                    }
                }
                TagEnd::Emphasis => {
                    result.push('/');
                }
                TagEnd::Strong => {
                    result.push('*');
                }
                TagEnd::Strikethrough => {
                    result.push('+');
                }
                TagEnd::Heading(_) => {
                    result.push_str("\n\n");
                }
                TagEnd::List(_) => {
                    list_depth = list_depth.saturating_sub(1);
                    result.push('\n');
                }
                TagEnd::Item => {
                    result.push('\n');
                }
                TagEnd::BlockQuote(_) => {
                    result.push_str("#+END_QUOTE\n\n");
                }
                TagEnd::CodeBlock => {
                    result.push_str("#+END_SRC\n\n");
                }
                TagEnd::Link => {
                    result.push_str("]]");
                }
                _ => {}
            },
            Event::Text(text) => {
                result.push_str(&text);
            }
            Event::Code(text) => {
                result.push_str(&format!("~{}~", text));
            }
            Event::SoftBreak => {
                if in_blockquote {
                    result.push('\n');
                } else {
                    result.push(' ');
                }
            }
            Event::HardBreak => {
                result.push('\n');
            }
            _ => { /* Ignore other events for simplicity */ }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_markdown_to_org() {
        let md = "**Bold**";
        let org = convert_markdown_to_org(md, 0);
        assert_eq!(org.trim(), "*Bold*");
    }
}
