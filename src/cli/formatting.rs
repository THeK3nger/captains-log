use colored::*;
use pulldown_cmark::{Event, Parser, Tag, TagEnd};

pub fn render_markdown(content: &str) -> String {
    let parser = pulldown_cmark::Parser::new(content);
    let mut result: String = String::new();

    // Tracking formatting states
    let mut is_italic = false;
    let mut is_bold = false;

    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Emphasis => {
                    is_italic = true;
                }
                Tag::Strong => {
                    is_bold = true;
                }
                _ => {
                    continue;
                }
            },
            Event::End(tag) => match tag {
                TagEnd::Paragraph => {
                    result.push_str("\n\n");
                }
                TagEnd::Emphasis => {
                    is_italic = false;
                }
                TagEnd::Strong => {
                    is_bold = false;
                }
                _ => {
                    continue;
                }
            },
            Event::Text(text) => {
                let mut s = text.to_string();

                s = if is_bold && is_italic {
                    s.bold().italic().to_string()
                } else if is_bold {
                    s.bold().to_string()
                } else if is_italic {
                    s.italic().to_string()
                } else {
                    s.normal().to_string()
                };

                result.push_str(&s);
            }
            Event::Code(text) => {
                result.push_str(format!("`{}`", text).bright_green().to_string().as_str());
            }
            Event::SoftBreak => {
                // Append a end line for soft breaks
                result.push_str("ENDLINE");
            }
            Event::HardBreak => {
                result.push('\n');
            }
            _ => { /* Ignore other events for simplicity */ }
        }
    }
    return result;
}
