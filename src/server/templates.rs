//! HTML for the web interface. Built with maud, which escapes every
//! interpolated value by default; only `PreEscaped` content is emitted raw.

use crate::journal::Entry;
use crate::time::to_local;
use maud::{DOCTYPE, Markup, PreEscaped, html};
use pulldown_cmark::{Options, Parser as MdParser, html as md_html};
use std::fmt::Write;

const CSS: &str = include_str!("static/lcars.css");
const JS: &str = include_str!("static/app.js");

/// Colors cycled through for the sidebar journal buttons.
const JOURNAL_COLORS: [&str; 5] = ["purple", "blue", "salmon", "tan", "red"];

/// Percent-encodes a query-string value.
fn url_encode(value: &str) -> String {
    value.bytes().fold(String::new(), |mut out, b| {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => {
                let _ = write!(out, "%{b:02X}");
            }
        }
        out
    })
}

/// `?journal=...` for URLs that should keep the active journal filter, or "".
pub fn journal_query(journal: Option<&str>) -> String {
    journal.map_or_else(String::new, |j| format!("?journal={}", url_encode(j)))
}

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
        div.placeholder { div.placeholder-text.alert { (text) } }
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

pub fn entry_list(entries: &[Entry], active_id: Option<i64>, journal: Option<&str>) -> Markup {
    if entries.is_empty() {
        return html! {
            div.placeholder style="height:100%" {
                div.placeholder-text { "NO ENTRIES ON RECORD" }
            }
        };
    }

    let query = journal_query(journal);

    html! {
        @for entry in entries {
            @let is_active = active_id == Some(entry.id);
            @let title = entry.title.as_deref().unwrap_or("UNTITLED ENTRY");
            @let preview: String = entry.content.chars().take(80).collect();
            a.entry-item.active[is_active]
                data-entry-id=(entry.id)
                href=(format!("/entry/{}{query}", entry.id))
                aria-current=[is_active.then_some("page")]
                hx-get=(format!("/entry/{}{query}", entry.id))
                hx-target="#entry-detail"
                hx-swap="innerHTML"
                hx-push-url="true"
                hx-sync="#entry-detail:replace"
            {
                div.ei-meta {
                    span.ei-date { (to_local(&entry.timestamp, None).format("%Y-%m-%d %H:%M")) }
                    span.ei-journal title=(entry.journal) { (entry.journal) }
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

pub fn entry_detail(entry: &Entry, journal: Option<&str>) -> Markup {
    let title = entry.title.as_deref().unwrap_or("UNTITLED ENTRY");
    let date = to_local(&entry.timestamp, None).format("%Y-%m-%d %H:%M:%S");

    let query = journal_query(journal);
    html! {
        article data-entry-id=(entry.id) {
            div.ed-header {
                div.ed-title { (title) }
                div.ed-meta-row {
                    span.ed-meta {
                        (date) " \u{a0}·\u{a0} " (entry.journal) " \u{a0}·\u{a0} ID #" (entry.id)
                    }
                    a.btn-edit href=(format!("/entry/{}/edit{query}", entry.id))
                        hx-get=(format!("/entry/{}/edit{query}", entry.id))
                        hx-target="#entry-detail"
                        hx-swap="innerHTML"
                        hx-push-url="true"
                        hx-sync="#entry-detail:replace"
                    { "EDIT" }
                }
            }
            @if let Some(path) = &entry.audio_path {
                div.ed-audio { "🎤 " (path) }
            }
            div.ed-content { (PreEscaped(markdown_to_html(&entry.content))) }
        }
    }
}

/// The create form (`entry` is `None`) or the edit form for an existing entry.
pub fn entry_form(entry: Option<&Entry>, journals: &[String], filter: Option<&str>) -> Markup {
    let query = journal_query(filter);
    let (action, heading, cancel_target) = match entry {
        Some(e) => (
            format!("/entry/{}/edit", e.id),
            "EDIT ENTRY",
            format!("/entry/{}", e.id),
        ),
        None => ("/entries".to_string(), "NEW ENTRY", "/".to_string()),
    };
    let title = entry.and_then(|e| e.title.as_deref()).unwrap_or("");
    let journal = entry.map_or(filter.unwrap_or("Personal"), |e| e.journal.as_str());
    let content = entry.map_or("", |e| e.content.as_str());

    html! {
        div.entry-form data-entry-id=[entry.map(|e| e.id)] {
            div.form-title-bar { div.form-heading { (heading) } }
            form action=(format!("{action}{query}")) method="post"
                hx-post=(format!("{action}{query}")) hx-target="#entry-detail" hx-swap="innerHTML"
                hx-disabled-elt="#entry-detail .btn-save" {
                div.form-field {
                    label.form-label for="entry-title" { "TITLE (OPTIONAL)" }
                    input.form-input #entry-title type="text" name="title" value=(title)
                        placeholder="Enter title..." autocomplete="off";
                }
                div.form-field {
                    label.form-label for="entry-journal" { "JOURNAL" }
                    input.form-input #entry-journal type="text" name="journal" value=(journal)
                        list="journal-list" autocomplete="off";
                    datalist id="journal-list" {
                        @for name in journals { option value=(name); }
                    }
                }
                @if let Some(e) = entry {
                    div.form-field {
                        label.form-label for="entry-timestamp" { "DATE" }
                        input.form-input #entry-timestamp type="datetime-local" name="timestamp" step="1"
                            value=(to_local(&e.timestamp, None).format("%Y-%m-%dT%H:%M:%S"));
                    }
                }
                div.form-field {
                    label.form-label for="entry-content" { "CONTENT" }
                    textarea.form-textarea #entry-content name="content" placeholder="Begin recording..." { (content) }
                }
                div.form-actions {
                    button.btn-save type="submit" { "SAVE ENTRY" }
                    a.btn-cancel href=(format!("{cancel_target}{query}"))
                        hx-get=(format!("{cancel_target}{query}")) hx-target="#entry-detail" hx-swap="innerHTML"
                        hx-push-url="true" hx-sync="#entry-detail:replace"
                    { "CANCEL" }
                }
            }
        }
    }
}

/// The full page: sidebar, entry list and `detail` in the main pane.
/// `journals` fills the sidebar; `journal` is the journal the list is filtered to.
pub fn page(
    entries: &[Entry],
    active_id: Option<i64>,
    detail: Markup,
    journals: &[String],
    journal: Option<&str>,
) -> Markup {
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
            body hx-history="false" {
                div.lcars {
                    div.tl-elbow {}
                    div.top-bar {
                        div.tb-title { "CAPTAIN'S LOG" }
                        div.tb-seg {}
                        div.tb-seg.sm {}
                        div.tb-cap {}
                    }

                    nav.sidebar aria-label="Journals" {
                        div.sb-stem {}
                        a.sb-btn.c-salmon href=(format!("/form/new{}", journal_query(journal)))
                            hx-get=(format!("/form/new{}", journal_query(journal)))
                            hx-target="#entry-detail" hx-swap="innerHTML" hx-push-url="true"
                            hx-sync="#entry-detail:replace"
                        { "NEW ENTRY" }
                        div.sb-links {
                            a class=(if journal.is_none() { "sb-btn c-tan active" } else { "sb-btn c-tan" })
                                href="/" aria-current=[journal.is_none().then_some("page")]
                            { "ALL ENTRIES" }
                            @for (i, name) in journals.iter().enumerate() {
                                @let color = JOURNAL_COLORS[i % JOURNAL_COLORS.len()];
                                @let active = journal == Some(name.as_str());
                                a class=(format!("sb-btn c-{color}{}", if active { " active" } else { "" }))
                                    href=(format!("/{}", journal_query(Some(name))))
                                    aria-current=[active.then_some("page")] title=(name)
                                { span.sb-label { (name) } }
                            }
                        }
                        div.sb-spacer {}
                    }
                    div.mobile-rail aria-hidden="true" {}

                    div.main {
                        div.entry-list #entry-list aria-label="Entries" { (entry_list(entries, active_id, journal)) }
                        main.entry-detail #entry-detail aria-label="Entry detail" { (detail) }
                    }

                    div.bl-elbow {}
                    div.bottom-bar {
                        div.bb-text {
                            span.bb-label title=[journal] {
                                (entries.len()) " ENTRIES ON RECORD"
                                @if let Some(name) = journal { " · " (name) }
                            }
                        }
                        div.bb-seg { "v" (env!("CARGO_PKG_VERSION")) }
                        div.bb-cap {}
                    }
                }
                script { (PreEscaped(JS)) }
            }
        }
    }
}
