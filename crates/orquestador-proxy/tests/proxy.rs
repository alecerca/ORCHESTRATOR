//! End-to-end tests: the proxy in front of a stub upstream.
//!
//! A stub `axum` server records what the proxy forwarded (headers/method/path),
//! and the real proxy router is served on an ephemeral port. We assert the role
//! gate, the 401 paths, and that the inject upstream credentials — never the
//! client's — reach upstream.

use std::sync::{Arc, Mutex};

use axum::{
    extract::{Request, State},
    http::{header::AUTHORIZATION, StatusCode},
    response::IntoResponse,
    Router,
};
use orquestador_proxy::{
    adduser,
    auth,
    config::{Config, Role},
    proxy::{self, AppState},
};

/// Records every request the stub upstream receives.
#[derive(Default)]
struct Captured {
    authorizations: Vec<String>,
    methods: Vec<String>,
}

async fn stub_upstream(State(cap): State<Arc<Mutex<Captured>>>, req: Request) -> impl IntoResponse {
    {
        let mut cap = cap.lock().unwrap();
        cap.authorizations.push(
            req.headers()
                .get(AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
                .unwrap_or_default()
                .to_string(),
        );
        cap.methods.push(req.method().to_string());
    }

    if req.uri().path() == "/event" {
        return (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
            "event: message\ndata: hello\n\n",
        )
            .into_response();
    }

    (StatusCode::OK, "stub-ok").into_response()
}

/// Spawn the stub upstream; returns its base URL and the shared capture.
async fn spawn_upstream() -> (String, Arc<Mutex<Captured>>) {
    let captured = Arc::new(Mutex::new(Captured::default()));
    let app = Router::new()
        .fallback(stub_upstream)
        .with_state(captured.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{addr}"), captured)
}

fn test_config(upstream: &str) -> Config {
    Config::from_json_str(&format!(
        r#"{{
            "listen": "127.0.0.1:0",
            "upstream": "{upstream}",
            "upstream_username": "opencode",
            "users": [
                {{ "name": "alice", "token": "editor-token", "role": "editor" }},
                {{ "name": "bob",   "token": "viewer-token", "role": "viewer" }}
            ]
        }}"#
    ))
    .expect("valid test config")
}

/// Spawn the proxy; returns its base URL.
async fn spawn_proxy(upstream: &str) -> String {
    let config = test_config(upstream);
    let state = Arc::new(AppState::new(config, Some("upstream-secret".to_string())));
    let app = proxy::router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}")
}

/// Spawn the proxy with a user whose credential is an Argon2id hash, built via
/// the `adduser` core so the write path and the auth path agree.
async fn spawn_proxy_with_hashed_user(upstream: &str) -> String {
    let base = format!(
        r#"{{
            "listen": "127.0.0.1:0",
            "upstream": "{upstream}",
            "upstream_username": "opencode",
            "users": [
                {{ "name": "alice", "token": "editor-token", "role": "editor" }}
            ]
        }}"#
    );
    let json = adduser::add_user_to_json(&base, "dave", Role::Viewer, "hunter2")
        .expect("hashed user added");
    let config = Config::from_json_str(&json).expect("hashed config is valid");
    let state = Arc::new(AppState::new(config, Some("upstream-secret".to_string())));
    let app = proxy::router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}")
}

#[tokio::test]
async fn missing_credentials_are_unauthorized() {
    let (upstream, captured) = spawn_upstream().await;
    let proxy = spawn_proxy(&upstream).await;

    let response = reqwest::get(format!("{proxy}/session")).await.unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(
        captured.lock().unwrap().authorizations.is_empty(),
        "unauthenticated request must never reach upstream"
    );
}

#[tokio::test]
async fn invalid_credentials_are_unauthorized() {
    let (upstream, captured) = spawn_upstream().await;
    let proxy = spawn_proxy(&upstream).await;

    let response = reqwest::Client::new()
        .get(format!("{proxy}/session"))
        .basic_auth("bob", Some("wrong-token"))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(
        captured.lock().unwrap().authorizations.is_empty(),
        "invalid credentials must never reach upstream"
    );
}

#[tokio::test]
async fn unknown_user_is_unauthorized() {
    let (upstream, _captured) = spawn_upstream().await;
    let proxy = spawn_proxy(&upstream).await;

    let response = reqwest::Client::new()
        .get(format!("{proxy}/session"))
        .basic_auth("carol", Some("viewer-token"))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn viewer_reads_but_cannot_write_editor_can_write() {
    let (upstream, captured) = spawn_upstream().await;
    let proxy = spawn_proxy(&upstream).await;
    let client = reqwest::Client::new();

    // viewer GET -> 200
    let viewer_get = client
        .get(format!("{proxy}/session"))
        .basic_auth("bob", Some("viewer-token"))
        .send()
        .await
        .unwrap();
    assert_eq!(viewer_get.status(), StatusCode::OK);

    // viewer POST -> 403
    let viewer_post = client
        .post(format!("{proxy}/session"))
        .basic_auth("bob", Some("viewer-token"))
        .body("{}")
        .send()
        .await
        .unwrap();
    assert_eq!(viewer_post.status(), StatusCode::FORBIDDEN);

    // editor POST -> 200
    let editor_post = client
        .post(format!("{proxy}/session"))
        .basic_auth("alice", Some("editor-token"))
        .body("{}")
        .send()
        .await
        .unwrap();
    assert_eq!(editor_post.status(), StatusCode::OK);

    let captured = captured.lock().unwrap();

    // Exactly one POST (the editor's) reached upstream — the viewer's rejected
    // write never did.
    let posts = captured
        .methods
        .iter()
        .filter(|m| m.as_str() == "POST")
        .count();
    assert_eq!(
        posts, 1,
        "only the editor POST should reach upstream: {:?}",
        captured.methods
    );

    // Every forwarded request carried the injected upstream Basic auth, and
    // never the client's credentials.
    let expected = auth::encode_basic("opencode", "upstream-secret");
    assert!(!captured.authorizations.is_empty());
    for seen in &captured.authorizations {
        assert_eq!(seen, &expected, "upstream must receive injected credentials");
    }
    let client_auth = auth::encode_basic("alice", "editor-token");
    assert!(
        !captured.authorizations.contains(&client_auth),
        "client credentials must never be forwarded upstream"
    );
}

#[tokio::test]
async fn viewer_event_stream_passes_through() {
    let (upstream, _captured) = spawn_upstream().await;
    let proxy = spawn_proxy(&upstream).await;

    let response = reqwest::Client::new()
        .get(format!("{proxy}/event"))
        .basic_auth("bob", Some("viewer-token"))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("text/event-stream"),
    );
    let body = response.text().await.unwrap();
    assert!(body.contains("data: hello"), "SSE body not passed through: {body:?}");
}

#[tokio::test]
async fn hashed_user_authenticates_and_wrong_password_is_unauthorized() {
    let (upstream, captured) = spawn_upstream().await;
    let proxy = spawn_proxy_with_hashed_user(&upstream).await;
    let client = reqwest::Client::new();

    let ok = client
        .get(format!("{proxy}/session"))
        .basic_auth("dave", Some("hunter2"))
        .send()
        .await
        .unwrap();
    assert_eq!(ok.status(), StatusCode::OK);

    let bad = client
        .get(format!("{proxy}/session"))
        .basic_auth("dave", Some("wrong"))
        .send()
        .await
        .unwrap();
    assert_eq!(bad.status(), StatusCode::UNAUTHORIZED);

    assert_eq!(
        captured.lock().unwrap().authorizations.len(),
        1,
        "only the correctly authenticated request should reach upstream"
    );
}
