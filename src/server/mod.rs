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
use crate::journal::{Entry, EntryFilters, Journal, NewEntry};
use crate::time::localize;

#[derive(Deserialize)]
struct EntryForm {
    title: String,
    content: String,
    journal: String,
    timestamp: Option<String>,
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

/// Journal and literal title/content search filters for page URLs.
#[derive(Deserialize, Default)]
struct ListQuery {
    journal: Option<String>,
    q: Option<String>,
}

impl ListQuery {
    fn journal(&self) -> Option<&str> {
        self.journal.as_deref().filter(|j| !j.is_empty())
    }

    fn search(&self) -> Option<&str> {
        self.q.as_deref().map(str::trim).filter(|q| !q.is_empty())
    }

    fn url_query(&self) -> String {
        templates::list_query(self.journal(), self.search())
    }
}

fn matches_search(title: Option<&str>, content: &str, search: &str) -> bool {
    let search = search.to_lowercase();
    title.is_some_and(|title| title.to_lowercase().contains(&search))
        || content.to_lowercase().contains(&search)
}

/// Everything the page chrome needs: the (filtered) entry list and all journal names.
fn sidebar_data(state: &AppState, query: &ListQuery) -> (Vec<Entry>, Vec<String>) {
    let j = state.journal.lock().expect("journal lock poisoned");
    let entries = match query.journal() {
        Some(name) => j.list_entries_filtered(&EntryFilters {
            journal: Some(name.to_string()),
            ..EntryFilters::default()
        }),
        None => j.list_entries(),
    };
    let mut entries = entries.unwrap_or_default();
    if let Some(search) = query.search() {
        entries.retain(|entry| matches_search(entry.title.as_deref(), &entry.content, search));
    }
    (entries, j.list_journals().unwrap_or_default())
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
        let (entries, journals) = sidebar_data(state, query);
        templates::page(&entries, active_id, detail, &journals, query)
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
        _ => templates::message_panel("ENTRY NOT FOUND", &query),
    };

    page_response(&state, &headers, &query, Some(id), detail)
}

fn save_redirect(headers: &HeaderMap, id: i64, filter: &ListQuery, data: &EntryForm) -> Response {
    let (title, journal) = data.normalized_title_and_journal();
    // Keep a search only if the saved entry still matches it.
    let query = templates::list_query(
        filter.journal().map(|_| journal),
        filter
            .search()
            .filter(|search| matches_search(title, &data.content, search)),
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
        j.list_journals().unwrap_or_default()
    };
    page_response(
        &state,
        &headers,
        &query,
        None,
        templates::entry_form(None, &journals, &query),
    )
}

async fn create_handler(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
    headers: HeaderMap,
    Form(data): Form<EntryForm>,
) -> Response {
    let (title, journal) = data.normalized_title_and_journal();

    let result = {
        let j = state.journal.lock().expect("journal lock poisoned");
        j.create_entry(
            NewEntry::new(data.content.trim())
                .title(title)
                .journal(Some(journal)),
        )
    };

    match result {
        Ok(id) => save_redirect(&headers, id, &query, &data),
        Err(_) => Html(templates::message_panel("ERROR SAVING ENTRY", &query).into_string())
            .into_response(),
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
        (j.get_entry(id), j.list_journals().unwrap_or_default())
    };
    page_response(
        &state,
        &headers,
        &query,
        Some(id),
        match result {
            Ok(Some(entry)) => templates::entry_form(Some(&entry), &journals, &query),
            _ => templates::message_panel("ENTRY NOT FOUND", &query),
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

    let result = {
        let j = state.journal.lock().expect("journal lock poisoned");
        match j.get_entry(id) {
            Ok(Some(entry)) => {
                let timestamp = match data.timestamp.as_deref().map(str::trim) {
                    Some("") | None => Ok(entry.timestamp),
                    Some(value) => parse_local_form_timestamp(value),
                };

                timestamp.and_then(|timestamp| {
                    j.update_entry_with_metadata(id, title, data.content.trim(), journal, timestamp)
                })
            }
            Ok(None) => Ok(false),
            Err(err) => Err(err),
        }
    };

    match result {
        Ok(true) => save_redirect(&headers, id, &query, &data),
        Ok(false) => {
            Html(templates::message_panel("ENTRY NOT FOUND", &query).into_string()).into_response()
        }
        Err(_) => Html(templates::message_panel("ERROR SAVING ENTRY", &query).into_string())
            .into_response(),
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
        };
        let (entries, _) = sidebar_data(&state, &filter);
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
        };
        let (entries, _) = sidebar_data(&state, &literal);
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
            }),
        )
        .await;
        assert_eq!(result.status(), StatusCode::SEE_OTHER);
        assert_eq!(result.headers()["location"], "/entry/2");
    }
}
