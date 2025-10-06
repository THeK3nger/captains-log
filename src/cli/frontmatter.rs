/// Module for parsing and formatting entries with YAML frontmatter.
/// This is a very hardcoded implementation tailored for CaptainLog's needs.
/// I don't want to introduce too much complexity here.
///
/// So, for now, this just handles the `journal` and `timestamp` fields.
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

const FRONTMATTER_DELIMITER: &str = "---";

#[derive(Debug, Serialize, Deserialize)]
pub struct EntryMetadata {
    pub journal: String,
    pub timestamp: DateTime<Utc>,
}

/// Parse content with YAML frontmatter
/// Returns (metadata, remaining_content)
pub fn parse_frontmatter(content: &str) -> Result<(EntryMetadata, String)> {
    let lines: Vec<&str> = content.lines().collect();

    // Check if content starts with frontmatter delimiter
    if lines.is_empty() || lines[0].trim() != FRONTMATTER_DELIMITER {
        return Err(anyhow::anyhow!(
            "No frontmatter found. Entry must start with '---'"
        ));
    }

    // Find the closing delimiter
    let closing_delimiter_pos = lines
        .iter()
        .skip(1)
        .position(|&line| line.trim() == FRONTMATTER_DELIMITER)
        .context("Frontmatter closing delimiter '---' not found")?;

    // Extract YAML content (between delimiters)
    let yaml_lines = &lines[1..closing_delimiter_pos + 1];
    let yaml_content = yaml_lines.join("\n");

    // Parse YAML
    let metadata: EntryMetadata = serde_yaml::from_str(&yaml_content).context(
        "Failed to parse frontmatter YAML. Check the format of journal and timestamp fields",
    )?;

    // Extract remaining content after frontmatter
    let content_start = closing_delimiter_pos + 2; // +2 to skip the closing delimiter line
    let remaining_content = if content_start < lines.len() {
        lines[content_start..].join("\n")
    } else {
        String::new()
    };

    Ok((metadata, remaining_content))
}

/// Format an entry with YAML frontmatter
pub fn format_entry_with_frontmatter(
    journal: &str,
    timestamp: DateTime<Utc>,
    content: &str,
) -> Result<String> {
    let metadata = EntryMetadata {
        journal: journal.to_string(),
        timestamp,
    };

    let yaml =
        serde_yaml::to_string(&metadata).context("Failed to serialize entry metadata to YAML")?;

    Ok(format!(
        "{}\n{}{}\n{}",
        FRONTMATTER_DELIMITER, yaml, FRONTMATTER_DELIMITER, content
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_parse_frontmatter_valid() {
        let content = r#"---
journal: Work
timestamp: 2025-10-06T14:30:00Z
---

# Entry Title

Entry content"#;

        let result = parse_frontmatter(content);
        assert!(result.is_ok());

        let (metadata, remaining) = result.unwrap();
        assert_eq!(metadata.journal, "Work");
        assert_eq!(
            metadata.timestamp,
            Utc.with_ymd_and_hms(2025, 10, 6, 14, 30, 0).unwrap()
        );
        assert!(remaining.contains("# Entry Title"));
        assert!(remaining.contains("Entry content"));
    }

    #[test]
    fn test_parse_frontmatter_no_delimiter() {
        let content = "Just some content without frontmatter";
        let result = parse_frontmatter(content);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_frontmatter_missing_closing() {
        let content = r#"---
journal: Work
timestamp: 2025-10-06T14:30:00Z

Content without closing delimiter"#;

        let result = parse_frontmatter(content);
        assert!(result.is_err());
    }

    #[test]
    fn test_format_entry_with_frontmatter() {
        let timestamp = Utc.with_ymd_and_hms(2025, 10, 6, 14, 30, 0).unwrap();
        let content = "# Title\n\nContent";

        let result = format_entry_with_frontmatter("Personal", timestamp, content);
        assert!(result.is_ok());

        let formatted = result.unwrap();
        assert!(formatted.starts_with("---"));
        assert!(formatted.contains("journal: Personal"));
        assert!(formatted.contains("timestamp:"));
        assert!(formatted.contains("# Title"));
    }
}
