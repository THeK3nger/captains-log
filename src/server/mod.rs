use anyhow::Result;
use axum::{
    Router,
    extract::{Form, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use chrono::{NaiveDateTime, Utc};
use serde::Deserialize;
use std::sync::{Arc, Mutex};

mod templates;

use crate::database::Database;
use crate::journal::{Entry, EntryFilters, Journal, NewEntry, TagCount, normalize_tag};
use crate::time::localize;

#[derive(Deserialize)]
struct EntryForm {
    title: String,
    content: String,
    journal: String,
    timestamp: Option<String>,
    /// Extra tags, separated by commas or spaces. `None` (field absent) keeps the entry's current ones.
    tags: Option<String>,
}

impl EntryForm {
    /// Title (`None` when blank) and journal (defaulting to "Personal"), both trimmed.
    fn normalized_title_and_journal(&self) -> (Option<&str>, &str) {
        let title = Some(self.title.trim()).filter(|title| !title.is_empty());
        let journal = Some(self.journal.trim())
            .filter(|journal| !journal.is_empty())
            .unwrap_or("Personal");
        (title, journal)
    }
}

/// Tag names from the form's tags field. `Err` holds the first invalid one.
fn parse_form_tags(raw: &str) -> Result<Vec<String>, String> {
    raw.split(|c: char| c == ',' || c.is_whitespace())
        .filter(|tag| !tag.is_empty())
        .map(|tag| normalize_tag(tag).ok_or_else(|| tag.to_string()))
        .collect()
}

fn invalid_tag_message(tag: &str) -> String {
    format!(
        "INVALID TAG: {} — TAGS START WITH A LETTER AND USE ONLY LETTERS, DIGITS, - AND _",
        tag.to_uppercase()
    )
}

/// Show the form again with everything the user submitted and `message` above it,
/// so a failed save never costs them their text. `id` is the entry being edited.
fn form_error_response(
    state: &AppState,
    headers: &HeaderMap,
    query: &ListQuery,
    id: Option<i64>,
    data: &EntryForm,
    message: &str,
) -> Response {
    let (entry, journals) = {
        let j = state.journal.lock().expect("journal lock poisoned");
        (id.map(|id| j.get_entry(id)), j.list_journals())
    };
    let detail = match entry {
        Some(Ok(None)) => templates::message_panel("ENTRY NOT FOUND", query),
        Some(Err(err)) => {
            eprintln!("unable to load entry for editing: {err:#}");
            templates::message_panel("UNABLE TO LOAD ENTRY", query)
        }
        entry => {
            let entry = entry.and_then(Result::ok).flatten();
            let journals = journals.unwrap_or_default();
            let draft = templates::FormDraft {
                title: &data.title,
                journal: &data.journal,
                tags: data.tags.as_deref().unwrap_or_default(),
                timestamp: data.timestamp.as_deref(),
                content: &data.content,
                error: message,
            };
            templates::entry_form(entry.as_ref(), &journals, query, Some(&draft))
        }
    };
    page_response(state, headers, query, id, detail).into_response()
}

/// Journal, tag and literal title/content search filters for page URLs.
#[derive(Deserialize, Default)]
struct ListQuery {
    journal: Option<String>,
    q: Option<String>,
    tag: Option<String>,
}

impl ListQuery {
    fn journal(&self) -> Option<&str> {
        self.journal.as_deref().filter(|j| !j.is_empty())
    }

    fn search(&self) -> Option<&str> {
        self.q.as_deref().map(str::trim).filter(|q| !q.is_empty())
    }

    /// The tag filter, normalized (`None` if absent or not a valid tag).
    fn tag(&self) -> Option<String> {
        self.tag.as_deref().and_then(normalize_tag)
    }

    fn url_query(&self) -> String {
        templates::list_query(self.journal(), self.search(), self.tag().as_deref())
    }
}

fn matches_search(title: Option<&str>, content: &str, search: &str) -> bool {
    let search = search.to_lowercase();
    title.is_some_and(|title| title.to_lowercase().contains(&search))
        || content.to_lowercase().contains(&search)
}

/// Everything the page chrome needs: the (filtered) entry list, all journal names
/// and tags, plus a message for each load that failed.
struct SidebarData {
    entries: Vec<Entry>,
    journals: Vec<String>,
    tags: Vec<TagCount>,
    errors: Vec<&'static str>,
}

/// Unwraps a load result, logging a failure and recording `message` instead of hiding it.
fn load_or_report<T: Default>(
    result: Result<T>,
    message: &'static str,
    errors: &mut Vec<&'static str>,
) -> T {
    result.unwrap_or_else(|err| {
        eprintln!("{message}: {err:#}");
        errors.push(message);
        T::default()
    })
}

fn sidebar_data(state: &AppState, query: &ListQuery) -> SidebarData {
    let j = state.journal.lock().expect("journal lock poisoned");
    let mut errors = Vec::new();
    let entries = j.list_entries_filtered(&EntryFilters {
        journal: query.journal().map(str::to_string),
        tags: query.tag().into_iter().collect(),
        ..EntryFilters::default()
    });
    let mut entries = load_or_report(entries, "UNABLE TO LOAD ENTRIES", &mut errors);
    if let Some(search) = query.search() {
        entries.retain(|entry| matches_search(entry.title.as_deref(), &entry.content, search));
    }
    let journals = load_or_report(j.list_journals(), "UNABLE TO LOAD JOURNALS", &mut errors);
    let tags = load_or_report(j.list_tags(), "UNABLE TO LOAD TAGS", &mut errors);
    SidebarData {
        entries,
        journals,
        tags,
        errors,
    }
}

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
            .route("/placeholder", get(placeholder_handler))
            .route("/entry/{id}", get(entry_handler))
            .route("/form/new", get(new_form_handler))
            .route("/entries", post(create_handler))
            .route(
                "/entry/{id}/edit",
                get(edit_form_handler).post(update_handler),
            )
            .with_state(state);

        let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;
        println!("  LCARS interface online → http://localhost:{port}");
        axum::serve(listener, app).await?;
        anyhow::Ok(())
    })
}

async fn index_handler(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
    headers: HeaderMap,
) -> Html<String> {
    page_response(&state, &headers, &query, None, templates::select_prompt())
}

/// History cache misses must receive the full page, even when HX-Request is set.
fn is_partial_request(headers: &HeaderMap) -> bool {
    headers.get("hx-request").is_some_and(|v| v == "true")
        && !headers
            .get("hx-history-restore-request")
            .is_some_and(|v| v == "true")
}

fn page_response(
    state: &AppState,
    headers: &HeaderMap,
    query: &ListQuery,
    active_id: Option<i64>,
    detail: maud::Markup,
) -> Html<String> {
    let markup = if is_partial_request(headers) {
        detail
    } else {
        let data = sidebar_data(state, query);
        templates::page(
            &data.entries,
            active_id,
            detail,
            &data.journals,
            &data.tags,
            &data.errors,
            query,
        )
    };
    Html(markup.into_string())
}

async fn entry_handler(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(query): Query<ListQuery>,
    headers: HeaderMap,
) -> Html<String> {
    let result = {
        let j = state.journal.lock().expect("journal lock poisoned");
        j.get_entry(id)
    };
    let detail = match result {
        Ok(Some(entry)) => templates::entry_detail(&entry, &query),
        Ok(None) => templates::message_panel("ENTRY NOT FOUND", &query),
        Err(err) => {
            eprintln!("unable to load entry {id}: {err:#}");
            templates::message_panel("UNABLE TO LOAD ENTRY", &query)
        }
    };

    page_response(&state, &headers, &query, Some(id), detail)
}

/// Tags of a just-saved entry (empty if they can't be loaded).
fn saved_tags(journal: &Journal, id: i64) -> Vec<String> {
    journal
        .get_entry(id)
        .ok()
        .flatten()
        .map(|entry| entry.tags)
        .unwrap_or_default()
}

fn save_redirect(
    headers: &HeaderMap,
    id: i64,
    filter: &ListQuery,
    data: &EntryForm,
    saved_tags: &[String],
) -> Response {
    let (title, journal) = data.normalized_title_and_journal();
    // Keep a search only if the saved entry still matches it, and a tag
    // filter only if the entry still has the tag.
    let query = templates::list_query(
        filter.journal().map(|_| journal),
        filter
            .search()
            .filter(|search| matches_search(title, &data.content, search)),
        filter
            .tag()
            .as_deref()
            .filter(|tag| saved_tags.iter().any(|t| t == tag)),
    );
    let url = format!("/entry/{id}{query}");
    if !is_partial_request(headers) {
        return Redirect::to(&url).into_response();
    }
    (StatusCode::OK, [("HX-Redirect", url)], String::new()).into_response()
}

async fn placeholder_handler() -> Html<String> {
    Html(templates::select_prompt().into_string())
}

async fn new_form_handler(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
    headers: HeaderMap,
) -> Html<String> {
    let journals = {
        let j = state.journal.lock().expect("journal lock poisoned");
        j.list_journals()
    };
    let detail = match journals {
        Ok(journals) => templates::entry_form(None, &journals, &query, None),
        Err(err) => {
            eprintln!("unable to load journals: {err:#}");
            templates::message_panel("UNABLE TO LOAD JOURNALS", &query)
        }
    };
    page_response(&state, &headers, &query, None, detail)
}

async fn create_handler(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
    headers: HeaderMap,
    Form(data): Form<EntryForm>,
) -> Response {
    let (title, journal) = data.normalized_title_and_journal();
    let tags = match parse_form_tags(data.tags.as_deref().unwrap_or_default()) {
        Ok(tags) => tags,
        Err(tag) => {
            let message = invalid_tag_message(&tag);
            return form_error_response(&state, &headers, &query, None, &data, &message);
        }
    };

    let result = {
        let j = state.journal.lock().expect("journal lock poisoned");
        j.create_entry(
            NewEntry::new(data.content.trim())
                .title(title)
                .journal(Some(journal))
                .tags(&tags),
        )
        .map(|id| (id, saved_tags(&j, id)))
    };

    match result {
        Ok((id, tags)) => save_redirect(&headers, id, &query, &data, &tags),
        Err(err) => {
            eprintln!("unable to save entry: {err:#}");
            form_error_response(&state, &headers, &query, None, &data, "ERROR SAVING ENTRY")
        }
    }
}

async fn edit_form_handler(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(query): Query<ListQuery>,
    headers: HeaderMap,
) -> Html<String> {
    let (result, journals) = {
        let j = state.journal.lock().expect("journal lock poisoned");
        (j.get_entry(id), j.list_journals())
    };
    page_response(
        &state,
        &headers,
        &query,
        Some(id),
        match (result, journals) {
            (Ok(Some(entry)), Ok(journals)) => {
                templates::entry_form(Some(&entry), &journals, &query, None)
            }
            (Ok(None), _) => templates::message_panel("ENTRY NOT FOUND", &query),
            (Err(err), _) | (_, Err(err)) => {
                eprintln!("unable to load entry {id} for editing: {err:#}");
                templates::message_panel("UNABLE TO LOAD ENTRY", &query)
            }
        },
    )
}

async fn update_handler(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(query): Query<ListQuery>,
    headers: HeaderMap,
    Form(data): Form<EntryForm>,
) -> Response {
    let (title, journal) = data.normalized_title_and_journal();
    let form_tags = match data.tags.as_deref().map(parse_form_tags).transpose() {
        Ok(tags) => tags,
        Err(tag) => {
            let message = invalid_tag_message(&tag);
            return form_error_response(&state, &headers, &query, Some(id), &data, &message);
        }
    };

    let result = {
        let j = state.journal.lock().expect("journal lock poisoned");
        match j.get_entry(id) {
            Ok(Some(entry)) => {
                let extra_tags = form_tags.unwrap_or_else(|| entry.extra_tags());
                let timestamp = match data.timestamp.as_deref().map(str::trim) {
                    Some("") | None => Ok(entry.timestamp),
                    Some(value) => parse_local_form_timestamp(value),
                };

                timestamp.and_then(|timestamp| {
                    j.update_entry_with_metadata(
                        id,
                        title,
                        data.content.trim(),
                        journal,
                        timestamp,
                        &extra_tags,
                    )
                })
            }
            Ok(None) => Ok(false),
            Err(err) => Err(err),
        }
    };

    match result {
        Ok(true) => {
            let tags = {
                let j = state.journal.lock().expect("journal lock poisoned");
                saved_tags(&j, id)
            };
            save_redirect(&headers, id, &query, &data, &tags)
        }
        Ok(false) => {
            Html(templates::message_panel("ENTRY NOT FOUND", &query).into_string()).into_response()
        }
        Err(err) => {
            eprintln!("unable to save entry {id}: {err:#}");
            form_error_response(
                &state,
                &headers,
                &query,
                Some(id),
                &data,
                "ERROR SAVING ENTRY",
            )
        }
    }
}

fn parse_local_form_timestamp(value: &str) -> Result<chrono::DateTime<Utc>> {
    let naive = NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S")
        .or_else(|_| NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M"))?;

    localize(naive, None).ok_or_else(|| anyhow::anyhow!("invalid local timestamp"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> AppState {
        AppState {
            journal: Arc::new(Mutex::new(Journal::new(
                Database::new_with_path(":memory:").unwrap(),
            ))),
        }
    }

    fn query(journal: &str) -> Query<ListQuery> {
        Query(ListQuery {
            journal: Some(journal.into()),
            ..ListQuery::default()
        })
    }

    fn hx_headers() -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("hx-request", "true".parse().unwrap());
        headers
    }

    #[tokio::test]
    async fn failed_loads_are_reported_instead_of_shown_as_empty() {
        let db = Database::new_with_path(":memory:").unwrap();
        db.connection().execute("DROP TABLE entries", []).unwrap();
        let state = AppState {
            journal: Arc::new(Mutex::new(Journal::new(db))),
        };
        let page = index_handler(
            State(state.clone()),
            Query(ListQuery::default()),
            HeaderMap::new(),
        )
        .await
        .0;
        assert!(page.contains("UNABLE TO LOAD ENTRIES"));
        assert!(page.contains("UNABLE TO LOAD JOURNALS"));
        assert!(!page.contains("NO ENTRIES ON RECORD"));

        let detail = entry_handler(
            State(state),
            Path(1),
            Query(ListQuery::default()),
            hx_headers(),
        )
        .await
        .0;
        assert!(detail.contains("UNABLE TO LOAD ENTRY"));
    }

    #[tokio::test]
    async fn search_combines_with_journal_and_matches_titles_or_content_literally() {
        let state = state();
        let title_id;
        let content_id;
        {
            let journal = state.journal.lock().unwrap();
            title_id = journal
                .create_entry(
                    NewEntry::new("50%_ready")
                        .title(Some("Navigation"))
                        .journal(Some("Ship Operations")),
                )
                .unwrap();
            content_id = journal
                .create_entry(
                    NewEntry::new("Navigation array online").journal(Some("Ship Operations")),
                )
                .unwrap();
            journal
                .create_entry(NewEntry::new("Navigation array online").journal(Some("Personal")))
                .unwrap();
        }
        let filter = ListQuery {
            journal: Some("Ship Operations".into()),
            q: Some(" NAVIGATION ".into()),
            ..ListQuery::default()
        };
        let entries = sidebar_data(&state, &filter).entries;
        let mut ids: Vec<_> = entries.iter().map(|entry| entry.id).collect();
        ids.sort();
        assert_eq!(ids, [title_id, content_id]);
        let page = index_handler(State(state.clone()), Query(filter), HeaderMap::new())
            .await
            .0;
        assert!(page.contains("2 MATCHING ENTRIES"));
        assert!(page.contains(&format!(
            "/entry/{title_id}?journal=Ship%20Operations&amp;q=NAVIGATION"
        )));

        let literal = ListQuery {
            journal: Some("Ship Operations".into()),
            q: Some("%_".into()),
            ..ListQuery::default()
        };
        let entries = sidebar_data(&state, &literal).entries;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, title_id);
        let empty = index_handler(
            State(state),
            Query(ListQuery {
                q: Some("missing phrase".into()),
                ..ListQuery::default()
            }),
            HeaderMap::new(),
        )
        .await
        .0;
        assert!(empty.contains("NO MATCHING ENTRIES"));
    }

    #[tokio::test]
    async fn saves_preserve_search_when_the_entry_matches_and_clear_it_when_it_does_not() {
        let state = state();
        let filter = || {
            Query(ListQuery {
                journal: Some("Ship Operations".into()),
                q: Some("navigation".into()),
                ..ListQuery::default()
            })
        };
        let result = create_handler(
            State(state.clone()),
            filter(),
            hx_headers(),
            Form(EntryForm {
                title: "Navigation".into(),
                content: "Online".into(),
                journal: "Ship Operations".into(),
                timestamp: None,
                tags: None,
            }),
        )
        .await;
        assert_eq!(
            result.headers()["hx-redirect"],
            "/entry/1?journal=Ship%20Operations&q=navigation"
        );
        let result = update_handler(
            State(state),
            Path(1),
            filter(),
            hx_headers(),
            Form(EntryForm {
                title: "Medical report".into(),
                content: "Crew healthy".into(),
                journal: "Personal".into(),
                timestamp: None,
                tags: None,
            }),
        )
        .await;
        assert_eq!(result.headers()["hx-redirect"], "/entry/1?journal=Personal");
    }

    #[tokio::test]
    async fn history_cache_misses_restore_full_filtered_pages_and_forms() {
        let state = state();
        let name = "Research & Development / α";
        let id = {
            let journal = state.journal.lock().unwrap();
            journal
                .create_entry(NewEntry::new("Filtered log").journal(Some(name)))
                .unwrap()
        };
        let partial = entry_handler(State(state.clone()), Path(id), query(name), hx_headers())
            .await
            .0;
        assert!(!partial.contains("<!DOCTYPE html>"));

        let mut headers = hx_headers();
        headers.insert("hx-history-restore-request", "true".parse().unwrap());
        let pages = [
            index_handler(State(state.clone()), query(name), headers.clone())
                .await
                .0,
            entry_handler(State(state.clone()), Path(id), query(name), headers.clone())
                .await
                .0,
            new_form_handler(State(state.clone()), query(name), headers.clone())
                .await
                .0,
            edit_form_handler(State(state), Path(id), query(name), headers)
                .await
                .0,
        ];
        for page in pages {
            assert!(page.starts_with("<!DOCTYPE html>"));
            assert!(page.contains("1 ENTRIES ON RECORD"));
            assert!(page.contains("?journal=Research%20%26%20Development%20%2F%20%CE%B1"));
        }
    }

    #[tokio::test]
    async fn filtered_saves_keep_the_saved_entry_visible_and_native_posts_redirect() {
        let state = state();
        let name = "Research & Development / α";
        let result = create_handler(
            State(state.clone()),
            query(name),
            hx_headers(),
            Form(EntryForm {
                title: " Test ".into(),
                content: "Log".into(),
                journal: name.into(),
                timestamp: None,
                tags: None,
            }),
        )
        .await;
        assert_eq!(
            result.headers()["hx-redirect"],
            "/entry/1?journal=Research%20%26%20Development%20%2F%20%CE%B1"
        );

        // Moving an entry while filtered follows its new journal so selection
        // and the entry list stay consistent after the save.
        let result = update_handler(
            State(state.clone()),
            Path(1),
            query(name),
            hx_headers(),
            Form(EntryForm {
                title: "Test".into(),
                content: "Updated log".into(),
                journal: "Ship Operations".into(),
                timestamp: None,
                tags: None,
            }),
        )
        .await;
        assert_eq!(
            result.headers()["hx-redirect"],
            "/entry/1?journal=Ship%20Operations"
        );
        assert_eq!(
            state
                .journal
                .lock()
                .unwrap()
                .get_entry(1)
                .unwrap()
                .unwrap()
                .journal,
            "Ship Operations"
        );

        let result = create_handler(
            State(state),
            Query(ListQuery::default()),
            HeaderMap::new(),
            Form(EntryForm {
                title: "".into(),
                content: "Native log".into(),
                journal: name.into(),
                timestamp: None,
                tags: None,
            }),
        )
        .await;
        assert_eq!(result.status(), StatusCode::SEE_OTHER);
        assert_eq!(result.headers()["location"], "/entry/2");
    }

    fn tag_form(content: &str, tags: Option<&str>) -> Form<EntryForm> {
        Form(EntryForm {
            title: "".into(),
            content: content.into(),
            journal: "Personal".into(),
            timestamp: None,
            tags: tags.map(str::to_string),
        })
    }

    fn tag_query(tag: &str) -> Query<ListQuery> {
        Query(ListQuery {
            tag: Some(tag.into()),
            ..ListQuery::default()
        })
    }

    #[test]
    fn form_tags_split_on_commas_and_spaces() {
        assert_eq!(
            parse_form_tags("Work, #planning  q4,,").unwrap(),
            ["work", "planning", "q4"]
        );
        assert!(parse_form_tags("").unwrap().is_empty());
        assert_eq!(parse_form_tags("ok, 9bad"), Err("9bad".to_string()));
    }

    #[tokio::test]
    async fn form_tags_and_inline_tags_are_saved_and_filterable() {
        let state = state();
        let result = create_handler(
            State(state.clone()),
            Query(ListQuery::default()),
            hx_headers(),
            tag_form("Talked about #pricing", Some("urgent, work")),
        )
        .await;
        assert_eq!(result.headers()["hx-redirect"], "/entry/1");
        create_handler(
            State(state.clone()),
            Query(ListQuery::default()),
            hx_headers(),
            tag_form("No tags here", None),
        )
        .await;

        let tagged = |tag: &str| {
            let data = sidebar_data(&state, &tag_query(tag).0);
            data.entries.iter().map(|e| e.id).collect::<Vec<_>>()
        };
        assert_eq!(tagged("urgent"), [1]);
        assert_eq!(tagged("PRICING"), [1]);
        assert!(tagged("nope").is_empty());

        let data = sidebar_data(&state, &ListQuery::default());
        assert_eq!(data.tags.len(), 3);

        let page = index_handler(State(state.clone()), tag_query("urgent"), HeaderMap::new())
            .await
            .0;
        assert!(page.contains("TAGS · #urgent"));
        assert!(page.contains("1 MATCHING") || page.contains("1 ENTRIES ON RECORD"));
        assert!(page.contains("#pricing"));
        // the active tag's chip links to the unfiltered list
        assert!(page.contains(r#"href="/" aria-current="true""#), "{page}");
    }

    async fn body_of(response: Response) -> String {
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        String::from_utf8_lossy(&body).into_owned()
    }

    #[tokio::test]
    async fn invalid_form_tag_keeps_the_typed_text_and_saves_nothing() {
        let state = state();
        let form = Form(EntryForm {
            title: "My <title>".into(),
            content: "Long text I would hate to lose".into(),
            journal: "Work".into(),
            timestamp: None,
            tags: Some("fine, no good!".into()),
        });

        let response = create_handler(
            State(state.clone()),
            Query(ListQuery::default()),
            hx_headers(),
            form,
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let html = body_of(response).await;
        assert!(html.contains("INVALID TAG: GOOD!"), "{html}");
        assert!(html.contains("Long text I would hate to lose"));
        assert!(html.contains(r#"value="My &lt;title&gt;""#));
        assert!(html.contains(r#"value="Work""#));
        assert!(html.contains(r#"value="fine, no good!""#));
        assert!(html.contains("<form"), "the form must stay on screen");
        assert!(
            state
                .journal
                .lock()
                .unwrap()
                .list_entries()
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn failed_edit_keeps_the_typed_text_and_the_entry_unchanged() {
        let state = state();
        create_handler(
            State(state.clone()),
            Query(ListQuery::default()),
            hx_headers(),
            tag_form("Original", None),
        )
        .await;

        let bad_tag = update_handler(
            State(state.clone()),
            Path(1),
            Query(ListQuery::default()),
            hx_headers(),
            Form(EntryForm {
                title: "".into(),
                content: "Rewritten text".into(),
                journal: "Personal".into(),
                timestamp: None,
                tags: Some("2025".into()),
            }),
        )
        .await;
        let html = body_of(bad_tag).await;
        assert!(html.contains("INVALID TAG: 2025"));
        assert!(html.contains("Rewritten text"));
        assert!(html.contains(r#"action="/entry/1/edit""#));

        // Same for any other failure, e.g. a malformed date
        let bad_date = update_handler(
            State(state.clone()),
            Path(1),
            Query(ListQuery::default()),
            hx_headers(),
            Form(EntryForm {
                title: "".into(),
                content: "Rewritten text".into(),
                journal: "Personal".into(),
                timestamp: Some("not a date".into()),
                tags: None,
            }),
        )
        .await;
        let html = body_of(bad_date).await;
        assert!(html.contains("ERROR SAVING ENTRY"));
        assert!(html.contains("Rewritten text"));
        assert!(html.contains(r#"value="not a date""#) || html.contains("entry-timestamp"));

        let entry = state.journal.lock().unwrap().get_entry(1).unwrap().unwrap();
        assert_eq!(entry.content, "Original");
    }

    #[tokio::test]
    async fn updates_replace_extra_tags_or_keep_them_when_the_field_is_absent() {
        let state = state();
        create_handler(
            State(state.clone()),
            Query(ListQuery::default()),
            hx_headers(),
            tag_form("Body #inline", Some("extra")),
        )
        .await;
        let tags = || {
            state
                .journal
                .lock()
                .unwrap()
                .get_entry(1)
                .unwrap()
                .unwrap()
                .tags
        };

        update_handler(
            State(state.clone()),
            Path(1),
            Query(ListQuery::default()),
            hx_headers(),
            tag_form("Edited #inline", None),
        )
        .await;
        assert_eq!(tags(), ["extra", "inline"]);

        update_handler(
            State(state.clone()),
            Path(1),
            Query(ListQuery::default()),
            hx_headers(),
            tag_form("Edited #inline", Some("")),
        )
        .await;
        assert_eq!(tags(), ["inline"]);

        // A tag filter survives a save only if the entry still has the tag
        let kept = update_handler(
            State(state.clone()),
            Path(1),
            tag_query("inline"),
            hx_headers(),
            tag_form("Edited #inline", Some("")),
        )
        .await;
        assert_eq!(kept.headers()["hx-redirect"], "/entry/1?tag=inline");
        let dropped = update_handler(
            State(state),
            Path(1),
            tag_query("inline"),
            hx_headers(),
            tag_form("No tag now", Some("")),
        )
        .await;
        assert_eq!(dropped.headers()["hx-redirect"], "/entry/1");
    }
}
