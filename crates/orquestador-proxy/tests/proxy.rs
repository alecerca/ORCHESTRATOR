//! End-to-end tests: the proxy in front of a stub upstream.
//!
//! A stub `axum` server records what the proxy forwarded (headers/method/path),
//! and the real proxy router is served on an ephemeral port. We assert the role
//! gate, the 401 paths, and that the inject upstream credentials — never the
//! client's — reach upstream.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use axum::{
    extract::{Request, State},
    http::{header::AUTHORIZATION, StatusCode},
    response::IntoResponse,
    Router,
};
use orquestador_proxy::{
    adduser, auth,
    config::{Config, Role},
    proxy::{self, AppState},
};

/// Records every request the stub upstream receives.
#[derive(Default)]
struct Captured {
    authorizations: Vec<String>,
    methods: Vec<String>,
    /// The `x-opencode-directory` header seen per request (`None` when absent).
    directory_headers: Vec<Option<String>>,
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
        cap.directory_headers.push(
            req.headers()
                .get("x-opencode-directory")
                .and_then(|value| value.to_str().ok())
                .map(str::to_string),
        );
    }

    if req.uri().path() == "/event" {
        return (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
            "event: message\ndata: hello\n\n",
        )
            .into_response();
    }

    if req.uri().path() == "/project" {
        return (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            r#"[{"id":"alpha","worktree":"/home/dev/alpha"},{"id":"beta","worktree":"/home/dev/beta"}]"#,
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
    spawn_proxy_with_state(upstream).await.0
}

/// Spawn the proxy and also hand back the shared state, for tests that need to
/// reach into `AppState` (e.g. presence injection).
async fn spawn_proxy_with_state(upstream: &str) -> (String, Arc<AppState>) {
    let config = test_config(upstream);
    let state = Arc::new(AppState::new(config, Some("upstream-secret".to_string())));
    let app = proxy::router(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{addr}"), state)
}

/// A config identical to [`test_config`] but with the instance pinned to `pin`.
fn test_config_with_pin(upstream: &str, pin: &str) -> Config {
    Config::from_json_str(&format!(
        r#"{{
            "listen": "127.0.0.1:0",
            "upstream": "{upstream}",
            "upstream_username": "opencode",
            "project_directory": "{pin}",
            "users": [
                {{ "name": "alice", "token": "editor-token", "role": "editor" }},
                {{ "name": "bob",   "token": "viewer-token", "role": "viewer" }}
            ]
        }}"#
    ))
    .expect("valid pinned test config")
}

/// Spawn the proxy from an arbitrary config; returns its base URL.
async fn spawn_proxy_with_config(config: Config) -> String {
    let state = Arc::new(AppState::new(config, Some("upstream-secret".to_string())));
    let app = proxy::router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}")
}

/// Spawn the proxy with the whole instance pinned to `pin`.
async fn spawn_proxy_pinned(upstream: &str, pin: &str) -> String {
    let config = test_config_with_pin(upstream, pin);
    spawn_proxy_with_config(config).await
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
        assert_eq!(
            seen, &expected,
            "upstream must receive injected credentials"
        );
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
    assert!(
        body.contains("data: hello"),
        "SSE body not passed through: {body:?}"
    );
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

#[tokio::test]
async fn whoami_returns_caller_identity() {
    let (upstream, _captured) = spawn_upstream().await;
    let proxy = spawn_proxy(&upstream).await;
    let client = reqwest::Client::new();

    let editor = client
        .get(format!("{proxy}/orquestador/whoami"))
        .basic_auth("alice", Some("editor-token"))
        .send()
        .await
        .unwrap();
    assert_eq!(editor.status(), StatusCode::OK);
    let body: serde_json::Value = editor.json().await.unwrap();
    assert_eq!(
        body,
        serde_json::json!({ "name": "alice", "role": "editor", "project": null })
    );

    let viewer = client
        .get(format!("{proxy}/orquestador/whoami"))
        .basic_auth("bob", Some("viewer-token"))
        .send()
        .await
        .unwrap();
    assert_eq!(viewer.status(), StatusCode::OK);
    let body: serde_json::Value = viewer.json().await.unwrap();
    assert_eq!(
        body,
        serde_json::json!({ "name": "bob", "role": "viewer", "project": null })
    );
}

#[tokio::test]
async fn whoami_without_credentials_is_unauthorized() {
    let (upstream, captured) = spawn_upstream().await;
    let proxy = spawn_proxy(&upstream).await;

    let response = reqwest::get(format!("{proxy}/orquestador/whoami"))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(
        captured.lock().unwrap().authorizations.is_empty(),
        "unauthenticated /orquestador/ request must never reach upstream"
    );
}

#[tokio::test]
async fn present_lists_recent_caller_with_role_and_small_idle() {
    let (upstream, captured) = spawn_upstream().await;
    let proxy = spawn_proxy(&upstream).await;
    let client = reqwest::Client::new();

    // bob (a viewer) makes a proxied request, which records his presence.
    let request = client
        .get(format!("{proxy}/session"))
        .basic_auth("bob", Some("viewer-token"))
        .send()
        .await
        .unwrap();
    assert_eq!(request.status(), StatusCode::OK);

    // alice polls `/present` — the poll itself is her heartbeat.
    let present = client
        .get(format!("{proxy}/orquestador/present"))
        .basic_auth("alice", Some("editor-token"))
        .send()
        .await
        .unwrap();
    assert_eq!(present.status(), StatusCode::OK);
    let body: serde_json::Value = present.json().await.unwrap();
    let list = body.as_array().expect("present returns an array");

    let by_name: std::collections::HashMap<&str, &serde_json::Value> = list
        .iter()
        .map(|entry| (entry["name"].as_str().unwrap(), entry))
        .collect();

    // A viewer who just acted is listed with a small idleMs.
    let bob = by_name["bob"];
    assert_eq!(bob["role"], serde_json::json!("viewer"));
    let bob_idle = bob["idleMs"].as_u64().unwrap();
    assert!(
        bob_idle < 5_000,
        "bob just made a request; idleMs too large: {bob_idle}"
    );

    // The caller appears too — her poll refreshed her own last-seen.
    let alice = by_name["alice"];
    assert_eq!(alice["role"], serde_json::json!("editor"));
    let alice_idle = alice["idleMs"].as_u64().unwrap();
    assert!(
        alice_idle < 5_000,
        "alice's own poll just ran; idleMs too large: {alice_idle}"
    );

    // Only bob's `/session` reached upstream; the reserved routes never did.
    assert_eq!(
        captured.lock().unwrap().methods.len(),
        1,
        "reserved routes must not be forwarded upstream"
    );
}

#[tokio::test]
async fn present_refreshes_caller_last_seen() {
    let (upstream, _captured) = spawn_upstream().await;
    let proxy = spawn_proxy(&upstream).await;
    let client = reqwest::Client::new();

    let first = client
        .get(format!("{proxy}/orquestador/present"))
        .basic_auth("alice", Some("editor-token"))
        .send()
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);
    let first_body: serde_json::Value = first.json().await.unwrap();
    let first_idle = first_body[0]["idleMs"].as_u64().unwrap();
    assert_eq!(first_body[0]["name"], serde_json::json!("alice"));

    // Wait far longer than the request handling time, then poll again. The
    // second poll is a heartbeat: it must refresh alice's last-seen instead of
    // reporting the elapsed sleep as idle time.
    tokio::time::sleep(Duration::from_millis(1_200)).await;

    let second = client
        .get(format!("{proxy}/orquestador/present"))
        .basic_auth("alice", Some("editor-token"))
        .send()
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::OK);
    let second_body: serde_json::Value = second.json().await.unwrap();
    let second_idle = second_body[0]["idleMs"].as_u64().unwrap();
    assert_eq!(second_body[0]["name"], serde_json::json!("alice"));

    assert!(
        second_idle < 500,
        "a second poll must refresh the caller's last-seen: \
         first idle {first_idle}ms, second idle {second_idle}ms"
    );
}

#[tokio::test]
async fn present_omits_users_older_than_presence_window() {
    let (upstream, _captured) = spawn_upstream().await;
    let (proxy, state) = spawn_proxy_with_state(&upstream).await;
    let client = reqwest::Client::new();

    // bob makes a request, then goes quiet: age his last-seen past the window.
    let request = client
        .get(format!("{proxy}/session"))
        .basic_auth("bob", Some("viewer-token"))
        .send()
        .await
        .unwrap();
    assert_eq!(request.status(), StatusCode::OK);
    state.presence.lock().unwrap().insert(
        "bob".to_string(),
        std::time::Instant::now() - Duration::from_secs(60),
    );

    let present = client
        .get(format!("{proxy}/orquestador/present"))
        .basic_auth("alice", Some("editor-token"))
        .send()
        .await
        .unwrap();
    assert_eq!(present.status(), StatusCode::OK);
    let body: serde_json::Value = present.json().await.unwrap();
    let list = body.as_array().unwrap();

    assert!(
        list.iter()
            .all(|entry| entry["name"] != serde_json::json!("bob")),
        "bob is outside the presence window and must be omitted: {list:?}"
    );
    // The caller's own poll keeps her inside the window.
    assert!(
        list.iter()
            .any(|entry| entry["name"] == serde_json::json!("alice")),
        "the polling caller must appear in her own present list: {list:?}"
    );
}

#[tokio::test]
async fn reserved_prefix_never_reaches_upstream() {
    let (upstream, captured) = spawn_upstream().await;
    let proxy = spawn_proxy(&upstream).await;
    let client = reqwest::Client::new();

    // Valid auth + unknown `/orquestador/` path -> 404, never forwarded.
    let authed = client
        .get(format!("{proxy}/orquestador/sessions"))
        .basic_auth("alice", Some("editor-token"))
        .send()
        .await
        .unwrap();
    assert_eq!(authed.status(), StatusCode::NOT_FOUND);

    // The prefix guard runs *after* auth: no credentials -> 401, not 404.
    let anonymous = reqwest::get(format!("{proxy}/orquestador/sessions"))
        .await
        .unwrap();
    assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);

    let captured = captured.lock().unwrap();
    assert!(
        captured.methods.is_empty(),
        "reserved paths must never reach upstream: {:?}",
        captured.methods
    );
    assert!(
        captured.authorizations.is_empty(),
        "reserved paths must never reach upstream: {:?}",
        captured.authorizations
    );
}

#[tokio::test]
async fn pin_replaces_foreign_directory_header() {
    let (upstream, captured) = spawn_upstream().await;
    let pin = "/home/owner/team-app";
    let proxy = spawn_proxy_pinned(&upstream, pin).await;

    let response = reqwest::Client::new()
        .get(format!("{proxy}/session"))
        .basic_auth("alice", Some("editor-token"))
        .header("x-opencode-directory", "/home/owner/someone-elses-project")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let captured = captured.lock().unwrap();
    assert_eq!(
        captured.directory_headers,
        vec![Some(pin.to_string())],
        "upstream must receive the pinned directory, never the client's"
    );
}

#[tokio::test]
async fn pin_injects_directory_when_client_sends_none() {
    let (upstream, captured) = spawn_upstream().await;
    let pin = "/home/owner/team-app";
    let proxy = spawn_proxy_pinned(&upstream, pin).await;

    let response = reqwest::Client::new()
        .get(format!("{proxy}/session"))
        .basic_auth("alice", Some("editor-token"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let captured = captured.lock().unwrap();
    assert_eq!(
        captured.directory_headers,
        vec![Some(pin.to_string())],
        "a pin injects the directory even when the client sends none"
    );
}

#[tokio::test]
async fn no_pin_forwards_directory_verbatim() {
    let (upstream, captured) = spawn_upstream().await;
    let proxy = spawn_proxy(&upstream).await;
    let client = reqwest::Client::new();

    // With a header: forwarded unchanged, nothing injected.
    let with_header = client
        .get(format!("{proxy}/session"))
        .basic_auth("alice", Some("editor-token"))
        .header("x-opencode-directory", "/home/owner/chosen-project")
        .send()
        .await
        .unwrap();
    assert_eq!(with_header.status(), StatusCode::OK);

    // Without a header: none reaches upstream (no implicit injection).
    let without_header = client
        .get(format!("{proxy}/session"))
        .basic_auth("alice", Some("editor-token"))
        .send()
        .await
        .unwrap();
    assert_eq!(without_header.status(), StatusCode::OK);

    assert_eq!(
        captured.lock().unwrap().directory_headers,
        vec![Some("/home/owner/chosen-project".to_string()), None],
        "without a pin the client's header passes through and nothing is injected"
    );
}

#[tokio::test]
async fn whoami_reports_project_pin() {
    let (upstream, _captured) = spawn_upstream().await;
    let client = reqwest::Client::new();

    let pin = "/home/owner/team-app";
    let pinned = spawn_proxy_pinned(&upstream, pin).await;
    let body: serde_json::Value = client
        .get(format!("{pinned}/orquestador/whoami"))
        .basic_auth("alice", Some("editor-token"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        body,
        serde_json::json!({ "name": "alice", "role": "editor", "project": pin })
    );

    let unpinned = spawn_proxy(&upstream).await;
    let body: serde_json::Value = client
        .get(format!("{unpinned}/orquestador/whoami"))
        .basic_auth("alice", Some("editor-token"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        body,
        serde_json::json!({ "name": "alice", "role": "editor", "project": null })
    );
}

#[tokio::test]
async fn project_list_is_filtered_to_the_pin() {
    let (upstream, _captured) = spawn_upstream().await;
    let proxy = spawn_proxy_pinned(&upstream, "/home/dev/beta").await;

    let response = reqwest::Client::new()
        .get(format!("{proxy}/project"))
        .basic_auth("alice", Some("editor-token"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(
        body,
        serde_json::json!([{ "id": "beta", "worktree": "/home/dev/beta" }]),
        "only the pinned project may be visible"
    );
}

#[tokio::test]
async fn project_list_is_unfiltered_without_a_pin() {
    let (upstream, _captured) = spawn_upstream().await;
    let proxy = spawn_proxy(&upstream).await;

    let response = reqwest::Client::new()
        .get(format!("{proxy}/project"))
        .basic_auth("alice", Some("editor-token"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(
        body.as_array().map(Vec::len),
        Some(2),
        "without a pin the upstream project list passes through: {body:?}"
    );
}
