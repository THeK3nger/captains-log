//! HTML for the web interface. Built with maud, which escapes every
//! interpolated value by default; only `PreEscaped` content is emitted raw.

use super::ListQuery;
use crate::journal::{Entry, TagCount};
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

/// Query string for URLs that keep the active journal, search and tag filters.
pub fn list_query(journal: Option<&str>, search: Option<&str>, tag: Option<&str>) -> String {
    let mut params = Vec::new();
    if let Some(journal) = journal {
        params.push(format!("journal={}", url_encode(journal)));
    }
    if let Some(search) = search {
        params.push(format!("q={}", url_encode(search)));
    }
    if let Some(tag) = tag {
        params.push(format!("tag={}", url_encode(tag)));
    }
    if params.is_empty() {
        String::new()
    } else {
        format!("?{}", params.join("&"))
    }
}

fn divider() -> Markup {
    html! { div.lcars-divider aria-hidden="true" { span {} span {} span {} } }
}

fn back_to_logs(query: &ListQuery) -> Markup {
    html! {
        a.mobile-back href=(format!("/{}", query.url_query()))
            hx-get=(format!("/{}", query.url_query())) hx-target="#entry-detail"
            hx-swap="innerHTML" hx-push-url="true" hx-sync="#entry-detail:replace"
        { "← BACK TO LOGS" }
    }
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
pub fn message_panel(text: &str, filter: &ListQuery) -> Markup {
    html! {
        div.placeholder data-detail-view="message" {
            (back_to_logs(filter))
            div.placeholder-text.alert { (text) }
        }
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

pub fn entry_list(entries: &[Entry], active_id: Option<i64>, filter: &ListQuery) -> Markup {
    if entries.is_empty() {
        return html! {
            div.placeholder style="height:100%" {
                div.placeholder-text {
                    @if filter.search().is_some() { "NO MATCHING ENTRIES" } @else { "NO ENTRIES ON RECORD" }
                }
            }
        };
    }

    let query = filter.url_query();

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
                @if !entry.tags.is_empty() {
                    div.ei-tags {
                        @for tag in &entry.tags { span.tag-chip { "#" (tag) } }
                    }
                }
            }
        }
    }
}

pub fn entry_detail(entry: &Entry, filter: &ListQuery) -> Markup {
    let title = entry.title.as_deref().unwrap_or("UNTITLED ENTRY");
    let date = to_local(&entry.timestamp, None).format("%Y-%m-%d %H:%M:%S");

    let query = filter.url_query();
    html! {
        article data-entry-id=(entry.id) data-detail-view="entry" {
            (back_to_logs(filter))
            div.ed-header {
                h2.ed-title tabindex="-1" { (title) }
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
                (divider())
            }
            @if !entry.tags.is_empty() {
                div.ed-tags {
                    @for tag in &entry.tags {
                        a.tag-chip href=(format!("/{}", list_query(filter.journal(), filter.search(), Some(tag))))
                        { "#" (tag) }
                    }
                }
            }
            @if let Some(path) = &entry.audio_path {
                div.ed-audio { "🎤 " (path) }
            }
            div.ed-content { (PreEscaped(markdown_to_html(&entry.content))) }
        }
    }
}

/// What the user submitted, shown again together with `error` when a save
/// fails, so the text they typed is never lost.
pub struct FormDraft<'a> {
    pub title: &'a str,
    pub journal: &'a str,
    pub tags: &'a str,
    pub timestamp: Option<&'a str>,
    pub content: &'a str,
    pub error: &'a str,
}

/// The create form (`entry` is `None`) or the edit form for an existing entry.
/// With a `draft`, its values replace the entry's and its error is shown above the fields.
pub fn entry_form(
    entry: Option<&Entry>,
    journals: &[String],
    filter: &ListQuery,
    draft: Option<&FormDraft>,
) -> Markup {
    let query = filter.url_query();
    let (action, heading, cancel_target) = match entry {
        Some(e) => (
            format!("/entry/{}/edit", e.id),
            "EDIT ENTRY",
            format!("/entry/{}", e.id),
        ),
        None => ("/entries".to_string(), "NEW ENTRY", "/".to_string()),
    };
    let title = entry.and_then(|e| e.title.as_deref()).unwrap_or("");
    let journal = entry.map_or(filter.journal().unwrap_or("Personal"), |e| {
        e.journal.as_str()
    });
    let content = entry.map_or("", |e| e.content.as_str());
    // Tags written as #tag in the content are picked up on their own; the field
    // holds the others. A new entry starts with the tag being filtered on.
    let mut tags = match entry {
        Some(e) => e.extra_tags().join(", "),
        None => filter.tag().unwrap_or_default(),
    };
    let (mut title, mut journal, mut content) = (title, journal, content);
    let mut timestamp = entry.map(|e| {
        to_local(&e.timestamp, None)
            .format("%Y-%m-%dT%H:%M:%S")
            .to_string()
    });
    if let Some(d) = draft {
        (title, journal, content) = (d.title, d.journal, d.content);
        tags = d.tags.to_string();
        if let Some(value) = d.timestamp {
            timestamp = Some(value.to_string());
        }
    }

    html! {
        div.entry-form data-entry-id=[entry.map(|e| e.id)] data-detail-view="form" {
            (back_to_logs(filter))
            div.form-title-bar { h2.form-heading { (heading) } (divider()) }
            @if let Some(d) = draft {
                div.load-error.form-error role="alert" { (d.error) }
            }
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
                div.form-field {
                    label.form-label for="entry-tags" { "TAGS (OPTIONAL)" }
                    input.form-input #entry-tags type="text" name="tags" value=(tags)
                        placeholder="work, planning — #tags in the content count too" autocomplete="off";
                }
                @if let Some(timestamp) = &timestamp {
                    div.form-field {
                        label.form-label for="entry-timestamp" { "DATE" }
                        input.form-input #entry-timestamp type="datetime-local" name="timestamp" step="1"
                            value=(timestamp);
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
/// `journals` fills the sidebar and `tags` the tag filter; `errors` are shown above the entry list; `filter` scopes the entry list and navigation.
pub fn page(
    entries: &[Entry],
    active_id: Option<i64>,
    detail: Markup,
    journals: &[String],
    tags: &[TagCount],
    errors: &[&str],
    filter: &ListQuery,
) -> Markup {
    let journal = filter.journal();
    let search = filter.search();
    let tag = filter.tag();
    let tag = tag.as_deref();
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
                        div.tb-title { h1.tb-name { "CAPTAIN'S LOG" } }
                        div.tb-seg {}
                        div.tb-seg.sm {}
                        div.tb-cap {}
                    }

                    nav.sidebar aria-label="Journals" {
                        div.sb-stem {}
                        a.sb-btn.c-salmon href=(format!("/form/new{}", filter.url_query()))
                            data-action="new-entry" title="New entry (N)"
                            hx-get=(format!("/form/new{}", filter.url_query()))
                            hx-target="#entry-detail" hx-swap="innerHTML" hx-push-url="true"
                            hx-sync="#entry-detail:replace"
                        { "NEW ENTRY" }
                        div.sb-links {
                            a class=(if journal.is_none() { "sb-btn c-tan active" } else { "sb-btn c-tan" })
                                href=(format!("/{}", list_query(None, search, tag))) aria-current=[journal.is_none().then_some("page")]
                            { "ALL ENTRIES" }
                            @for (i, name) in journals.iter().enumerate() {
                                @let color = JOURNAL_COLORS[i % JOURNAL_COLORS.len()];
                                @let active = journal == Some(name.as_str());
                                a class=(format!("sb-btn c-{color}{}", if active { " active" } else { "" }))
                                    href=(format!("/{}", list_query(Some(name), search, tag)))
                                    aria-current=[active.then_some("page")] title=(name)
                                { span.sb-label { (name) } }
                            }
                        }
                        div.sb-spacer {}
                    }
                    div.mobile-rail aria-hidden="true" {}

                    div.main {
                        section.entry-browser aria-label="Browse logs" {
                            form.list-search action="/" method="get" role="search" {
                                label.form-label for="log-search" { "SEARCH LOGS" }
                                @if let Some(journal) = journal { input type="hidden" name="journal" value=(journal); }
                                @if let Some(tag) = tag { input type="hidden" name="tag" value=(tag); }
                                div.search-controls {
                                    input.form-input #log-search type="search" name="q" value=(search.unwrap_or(""))
                                        placeholder="Title or content…" autocomplete="off";
                                    button.search-submit type="submit" { "SEARCH" }
                                }
                                @if search.is_some() {
                                    a.search-clear href=(format!("/{}", list_query(journal, None, tag))) { "CLEAR SEARCH" }
                                }
                            }
                            @if !tags.is_empty() {
                                details.tag-filter open[tag.is_some()] {
                                    summary.form-label {
                                        "TAGS"
                                        @if let Some(tag) = tag { " · #" (tag) }
                                    }
                                    div.tag-cloud {
                                        @for t in tags {
                                            @let active = tag == Some(t.name.as_str());
                                            // Clicking the active tag turns the filter off
                                            @let target = list_query(journal, search, (!active).then_some(t.name.as_str()));
                                            a.tag-chip.active[active] href=(format!("/{target}"))
                                                aria-current=[active.then_some("true")]
                                            { "#" (t.name) " " span.tag-count { (t.count) } }
                                        }
                                    }
                                }
                            }
                            @for error in errors {
                                div.load-error role="alert" { (error) }
                            }
                            div.entry-list #entry-list aria-label="Entries" {
                                // A failed load must not masquerade as "no entries".
                                @if !(entries.is_empty() && !errors.is_empty()) {
                                    (entry_list(entries, active_id, filter))
                                }
                            }
                        }
                        main.entry-detail #entry-detail aria-label="Entry detail" { (detail) }
                    }

                    div.bl-elbow {}
                    div.bottom-bar {
                        div.bb-text {
                            span.bb-label title=[journal] {
                                (entries.len())
                                @if search.is_some() { " MATCHING ENTRIES" } @else { " ENTRIES ON RECORD" }
                                @if let Some(name) = journal { " · " (name) }
                                @if let Some(tag) = tag { " · #" (tag) }
                            }
                            span.keyboard-help title="J/K: next/previous log · N: new entry · Esc: cancel editing" { "J/K · N · ESC" }
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
