use colored::*;
use pulldown_cmark::{Event, HeadingLevel, Options, Tag, TagEnd};

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
                    result.push_str("[");
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
                    result.push_str("]");
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
    return result;
}
