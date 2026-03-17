use anyhow::Result;
use axum::{
    extract::{Path, State},
    http::HeaderMap,
    response::Html,
    routing::get,
    Router,
};
use chrono::Local;
use pulldown_cmark::{html as md_html, Options, Parser as MdParser};
use std::sync::{Arc, Mutex};

use crate::database::Database;
use crate::journal::{Entry, Journal};

#[derive(Clone)]
struct AppState {
    journal: Arc<Mutex<Journal>>,
}

pub fn run(db_path: &std::path::Path, port: u16) -> Result<()> {
    let db = Database::new_with_path(db_path)?;
    let journal = Journal::new(db);
    let state = AppState {
        journal: Arc::new(Mutex::new(journal)),
    };

    tokio::runtime::Runtime::new()?.block_on(async move {
        let app = Router::new()
            .route("/", get(index_handler))
            .route("/entry/{id}", get(entry_handler))
            .with_state(state);

        let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;
        println!("  LCARS interface online → http://localhost:{port}");
        axum::serve(listener, app).await?;
        anyhow::Ok(())
    })
}

async fn index_handler(State(state): State<AppState>) -> Html<String> {
    let entries = {
        let j = state.journal.lock().expect("journal lock poisoned");
        j.list_entries().unwrap_or_default()
    };
    let count = entries.len();
    let list_html = render_entry_list(&entries, None);
    Html(full_page(&list_html, count, PLACEHOLDER_HTML))
}

async fn entry_handler(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Html<String> {
    let result = {
        let j = state.journal.lock().expect("journal lock poisoned");
        j.get_entry(id)
    };
    let detail_html = match result {
        Ok(Some(entry)) => render_entry_detail(&entry),
        _ => r#"<div class="placeholder"><div class="placeholder-text">ENTRY NOT FOUND</div></div>"#.to_string(),
    };

    // HTMX requests get just the partial; direct browser navigation gets the full page
    if headers.contains_key("hx-request") {
        Html(detail_html)
    } else {
        let entries = {
            let j = state.journal.lock().expect("journal lock poisoned");
            j.list_entries().unwrap_or_default()
        };
        let count = entries.len();
        let list_html = render_entry_list(&entries, Some(id));
        Html(full_page(&list_html, count, &detail_html))
    }
}

const PLACEHOLDER_HTML: &str = r#"<div class="placeholder"><div class="placeholder-icon">✦</div><div class="placeholder-text">SELECT AN ENTRY TO VIEW</div></div>"#;

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn to_html(markdown: &str) -> String {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TABLES);
    let parser = MdParser::new_ext(markdown, opts);
    let mut out = String::new();
    md_html::push_html(&mut out, parser);
    out
}

fn render_entry_list(entries: &[Entry], active_id: Option<i64>) -> String {
    if entries.is_empty() {
        return r#"<div class="placeholder" style="height:100%"><div class="placeholder-text">NO ENTRIES ON RECORD</div></div>"#.to_string();
    }
    entries
        .iter()
        .map(|e| {
            let title = e.title.as_deref().unwrap_or("UNTITLED ENTRY");
            let preview: String = e.content.chars().take(80).collect();
            let audio_badge = if e.audio_path.is_some() {
                r#" <span class="audio-badge">🎤</span>"#
            } else {
                ""
            };
            let active = if active_id == Some(e.id) { " active" } else { "" };
            let local_ts = e.timestamp.with_timezone(&Local);
            format!(
                concat!(
                    r#"<div class="entry-item{active}""#,
                    r#" hx-get="/entry/{id}""#,
                    r##" hx-target="#entry-detail""##,
                    r#" hx-swap="innerHTML""#,
                    r#" hx-push-url="true""#,
                    r#" onclick="document.querySelectorAll('.entry-item').forEach(n=>n.classList.remove('active'));this.classList.add('active')">"#,
                    r#"<div class="ei-meta"><span class="ei-date">{date}</span><span class="ei-journal">{journal}</span></div>"#,
                    r#"<div class="ei-title">{title}{audio}</div>"#,
                    r#"<div class="ei-preview">{preview}</div>"#,
                    r#"</div>"#,
                ),
                active = active,
                id = e.id,
                date = local_ts.format("%Y-%m-%d %H:%M"),
                journal = escape_html(&e.journal),
                title = escape_html(title),
                audio = audio_badge,
                preview = escape_html(&preview),
            )
        })
        .collect::<Vec<_>>()
        .join("")
}

fn render_entry_detail(entry: &Entry) -> String {
    let title = entry.title.as_deref().unwrap_or("UNTITLED ENTRY");
    let local_ts = entry.timestamp.with_timezone(&Local);
    let content_html = to_html(&entry.content);
    let audio_section = if let Some(ref path) = entry.audio_path {
        format!(r#"<div class="ed-audio">🎤 {}</div>"#, escape_html(path))
    } else {
        String::new()
    };
    format!(
        concat!(
            r#"<div class="ed-header">"#,
            r#"<div class="ed-title">{title}</div>"#,
            r#"<div class="ed-meta">{date} &nbsp;·&nbsp; {journal} &nbsp;·&nbsp; ID #{id}</div>"#,
            r#"</div>"#,
            r#"{audio}"#,
            r#"<div class="ed-content">{content}</div>"#,
        ),
        title = escape_html(title),
        date = local_ts.format("%Y-%m-%d %H:%M:%S"),
        journal = escape_html(&entry.journal),
        id = entry.id,
        audio = audio_section,
        content = content_html,
    )
}

fn full_page(list_html: &str, count: usize, detail_html: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>CAPTAIN'S LOG — LCARS</title>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link href="https://fonts.googleapis.com/css2?family=Antonio:wght@400;700&family=Exo+2:ital,wght@0,400;0,600;1,400&display=swap" rel="stylesheet">
<script src="https://unpkg.com/htmx.org@1.9.12/dist/htmx.min.js"></script>
<style>{css}</style>
</head>
<body>
<div class="lcars">
  <div class="tl-elbow"></div>
  <div class="top-bar">CAPTAIN'S LOG</div>

  <div class="sidebar">
    <div class="sb-btn" style="background:var(--orange)">ALL ENTRIES</div>
    <div class="sb-filler" style="background:var(--purple);height:40px"></div>
    <div class="sb-filler" style="background:var(--blue);height:40px"></div>
    <div class="sb-filler" style="background:var(--orange);height:80px"></div>
    <div class="sb-filler" style="background:var(--purple);height:35px"></div>
    <div class="sb-filler" style="background:var(--blue);height:35px"></div>
    <div class="sb-filler" style="background:var(--purple);height:35px"></div>
    <div class="sb-spacer"></div>
  </div>

  <div class="main">
    <div class="entry-list" id="entry-list">
      {list_html}
    </div>
    <div class="entry-detail" id="entry-detail">
      {detail_html}
    </div>
  </div>

  <div class="bl-elbow"></div>
  <div class="bottom-bar">
    <span>{count} ENTRIES ON RECORD</span>
    <span class="bottom-bar-version">v{version}</span>
  </div>
</div>
<script>
window.addEventListener('popstate', function() {{
  var m = location.pathname.match(/^\/entry\/(\d+)$/);
  var detail = document.getElementById('entry-detail');
  document.querySelectorAll('.entry-item').forEach(function(n) {{ n.classList.remove('active'); }});
  if (m) {{
    var el = document.querySelector('[hx-get="/entry/' + m[1] + '"]');
    if (el) el.classList.add('active');
    fetch('/entry/' + m[1], {{headers: {{'HX-Request': 'true'}}}})
      .then(function(r) {{ return r.text(); }})
      .then(function(html) {{ detail.innerHTML = html; }});
  }} else {{
    detail.innerHTML = '{placeholder}';
  }}
}});
</script>
</body>
</html>"#,
        css = CSS,
        list_html = list_html,
        count = count,
        detail_html = detail_html,
        placeholder = PLACEHOLDER_HTML,
        version = env!("CARGO_PKG_VERSION"),
    )
}

const CSS: &str = r#"
:root {
  --orange: #FF9900;
  --tan:    #FFCC99;
  --purple: #CC99CC;
  --blue:   #9999CC;
  --bg:     #000;
  --panel-w: 190px;
  --bar-h:   72px;
  --bot-h:   40px;
  --r:       28px;
  --gap:     5px;
}

* { margin: 0; padding: 0; box-sizing: border-box; }

html, body {
  height: 100%;
  background: var(--bg);
  color: var(--tan);
  font-family: 'Antonio', 'Arial Narrow', Arial, sans-serif;
  font-size: 16px;
  overflow: hidden;
}

.lcars {
  height: 100vh;
  display: grid;
  grid-template-rows: var(--bar-h) 1fr var(--bot-h);
  grid-template-columns: var(--panel-w) 1fr;
  gap: var(--gap);
  padding: 8px;
}

.tl-elbow {
  background: var(--orange);
  border-radius: var(--r) 0 0 0;
}

.top-bar {
  background: var(--orange);
  display: flex;
  align-items: flex-end;
  padding: 0 24px 10px;
  font-size: 26px;
  font-weight: 700;
  letter-spacing: 5px;
  color: #000;
  text-transform: uppercase;
}

.sidebar {
  display: flex;
  flex-direction: column;
  gap: 5px;
  overflow: hidden;
}

.sb-btn {
  flex-shrink: 0;
  height: 50px;
  border-radius: var(--r) 0 0 var(--r);
  display: flex;
  align-items: center;
  padding: 0 18px;
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 2px;
  color: #000;
  text-transform: uppercase;
  cursor: default;
}

.sb-filler {
  flex-shrink: 0;
  border-radius: var(--r) 0 0 var(--r);
}

.sb-spacer {
  flex: 1;
  border-radius: var(--r) 0 0 var(--r);
  background: var(--tan);
  min-height: 20px;
}

.main {
  display: flex;
  overflow: hidden;
  gap: 2px;
}

.entry-list {
  width: 320px;
  flex-shrink: 0;
  overflow-y: auto;
  border-right: 1px solid #1a1a1a;
}

.entry-detail {
  flex: 1;
  overflow-y: auto;
  padding: 20px 28px;
}

.bl-elbow {
  background: var(--tan);
  border-radius: 0 0 0 var(--r);
}

.bottom-bar {
  background: var(--tan);
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 24px;
  font-size: 13px;
  font-weight: 700;
  letter-spacing: 2px;
  color: #000;
  text-transform: uppercase;
}

.bottom-bar-version {
  font-size: 11px;
  font-weight: 400;
  letter-spacing: 1px;
  opacity: 0.6;
}

/* Entry list items */
.entry-item {
  padding: 12px 16px;
  cursor: pointer;
  border-bottom: 1px solid #161616;
  transition: background 0.12s;
  border-left: 3px solid transparent;
}

.entry-item:hover { background: #0d0d0d; }

.entry-item.active {
  background: #111;
  border-left-color: var(--orange);
}

.ei-meta {
  display: flex;
  gap: 10px;
  margin-bottom: 4px;
}

.ei-date {
  font-size: 13px;
  color: var(--blue);
}

.ei-journal {
  font-size: 12px;
  color: var(--purple);
  text-transform: uppercase;
  letter-spacing: 1px;
}

.ei-title {
  font-size: 16px;
  color: var(--orange);
  font-weight: 700;
  margin-bottom: 4px;
  line-height: 1.2;
}

.ei-preview {
  font-size: 13px;
  color: #666;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* Entry detail */
.ed-header {
  border-bottom: 2px solid var(--orange);
  padding-bottom: 16px;
  margin-bottom: 22px;
}

.ed-title {
  font-size: 26px;
  font-weight: 700;
  letter-spacing: 2px;
  color: var(--orange);
  text-transform: uppercase;
  margin-bottom: 8px;
}

.ed-meta {
  font-size: 13px;
  color: var(--blue);
  letter-spacing: 1px;
  text-transform: uppercase;
}

.ed-audio {
  font-size: 14px;
  color: var(--purple);
  margin-bottom: 16px;
}

.ed-content {
  font-size: 17px;
  color: var(--tan);
  line-height: 1.75;
  font-family: 'Exo 2', 'Segoe UI', system-ui, sans-serif;
}

.ed-content p { margin-bottom: 14px; }

.ed-content h1, .ed-content h2, .ed-content h3 {
  font-family: 'Antonio', 'Arial Narrow', Arial, sans-serif;
  color: var(--orange);
  letter-spacing: 2px;
  margin: 20px 0 10px;
  text-transform: uppercase;
}

.ed-content strong { color: #fff; }
.ed-content em { color: var(--purple); }

.ed-content code {
  background: #111;
  color: var(--blue);
  padding: 2px 8px;
  border-radius: 3px;
  font-size: 15px;
  font-family: 'Courier New', monospace;
}

.ed-content pre {
  background: #0a0a0a;
  border: 1px solid #222;
  padding: 16px;
  border-radius: 4px;
  overflow-x: auto;
  margin-bottom: 16px;
}

.ed-content pre code {
  background: none;
  padding: 0;
  font-size: 14px;
}

.ed-content ul, .ed-content ol {
  margin: 0 0 14px 22px;
}

.ed-content li { margin-bottom: 4px; }

.ed-content blockquote {
  border-left: 3px solid var(--purple);
  padding-left: 16px;
  color: #999;
  margin-bottom: 14px;
  font-style: italic;
}

/* Placeholder */
.placeholder {
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 14px;
  color: #2a2a2a;
}

.placeholder-icon { font-size: 40px; }

.placeholder-text {
  font-size: 14px;
  letter-spacing: 3px;
  text-transform: uppercase;
}

.audio-badge { font-size: 12px; margin-left: 4px; }

::-webkit-scrollbar { width: 3px; }
::-webkit-scrollbar-track { background: transparent; }
::-webkit-scrollbar-thumb { background: #2a2a2a; border-radius: 2px; }
"#;
