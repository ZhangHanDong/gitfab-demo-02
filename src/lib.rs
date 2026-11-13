use axum::{
    body::{to_bytes, Body, Bytes},
    extract::{Path, Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use chrono::{SecondsFormat, Utc};
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

const MAX_BODY: usize = 64 * 1024;

#[derive(Clone, Serialize)]
pub struct Task {
    id: u64,
    title: String,
    completed: bool,
    created_at: String,
}

#[derive(Default)]
struct Store {
    next_id: u64,
    tasks: BTreeMap<u64, Task>,
}

type Shared = Arc<Mutex<Store>>;

fn error(status: StatusCode, code: &str) -> Response {
    (status, Json(json!({"error": code}))).into_response()
}

async fn limit_body(req: Request, next: Next) -> Response {
    let (parts, body) = req.into_parts();
    match to_bytes(body, MAX_BODY).await {
        Ok(bytes) => {
            next.run(Request::from_parts(parts, Body::from(bytes)))
                .await
        }
        Err(_) => error(StatusCode::PAYLOAD_TOO_LARGE, "payload_too_large"),
    }
}

pub fn app() -> Router {
    let store = Arc::new(Mutex::new(Store {
        next_id: 1,
        ..Store::default()
    }));
    Router::new()
        .route("/health", get(|| async { Json(json!({"status": "ok"})) }))
        .route("/tasks", get(list).post(create))
        .route("/tasks/{id}", get(get_task).delete(delete_task))
        .route("/tasks/{id}/complete", post(complete))
        .with_state(store)
        .layer(middleware::from_fn(limit_body))
}

async fn create(State(store): State<Shared>, body: Bytes) -> Response {
    let title = serde_json::from_slice::<Value>(&body)
        .ok()
        .and_then(|v| v.get("title").and_then(Value::as_str).map(str::to_owned));
    let Some(title) = title else {
        return error(StatusCode::BAD_REQUEST, "invalid_json");
    };
    let title = title.trim();
    if title.is_empty() {
        return error(StatusCode::BAD_REQUEST, "title_empty");
    }
    if title.chars().count() > 200 {
        return error(StatusCode::BAD_REQUEST, "title_too_long");
    }
    let mut store = store.lock().unwrap_or_else(|e| e.into_inner());
    let Some(next_id) = store.next_id.checked_add(1) else {
        return error(StatusCode::INTERNAL_SERVER_ERROR, "id_exhausted");
    };
    let task = Task {
        id: store.next_id,
        title: title.to_owned(),
        completed: false,
        created_at: Utc::now().to_rfc3339_opts(SecondsFormat::AutoSi, true),
    };
    store.next_id = next_id;
    store.tasks.insert(task.id, task.clone());
    (StatusCode::CREATED, Json(task)).into_response()
}

fn parse_id(id: &str) -> Option<u64> {
    if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    id.parse::<u64>().ok().filter(|id| *id > 0)
}

async fn list(State(store): State<Shared>) -> Json<Vec<Task>> {
    Json(
        store
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .tasks
            .values()
            .cloned()
            .collect(),
    )
}

async fn get_task(State(store): State<Shared>, Path(id): Path<String>) -> Response {
    let store = store.lock().unwrap_or_else(|e| e.into_inner());
    match parse_id(&id).and_then(|id| store.tasks.get(&id)) {
        Some(task) => Json(task).into_response(),
        None => error(StatusCode::NOT_FOUND, "not_found"),
    }
}

async fn complete(State(store): State<Shared>, Path(id): Path<String>) -> Response {
    let mut store = store.lock().unwrap_or_else(|e| e.into_inner());
    match parse_id(&id).and_then(|id| store.tasks.get_mut(&id)) {
        Some(task) => {
            task.completed = true;
            Json(task.clone()).into_response()
        }
        None => error(StatusCode::NOT_FOUND, "not_found"),
    }
}

async fn delete_task(State(store): State<Shared>, Path(id): Path<String>) -> Response {
    let mut store = store.lock().unwrap_or_else(|e| e.into_inner());
    match parse_id(&id).and_then(|id| store.tasks.remove(&id)) {
        Some(_) => StatusCode::NO_CONTENT.into_response(),
        None => error(StatusCode::NOT_FOUND, "not_found"),
    }
}
