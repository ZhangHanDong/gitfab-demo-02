use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use serde_json::{json, Value};
use tower::ServiceExt;

async fn call(
    app: &Router,
    method: &str,
    path: &str,
    body: impl Into<Body>,
) -> (StatusCode, Vec<u8>) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .body(body.into())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    (
        status,
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
}

#[tokio::test]
async fn validation_and_boundaries() {
    let app = demo02::app();
    for (body, code) in [
        ("[]", "invalid_json"),
        ("{}", "invalid_json"),
        ("{\"title\":null}", "invalid_json"),
        ("{", "invalid_json"),
        ("{\"title\":\"  \"}", "title_empty"),
    ] {
        let (status, data) = call(&app, "POST", "/tasks", body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            serde_json::from_slice::<Value>(&data).unwrap()["error"],
            code
        );
    }
    let body = json!({"title": "字".repeat(200)}).to_string();
    let (status, data) = call(&app, "POST", "/tasks", body).await;
    assert_eq!(status, StatusCode::CREATED);
    let task: Value = serde_json::from_slice(&data).unwrap();
    let timestamp = task["created_at"].as_str().unwrap();
    assert!(timestamp.ends_with('Z'));
    assert!(chrono::DateTime::parse_from_rfc3339(timestamp).is_ok());
    assert_eq!(
        call(
            &app,
            "POST",
            "/tasks",
            json!({"title": "字".repeat(201)}).to_string()
        )
        .await
        .0,
        400
    );
    let exact = format!(
        "{{\"title\":\"x\",\"padding\":\"{}\"}}",
        "a".repeat(65536 - 26)
    );
    assert_eq!(exact.len(), 65536);
    assert_eq!(call(&app, "POST", "/tasks", exact).await.0, 201);
    assert_eq!(call(&app, "POST", "/tasks", "x".repeat(65537)).await.0, 413);
    for id in ["0", "-1", "abc", "18446744073709551616", "999"] {
        for (method, suffix) in [("GET", ""), ("DELETE", ""), ("POST", "/complete")] {
            let (status, data) = call(&app, method, &format!("/tasks/{id}{suffix}"), "").await;
            assert_eq!(status, 404);
            assert_eq!(
                serde_json::from_slice::<Value>(&data).unwrap(),
                json!({"error":"not_found"})
            );
        }
    }
    let (status, body) = call(&app, "DELETE", "/tasks/1", "").await;
    assert_eq!(status, 204);
    assert!(body.is_empty());
}

#[tokio::test]
async fn concurrent_creation_and_writes() {
    let app = demo02::app();
    let mut jobs = Vec::new();
    for _ in 0..100 {
        let app = app.clone();
        jobs.push(tokio::spawn(async move {
            let (status, body) = call(&app, "POST", "/tasks", r#"{"title":"x"}"#).await;
            assert_eq!(status, 201);
            serde_json::from_slice::<Value>(&body).unwrap()["id"]
                .as_u64()
                .unwrap()
        }));
    }
    let mut ids = Vec::new();
    for job in jobs {
        ids.push(job.await.unwrap());
    }
    ids.sort_unstable();
    assert_eq!(ids, (1..=100).collect::<Vec<_>>());
    let mut jobs = Vec::new();
    for id in 1..=100 {
        let app = app.clone();
        jobs.push(tokio::spawn(async move {
            assert_eq!(
                call(&app, "POST", &format!("/tasks/{id}/complete"), "")
                    .await
                    .0,
                200
            );
        }));
    }
    for job in jobs {
        job.await.unwrap();
    }
    let (_, body) = call(&app, "GET", "/tasks", "").await;
    let tasks: Vec<Value> = serde_json::from_slice(&body).unwrap();
    assert!(tasks
        .iter()
        .enumerate()
        .all(|(i, task)| task["id"] == i + 1 && task["completed"] == true));
}
