use colored::*;
use pulldown_cmark::{Event, HeadingLevel, Options, Tag, TagEnd};
use terminal_size::{Width, terminal_size};

pub fn render_markdown(content: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = pulldown_cmark::Parser::new_ext(content, options);
    let mut result: String = String::new();

    // Tracking formatting states
    let mut is_italic = false;
    let mut is_bold = false;
    let mut is_strikethrough = false;
    let mut in_code_block = false;
    let mut list_depth: usize = 0;
    let mut in_blockquote = false;

    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Emphasis => {
                    is_italic = true;
                }
                Tag::Strong => {
                    is_bold = true;
                }
                Tag::Strikethrough => {
                    is_strikethrough = true;
                }
                Tag::Heading { level, .. } => {
                    let prefix = match level {
                        HeadingLevel::H1 => "# ",
                        HeadingLevel::H2 => "## ",
                        HeadingLevel::H3 => "### ",
                        HeadingLevel::H4 => "#### ",
                        HeadingLevel::H5 => "##### ",
                        HeadingLevel::H6 => "###### ",
                    };
                    result.push_str(&prefix.bright_blue().bold().to_string());
                }
                Tag::List(_) => {
                    list_depth += 1;
                    result.push('\n');
                }
                Tag::Item => {
                    let indent = "  ".repeat(list_depth.saturating_sub(1));
                    result.push_str(&format!("{}• ", indent).bright_yellow().to_string());
                }
                Tag::BlockQuote(_) => {
                    in_blockquote = true;
                }
                Tag::CodeBlock(_) => {
                    in_code_block = true;
                    result.push_str(&"```".bright_green().to_string());
                    result.push('\n');
                }
                Tag::Link { dest_url, .. } => {
                    result.push('[');
                    result.push_str(&hyperlink_start(&dest_url));
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
                    is_italic = false;
                }
                TagEnd::Strong => {
                    is_bold = false;
                }
                TagEnd::Strikethrough => {
                    is_strikethrough = false;
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
                    in_blockquote = false;
                    result.push('\n');
                }
                TagEnd::CodeBlock => {
                    in_code_block = false;
                    result.push_str(&"```".bright_green().to_string());
                    result.push_str("\n\n");
                }
                TagEnd::Link => {
                    result.push_str(hyperlink_end());
                    result.push(']');
                }
                _ => {}
            },
            Event::Text(text) => {
                let s = if in_code_block {
                    text.to_string().bright_green().to_string()
                } else if in_blockquote {
                    text.lines()
                        .map(|line| format!("│ {}", line).bright_black().to_string())
                        .collect::<Vec<_>>()
                        .join("\n")
                } else {
                    let mut styled = text.as_ref().normal();
                    if is_bold {
                        styled = styled.bold();
                    }
                    if is_italic {
                        styled = styled.italic();
                    }
                    if is_strikethrough {
                        styled = styled.strikethrough();
                    }
                    styled.to_string()
                };

                result.push_str(&s);
            }
            Event::Code(text) => {
                result.push_str(format!("`{}`", text).bright_green().to_string().as_str());
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

// OSC 8 (hyperlink) escape sequences.
// Format: OSC 8 ; ; <URL> ST <TEXT> OSC 8 ; ; ST
// We use ST = ESC \ (can also be BEL \x07, but ESC \ is broadly supported).
// Because this parser is sequential, I need to build the full link using this schema
//
// `\x1b]8;;<URL>\x1b\\<TEXT>\x1b]8;;\x1b\\`
//
// The first part is created by `build_link_start`. Then, the text is added
// "automatically" as a `Event::Text`. Finally, the link is closed with
// `HYPERLINK_END`.

const OSC8_LINK_PREFIX: &str = "\x1b]8;;";
const ST: &str = "\x1b\\";
const OSC8_LINK_END: &str = "\x1b]8;;\x1b\\";

#[inline]
fn hyperlink_start(link_url: &str) -> String {
    // URL must not contain ESC or BEL per OSC 8 requirements
    debug_assert!(!link_url.contains('\x1b') && !link_url.contains('\x07'));

    let mut s = String::with_capacity(OSC8_LINK_PREFIX.len() + link_url.len() + ST.len());
    s.push_str(OSC8_LINK_PREFIX);
    s.push_str(link_url);
    s.push_str(ST);
    s
}

#[inline]
fn hyperlink_end() -> &'static str {
    OSC8_LINK_END
}

/// Get the terminal width for wrapping text, capped at 100 columns.
/// If the terminal size cannot be determined, defaults to 100.
///
/// TODO: 100 is arbitrary, consider making it configurable.
pub fn get_wrap_width() -> u16 {
    let size = terminal_size();
    // Try to get terminal width, fallback to 100
    if let Some((Width(w), _)) = size {
        std::cmp::min(w, 100)
    } else {
        100
    }
}

/// Wrap text to the specified width, preserving existing line breaks.
///
/// TODO: handle ANSI escape codes properly so that they don't count towards the width.
///
/// # Arguments
/// * `text` - The input text to wrap.
/// * `width` - The maximum width of each line.
///
/// # Returns
/// A new `String` with the text wrapped to the specified width.
///
/// # Example
///
/// ```
/// let text = "This is a long line that needs to be wrapped.";
/// let wrapped = wrap_text(text, 20);
/// println!("{}", wrapped);
/// ```
pub fn wrap_text(text: &str, width: u16) -> String {
    use textwrap::{Options, wrap};

    let opts = Options::new(width as usize)
        // Keep long “words” (like very long URLs) from exceeding the width.
        .break_words(true);

    text.lines()
        .map(|line| {
            if line.is_empty() {
                String::new()
            } else {
                wrap(line, &opts).join("\n")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}
