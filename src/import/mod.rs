use crate::journal::Journal;
use anyhow::{Context, Result};
use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

pub struct Importer<'a> {
    journal: &'a Journal,
}

impl<'a> Importer<'a> {
    pub fn new(journal: &'a Journal) -> Self {
        Self { journal }
    }

    /// Import entries from an org-journal file
    pub fn import_from_org(
        &self,
        file_path: &str,
        journal_category: Option<&str>,
        filter_date: Option<NaiveDate>,
    ) -> Result<ImportStats> {
        let content =
            fs::read_to_string(file_path).context(format!("Failed to read file: {}", file_path))?;

        let entries = parse_org_journal(&content, filter_date)?;

        let mut stats = ImportStats {
            total: entries.len(),
            imported: 0,
            skipped: 0,
            errors: Vec::new(),
        };

        for entry in entries {
            match self.journal.create_entry_with_timestamp(
                entry.title.as_deref(),
                &entry.content,
                journal_category,
                entry.timestamp,
            ) {
                Ok(_) => stats.imported += 1,
                Err(e) => {
                    stats.errors.push(format!(
                        "Failed to import entry at {}: {}",
                        entry.timestamp, e
                    ));
                    stats.skipped += 1;
                }
            }
        }

        Ok(stats)
    }

    /// Import entries from a DayOne JSON export file
    pub fn import_from_dayone(
        &self,
        file_path: &str,
        journal_category: Option<&str>,
        filter_date: Option<NaiveDate>,
    ) -> Result<ImportStats> {
        let content =
            fs::read_to_string(file_path).context(format!("Failed to read file: {}", file_path))?;

        let entries = parse_dayone_json(&content, filter_date)?;

        let mut stats = ImportStats {
            total: entries.len(),
            imported: 0,
            skipped: 0,
            errors: Vec::new(),
        };

        for entry in entries {
            match self.journal.create_entry_with_timestamp(
                entry.title.as_deref(),
                &entry.content,
                journal_category,
                entry.timestamp,
            ) {
                Ok(_) => stats.imported += 1,
                Err(e) => {
                    stats.errors.push(format!(
                        "Failed to import entry at {}: {}",
                        entry.timestamp, e
                    ));
                    stats.skipped += 1;
                }
            }
        }

        Ok(stats)
    }
}

#[derive(Debug)]
pub struct ImportStats {
    pub total: usize,
    pub imported: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone)]
struct ParsedEntry {
    timestamp: NaiveDateTime,
    title: Option<String>,
    content: String,
}

/// Parse an org-journal file and extract entries
fn parse_org_journal(content: &str, filter_date: Option<NaiveDate>) -> Result<Vec<ParsedEntry>> {
    let lines: Vec<&str> = content.lines().collect();
    let mut entries = Vec::new();
    let mut current_date: Option<NaiveDate> = None;
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim();

        // Parse date header (e.g., "* Saturday, 07/09/2025")
        if line.starts_with("* ") && !line.starts_with("** ") {
            let date_str = line.strip_prefix("* ").unwrap().trim();
            current_date = parse_org_date_header(date_str);

            // Skip to after the :PROPERTIES: block
            i += 1;
            while i < lines.len() {
                let prop_line = lines[i].trim();
                if prop_line == ":END:" {
                    i += 1;
                    break;
                }
                i += 1;
            }
            continue;
        }

        // Parse entry header (e.g., "** 14:30 My Title")
        if line.starts_with("** ") {
            if let Some(date) = current_date {
                // Skip if filter_date is set and doesn't match
                if let Some(filter) = filter_date {
                    if date != filter {
                        i += 1;
                        continue;
                    }
                }

                let entry_header = line.strip_prefix("** ").unwrap().trim();
                let (time_str, title) = parse_entry_header(entry_header);

                // Parse timestamp
                if let Some(timestamp) = parse_timestamp(date, time_str) {
                    // Collect entry content until next entry or date header
                    i += 1;
                    let mut content_lines = Vec::new();
                    while i < lines.len() {
                        let content_line = lines[i];
                        if content_line.trim().starts_with("**")
                            || (content_line.trim().starts_with("* ")
                                && !content_line.trim().starts_with("** "))
                        {
                            break;
                        }
                        content_lines.push(content_line);
                        i += 1;
                    }

                    let content = content_lines.join("\n").trim().to_string();
                    let markdown_content = convert_org_to_markdown(&content);

                    entries.push(ParsedEntry {
                        timestamp,
                        title,
                        content: markdown_content,
                    });
                    continue;
                }
            }
        }

        i += 1;
    }

    Ok(entries)
}

/// Parse org-journal date header (e.g., "Saturday, 07/09/2025")
fn parse_org_date_header(date_str: &str) -> Option<NaiveDate> {
    // Extract date part after the comma
    if let Some(date_part) = date_str.split(',').nth(1) {
        let date_part = date_part.trim();
        // Parse "07/09/2025" format (DD/MM/YYYY)
        let parts: Vec<&str> = date_part.split('/').collect();
        if parts.len() == 3 {
            if let (Ok(day), Ok(month), Ok(year)) = (
                parts[0].parse::<u32>(),
                parts[1].parse::<u32>(),
                parts[2].parse::<i32>(),
            ) {
                return NaiveDate::from_ymd_opt(year, month, day);
            }
        }
    }
    None
}

/// Parse entry header to extract time and title
fn parse_entry_header(header: &str) -> (Option<&str>, Option<String>) {
    // Format: "14:30 My Title" or just "14:30"
    let parts: Vec<&str> = header.splitn(2, ' ').collect();
    if parts.is_empty() {
        return (None, None);
    }

    let time_str = parts[0];
    let title = if parts.len() > 1 {
        let title_text = parts[1].trim();
        if title_text.is_empty() {
            None
        } else {
            Some(title_text.to_string())
        }
    } else {
        None
    };

    (Some(time_str), title)
}

/// Parse timestamp from date and time string
fn parse_timestamp(date: NaiveDate, time_str: Option<&str>) -> Option<NaiveDateTime> {
    if let Some(time) = time_str {
        let parts: Vec<&str> = time.split(':').collect();
        if parts.len() >= 2 {
            if let (Ok(hour), Ok(minute)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
                return date.and_hms_opt(hour, minute, 0);
            }
        }
    }
    None
}

/// Convert org-mode format to markdown
fn convert_org_to_markdown(org: &str) -> String {
    let mut result = String::new();
    let lines: Vec<&str> = org.lines().collect();
    let mut in_src_block = false;
    let mut in_quote = false;

    for line in lines {
        let trimmed = line.trim();

        // Handle code blocks
        if trimmed.starts_with("#+BEGIN_SRC") {
            in_src_block = true;
            let lang = trimmed.strip_prefix("#+BEGIN_SRC").unwrap_or("").trim();
            result.push_str(&format!("```{}\n", lang));
            continue;
        }
        if trimmed == "#+END_SRC" {
            in_src_block = false;
            result.push_str("```\n");
            continue;
        }

        // Handle quotes
        if trimmed.starts_with("#+BEGIN_QUOTE") {
            in_quote = true;
            result.push_str("> ");
            continue;
        }
        if trimmed == "#+END_QUOTE" {
            in_quote = false;
            result.push('\n');
            continue;
        }

        // Pass through content in code blocks as-is
        if in_src_block {
            result.push_str(line);
            result.push('\n');
            continue;
        }

        // Convert org formatting to markdown
        let mut converted = line.to_string();

        // Headings: *** -> ###
        if trimmed.starts_with('*') && trimmed.chars().nth(1) != Some('*') {
            let level = trimmed.chars().take_while(|&c| c == '*').count();
            let rest = trimmed.trim_start_matches('*').trim();
            converted = format!("{} {}", "#".repeat(level), rest);
        } else {
            // Bold: *text* -> **text**
            converted = converted.replace("*", "**");

            // Italic: /text/ -> *text*
            converted = convert_delimiter(&converted, '/', '*');

            // Strikethrough: +text+ -> ~~text~~
            converted = convert_delimiter(&converted, '+', '~');

            // Inline code: ~code~ -> `code`
            converted = convert_delimiter(&converted, '~', '`');

            // Links: [[url][text]] -> [text](url)
            converted = convert_org_links(&converted);
        }

        if in_quote && !converted.trim().is_empty() {
            result.push_str("> ");
        }
        result.push_str(&converted);
        result.push('\n');
    }

    result.trim().to_string()
}

/// Convert delimiter-based formatting (helper for org to markdown conversion)
fn convert_delimiter(text: &str, from_delim: char, to_delim: char) -> String {
    let mut result = String::new();
    let mut chars = text.chars().peekable();
    let mut in_delimiter = false;

    while let Some(c) = chars.next() {
        if c == from_delim {
            if in_delimiter {
                result.push(to_delim);
                in_delimiter = false;
            } else {
                result.push(to_delim);
                in_delimiter = true;
            }
        } else {
            result.push(c);
        }
    }

    result
}

/// Convert org-mode links [[url][text]] to markdown [text](url)
fn convert_org_links(text: &str) -> String {
    let mut result = text.to_string();

    // Simple regex-like replacement for [[url][text]] pattern
    while let Some(start) = result.find("[[") {
        if let Some(middle) = result[start..].find("][") {
            if let Some(end) = result[start + middle..].find("]]") {
                let url_start = start + 2;
                let url_end = start + middle;
                let text_start = url_end + 2;
                let text_end = start + middle + end;

                let url = &result[url_start..url_end];
                let link_text = &result[text_start..text_end];

                let markdown_link = format!("[{}]({})", link_text, url);
                result.replace_range(start..text_end + 2, &markdown_link);
            } else {
                break;
            }
        } else {
            break;
        }
    }

    result
}

// DayOne JSON import structures and functions

#[derive(Debug, Deserialize, Serialize)]
struct DayOneExport {
    metadata: DayOneMetadata,
    entries: Vec<DayOneEntry>,
}

#[derive(Debug, Deserialize, Serialize)]
struct DayOneMetadata {
    version: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct DayOneEntry {
    uuid: String,
    creation_date: String,
    modified_date: Option<String>,
    text: String,
    rich_text: Option<String>,
    #[serde(default)]
    starred: bool,
    #[serde(default)]
    is_pinned: bool,
}

#[derive(Debug, Deserialize)]
struct RichTextContent {
    contents: Vec<RichTextBlock>,
}

#[derive(Debug, Deserialize)]
struct RichTextBlock {
    text: String,
    attributes: Option<RichTextAttributes>,
}

#[derive(Debug, Deserialize)]
struct RichTextAttributes {
    line: Option<LineAttributes>,
}

#[derive(Debug, Deserialize)]
struct LineAttributes {
    header: Option<u32>,
}

/// Parse a DayOne JSON export file and extract entries
fn parse_dayone_json(content: &str, filter_date: Option<NaiveDate>) -> Result<Vec<ParsedEntry>> {
    let export: DayOneExport =
        serde_json::from_str(content).context("Failed to parse DayOne JSON file")?;

    let mut entries = Vec::new();

    for dayone_entry in export.entries {
        // Parse timestamp from ISO 8601 format
        let timestamp = chrono::DateTime::parse_from_rfc3339(&dayone_entry.creation_date)
            .context(format!(
                "Failed to parse creation date: {}",
                dayone_entry.creation_date
            ))?
            .naive_utc();

        // Skip if filter_date is set and doesn't match
        if let Some(filter) = filter_date {
            if timestamp.date() != filter {
                continue;
            }
        }

        // Try to extract title from richText if available
        let title = if let Some(rich_text_str) = &dayone_entry.rich_text {
            extract_title_from_rich_text(rich_text_str)
        } else {
            None
        };

        // Use the text field as content (it's already plain text)
        let content = dayone_entry.text.trim().to_string();

        // Skip empty entries
        if content.is_empty() && title.is_none() {
            continue;
        }

        entries.push(ParsedEntry {
            timestamp,
            title,
            content,
        });
    }

    Ok(entries)
}

/// Extract title from DayOne richText JSON
/// The first block with a header attribute is considered the title
fn extract_title_from_rich_text(rich_text_str: &str) -> Option<String> {
    if let Ok(rich_text) = serde_json::from_str::<RichTextContent>(rich_text_str) {
        for block in &rich_text.contents {
            if let Some(attributes) = &block.attributes {
                if let Some(line_attrs) = &attributes.line {
                    if line_attrs.header.is_some() {
                        // Found a header line, use its text as the title
                        let title = block.text.trim().trim_end_matches('\n');
                        if !title.is_empty() {
                            return Some(title.to_string());
                        }
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_org_date_header() {
        let date = parse_org_date_header("Saturday, 07/09/2025");
        assert_eq!(date, NaiveDate::from_ymd_opt(2025, 9, 7));
    }

    #[test]
    fn test_parse_entry_header() {
        let (time, title) = parse_entry_header("14:30 My Title");
        assert_eq!(time, Some("14:30"));
        assert_eq!(title, Some("My Title".to_string()));

        let (time, title) = parse_entry_header("14:30");
        assert_eq!(time, Some("14:30"));
        assert_eq!(title, None);
    }

    #[test]
    fn test_convert_org_to_markdown() {
        let org = "*Bold* /italic/ +strikethrough+ ~code~";
        let md = convert_org_to_markdown(org);
        assert!(md.contains("**Bold**"));
        assert!(md.contains("*italic*"));
    }
}
