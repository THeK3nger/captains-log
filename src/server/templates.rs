//! HTML for the web interface. Built with maud, which escapes every
//! interpolated value by default; only `PreEscaped` content is emitted raw.

use crate::journal::Entry;
use crate::time::to_local;
use maud::{DOCTYPE, Markup, PreEscaped, html};
use pulldown_cmark::{Options, Parser as MdParser, html as md_html};

const CSS: &str = include_str!("static/lcars.css");
const JS: &str = include_str!("static/app.js");

/// Sidebar decoration: (CSS color variable, height in px).
const SIDEBAR_FILLERS: [(&str, u32); 6] = [
    ("purple", 40),
    ("blue", 40),
    ("orange", 80),
    ("purple", 35),
    ("blue", 35),
    ("purple", 35),
];

/// Markdown to HTML. Raw HTML inside the markdown is passed through as written.
fn markdown_to_html(markdown: &str) -> String {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TABLES);
    let mut out = String::new();
    md_html::push_html(&mut out, MdParser::new_ext(markdown, opts));
    out
}

/// A centered message in the detail pane (errors, "not found", ...).
pub fn message_panel(text: &str) -> Markup {
    html! {
        div.placeholder { div.placeholder-text { (text) } }
    }
}

/// The detail pane when nothing is selected.
pub fn select_prompt() -> Markup {
    html! {
        div.placeholder {
            div.placeholder-icon { "✦" }
            div.placeholder-text { "SELECT AN ENTRY TO VIEW" }
        }
    }
}

pub fn entry_list(entries: &[Entry], active_id: Option<i64>) -> Markup {
    if entries.is_empty() {
        return html! {
            div.placeholder style="height:100%" {
                div.placeholder-text { "NO ENTRIES ON RECORD" }
            }
        };
    }

    html! {
        @for entry in entries {
            @let is_active = active_id == Some(entry.id);
            @let title = entry.title.as_deref().unwrap_or("UNTITLED ENTRY");
            @let preview: String = entry.content.chars().take(80).collect();
            div.entry-item.active[is_active]
                hx-get=(format!("/entry/{}", entry.id))
                hx-target="#entry-detail"
                hx-swap="innerHTML"
                hx-push-url="true"
                onclick="document.querySelectorAll('.entry-item').forEach(n=>n.classList.remove('active'));this.classList.add('active')"
            {
                div.ei-meta {
                    span.ei-date { (to_local(&entry.timestamp, None).format("%Y-%m-%d %H:%M")) }
                    span.ei-journal { (entry.journal) }
                }
                div.ei-title {
                    (title)
                    @if entry.audio_path.is_some() {
                        " " span.audio-badge { "🎤" }
                    }
                }
                div.ei-preview { (preview) }
            }
        }
    }
}

pub fn entry_detail(entry: &Entry) -> Markup {
    let title = entry.title.as_deref().unwrap_or("UNTITLED ENTRY");
    let date = to_local(&entry.timestamp, None).format("%Y-%m-%d %H:%M:%S");

    html! {
        div.ed-header {
            div.ed-title { (title) }
            div.ed-meta-row {
                span.ed-meta {
                    (date) " \u{a0}·\u{a0} " (entry.journal) " \u{a0}·\u{a0} ID #" (entry.id)
                }
                button.btn-edit
                    hx-get=(format!("/entry/{}/edit", entry.id))
                    hx-target="#entry-detail"
                    hx-swap="innerHTML"
                { "EDIT" }
            }
        }
        @if let Some(path) = &entry.audio_path {
            div.ed-audio { "🎤 " (path) }
        }
        div.ed-content { (PreEscaped(markdown_to_html(&entry.content))) }
    }
}

/// The create form (`entry` is `None`) or the edit form for an existing entry.
pub fn entry_form(entry: Option<&Entry>, journals: &[String]) -> Markup {
    let (action, heading, cancel_target) = match entry {
        Some(e) => (
            format!("/entry/{}/edit", e.id),
            "EDIT ENTRY",
            format!("/entry/{}", e.id),
        ),
        None => (
            "/entries".to_string(),
            "NEW ENTRY",
            "/placeholder".to_string(),
        ),
    };
    let title = entry.and_then(|e| e.title.as_deref()).unwrap_or("");
    let journal = entry.map_or("Personal", |e| e.journal.as_str());
    let content = entry.map_or("", |e| e.content.as_str());

    html! {
        div.entry-form {
            div.form-title-bar { div.form-heading { (heading) } }
            form hx-post=(action) hx-target="#entry-detail" hx-swap="innerHTML" {
                div.form-field {
                    label.form-label { "TITLE (OPTIONAL)" }
                    input.form-input type="text" name="title" value=(title)
                        placeholder="Enter title..." autocomplete="off";
                }
                div.form-field {
                    label.form-label { "JOURNAL" }
                    input.form-input type="text" name="journal" value=(journal)
                        list="journal-list" autocomplete="off";
                    datalist id="journal-list" {
                        @for name in journals { option value=(name); }
                    }
                }
                @if let Some(e) = entry {
                    div.form-field {
                        label.form-label { "DATE" }
                        input.form-input type="datetime-local" name="timestamp" step="1"
                            value=(to_local(&e.timestamp, None).format("%Y-%m-%dT%H:%M:%S"));
                    }
                }
                div.form-field {
                    label.form-label { "CONTENT" }
                    textarea.form-textarea name="content" placeholder="Begin recording..." { (content) }
                }
                div.form-actions {
                    button.btn-save type="submit" { "SAVE ENTRY" }
                    button.btn-cancel type="button"
                        hx-get=(cancel_target) hx-target="#entry-detail" hx-swap="innerHTML"
                    { "CANCEL" }
                }
            }
        }
    }
}

/// The full page: sidebar, entry list and `detail` in the main pane.
pub fn page(entries: &[Entry], active_id: Option<i64>, detail: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="UTF-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "CAPTAIN'S LOG — LCARS" }
                link rel="preconnect" href="https://fonts.googleapis.com";
                link href="https://fonts.googleapis.com/css2?family=Antonio:wght@400;700&family=Exo+2:ital,wght@0,400;0,600;1,400&display=swap" rel="stylesheet";
                script src="https://unpkg.com/htmx.org@1.9.12/dist/htmx.min.js" {}
                style { (PreEscaped(CSS)) }
            }
            body {
                div.lcars {
                    div.tl-elbow {}
                    div.top-bar { "CAPTAIN'S LOG" }

                    div.sidebar {
                        div.sb-btn style="background:var(--orange)"
                            hx-get="/form/new" hx-target="#entry-detail" hx-swap="innerHTML"
                        { "NEW ENTRY" }
                        @for (color, height) in SIDEBAR_FILLERS {
                            div.sb-filler style=(format!("background:var(--{color});height:{height}px")) {}
                        }
                        div.sb-spacer {}
                    }

                    div.main {
                        div.entry-list #entry-list { (entry_list(entries, active_id)) }
                        div.entry-detail #entry-detail { (detail) }
                    }

                    div.bl-elbow {}
                    div.bottom-bar {
                        span { (entries.len()) " ENTRIES ON RECORD" }
                        span.bottom-bar-version { "v" (env!("CARGO_PKG_VERSION")) }
                    }
                }
                template #placeholder-tpl { (select_prompt()) }
                script { (PreEscaped(JS)) }
            }
        }
    }
}
