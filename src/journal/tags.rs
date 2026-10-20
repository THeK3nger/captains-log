//! Tag syntax: normalization and extraction of inline `#tags` from entry text.

use std::ops::Range;

fn is_tag_char(c: char) -> bool {
    c.is_alphanumeric() || c == '-' || c == '_'
}

/// Characters that may directly precede the `#` of a tag (besides whitespace).
fn is_tag_opener(c: char) -> bool {
    c.is_whitespace() || matches!(c, '(' | '[' | '{' | '"' | '\'')
}

/// Canonical form of a tag name: no leading `#`, lowercase. `None` if it is not
/// a valid tag (must start with a letter, then letters, digits, `-` or `_`).
pub fn normalize_tag(raw: &str) -> Option<String> {
    let name = raw.trim().trim_start_matches('#');
    let mut chars = name.chars();
    let first = chars.next()?;
    if !first.is_alphabetic() || !chars.all(is_tag_char) {
        return None;
    }
    Some(name.to_lowercase())
}

/// Every inline `#tag` in `text` with its byte range (including the `#`) and
/// normalized name, in order of appearance, duplicates included.
///
/// A tag must start the text or follow whitespace or an opening bracket/quote,
/// so URL fragments don't count, and its first character must be a letter, so
/// `#123` and `# Heading` don't either. Fenced code blocks and inline code are
/// skipped.
pub fn scan_tags(text: &str) -> Vec<(Range<usize>, String)> {
    let mut found = Vec::new();
    let mut in_fence = false;
    let mut line_start = 0;

    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
        } else if !in_fence {
            scan_line(line, line_start, &mut found);
        }
        line_start += line.len();
    }
    found
}

fn scan_line(line: &str, offset: usize, found: &mut Vec<(Range<usize>, String)>) {
    let mut in_code = false;
    let mut prev: Option<char> = None;
    let mut chars = line.char_indices().peekable();

    while let Some((start, c)) = chars.next() {
        if c == '`' {
            in_code = !in_code;
        } else if c == '#' && !in_code && prev.is_none_or(is_tag_opener) {
            let mut end = start + 1;
            while let Some(&(index, next)) = chars.peek() {
                if !is_tag_char(next) {
                    break;
                }
                end = index + next.len_utf8();
                chars.next();
            }
            let raw = line[start + 1..end].trim_end_matches(['-', '_']);
            if let Some(name) = normalize_tag(raw) {
                let tag_end = start + 1 + raw.len();
                found.push((offset + start..offset + tag_end, name));
            }
            prev = line[..end].chars().next_back();
            continue;
        }
        prev = Some(c);
    }
}

/// Distinct normalized inline tags of `text`, in order of first appearance.
pub fn extract_tags(text: &str) -> Vec<String> {
    let mut tags: Vec<String> = Vec::new();
    for (_, name) in scan_tags(text) {
        if !tags.contains(&name) {
            tags.push(name);
        }
    }
    tags
}

/// `text` with every inline `#old` tag rewritten to `#new` (both normalized).
pub fn rename_in_text(text: &str, old: &str, new: &str) -> String {
    let mut result = text.to_string();
    for (range, name) in scan_tags(text).into_iter().rev() {
        if name == old {
            result.replace_range(range, &format!("#{new}"));
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_valid_names() {
        assert_eq!(normalize_tag("Work"), Some("work".into()));
        assert_eq!(normalize_tag("#Q4-plan_b"), Some("q4-plan_b".into()));
        assert_eq!(normalize_tag("  città "), Some("città".into()));
    }

    #[test]
    fn rejects_invalid_names() {
        for raw in ["", "#", "123", "9lives", "two words", "a,b", "-x", "a#b"] {
            assert_eq!(normalize_tag(raw), None, "{raw:?}");
        }
    }

    #[test]
    fn extracts_inline_tags_in_order_without_duplicates() {
        let text = "Call with Ana about #Pricing and #work.\nMore #work and #pricing!";
        assert_eq!(extract_tags(text), ["pricing", "work"]);
    }

    #[test]
    fn tag_may_start_text_or_follow_brackets_and_quotes() {
        assert_eq!(
            extract_tags("#a (#b) [#c] \"#d\" '#e'"),
            ["a", "b", "c", "d", "e"]
        );
    }

    #[test]
    fn ignores_non_tags() {
        let text =
            "# Heading\n## Another\nissue #123, url https://x.org/#anchor, a#b, #!/bin/sh, #";
        assert!(extract_tags(text).is_empty());
    }

    #[test]
    fn trailing_punctuation_is_not_part_of_the_tag() {
        assert_eq!(
            extract_tags("done #ship-it, #v2_ and #x-"),
            ["ship-it", "v2", "x"]
        );
    }

    #[test]
    fn ignores_code() {
        let text = "use `#include` here\n```c\n#define X\n```\n~~~\n#nope\n~~~\n#real";
        assert_eq!(extract_tags(text), ["real"]);
    }

    #[test]
    fn rename_rewrites_only_matching_tags() {
        let text = "#Old and `#old` and #older, #old.\n```\n#old\n```";
        assert_eq!(
            rename_in_text(text, "old", "new"),
            "#new and `#old` and #older, #new.\n```\n#old\n```"
        );
    }

    #[test]
    fn scan_returns_byte_ranges() {
        let text = "è #tag";
        let (range, name) = scan_tags(text).remove(0);
        assert_eq!(&text[range], "#tag");
        assert_eq!(name, "tag");
    }
}
