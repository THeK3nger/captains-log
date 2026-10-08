use anyhow::Result;
use axum::{
    Router,
    extract::{Form, Path, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use chrono::{NaiveDateTime, Utc};
use serde::Deserialize;
use std::sync::{Arc, Mutex};

mod templates;

use crate::database::Database;
use crate::journal::{Journal, NewEntry};
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

async fn index_handler(State(state): State<AppState>) -> Html<String> {
    let entries = {
        let j = state.journal.lock().expect("journal lock poisoned");
        j.list_entries().unwrap_or_default()
    };
    Html(templates::page(&entries, None, templates::select_prompt()).into_string())
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
    let detail = match result {
        Ok(Some(entry)) => templates::entry_detail(&entry),
        _ => templates::message_panel("ENTRY NOT FOUND"),
    };

    // HTMX requests get just the partial; direct browser navigation gets the full page
    if headers.contains_key("hx-request") {
        Html(detail.into_string())
    } else {
        let entries = {
            let j = state.journal.lock().expect("journal lock poisoned");
            j.list_entries().unwrap_or_default()
        };
        Html(templates::page(&entries, Some(id), detail).into_string())
    }
}

fn hx_redirect(url: &str) -> Response {
    (
        StatusCode::OK,
        [("HX-Redirect", url.to_string())],
        String::new(),
    )
        .into_response()
}

async fn placeholder_handler() -> Html<String> {
    Html(templates::select_prompt().into_string())
}

async fn new_form_handler(State(state): State<AppState>) -> Html<String> {
    let journals = {
        let j = state.journal.lock().expect("journal lock poisoned");
        j.list_journals().unwrap_or_default()
    };
    Html(templates::entry_form(None, &journals).into_string())
}

async fn create_handler(State(state): State<AppState>, Form(data): Form<EntryForm>) -> Response {
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
        Ok(id) => hx_redirect(&format!("/entry/{id}")),
        Err(_) => {
            Html(templates::message_panel("ERROR SAVING ENTRY").into_string()).into_response()
        }
    }
}

async fn edit_form_handler(State(state): State<AppState>, Path(id): Path<i64>) -> Html<String> {
    let (result, journals) = {
        let j = state.journal.lock().expect("journal lock poisoned");
        (j.get_entry(id), j.list_journals().unwrap_or_default())
    };
    Html(
        match result {
            Ok(Some(entry)) => templates::entry_form(Some(&entry), &journals),
            _ => templates::message_panel("ENTRY NOT FOUND"),
        }
        .into_string(),
    )
}

async fn update_handler(
    State(state): State<AppState>,
    Path(id): Path<i64>,
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
        Ok(true) => hx_redirect(&format!("/entry/{id}")),
        Ok(false) => {
            Html(templates::message_panel("ENTRY NOT FOUND").into_string()).into_response()
        }
        Err(_) => {
            Html(templates::message_panel("ERROR SAVING ENTRY").into_string()).into_response()
        }
    }
}

fn parse_local_form_timestamp(value: &str) -> Result<chrono::DateTime<Utc>> {
    let naive = NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S")
        .or_else(|_| NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M"))?;

    localize(naive, None).ok_or_else(|| anyhow::anyhow!("invalid local timestamp"))
}
