//! The axum server: authentication, role gate, upstream forwarding, audit.
//!
//! Most requests flow through the fallback handler:
//! 1. authenticate the inbound Basic credentials against `proxy.json`;
//! 2. record the caller's presence (last-seen) — before the role gate, so a
//!    read-only viewer is still "online";
//! 3. reject any `/orquestador/` path (the prefix is reserved for proxy-native
//!    routes and is never forwarded upstream);
//! 4. authorize the method against the user's role (viewer = safe methods only);
//! 5. forward to `opencode serve` with the injected upstream credentials and
//!    stream the response back (SSE included).
//!
//! Two proxy-native routes are registered ahead of the fallback:
//! `GET /orquestador/whoami` (the caller's identity) and
//! `GET /orquestador/present` (who is online; the poll doubles as the caller's
//! heartbeat, so no separate presence write is needed).
//!
//! The proxy is the trust boundary: client credentials never leave it, and an
//! unauthenticated request never reaches upstream.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::{
    body::Body,
    extract::{Request, State},
    http::{header, HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri},
    response::{IntoResponse, Json, Response},
    routing::get,
    Router,
};
use reqwest::Client;
use serde_json::json;

use crate::{
    auth, authorize,
    config::{Config, Role},
};

/// Upper bound on buffered request bodies. OpenCode writes are small JSON
/// documents; this only bounds the memory a single write can pin.
const MAX_REQUEST_BODY: usize = 16 * 1024 * 1024;

/// Presence window: users whose last request is older than this are omitted
/// from `/present`. ~3× the app's 15s poll interval, so a client that only
/// polls stays present between polls.
const PRESENCE_WINDOW: Duration = Duration::from_secs(45);

/// Reserved prefix for proxy-native routes; never forwarded upstream.
const RESERVED_PREFIX: &str = "/orquestador/";

/// The `x-opencode-directory` request header OpenCode uses to scope a request
/// to a project directory. `orquestador-core` declares the same header as
/// `DIRECTORY_HEADER` (`crates/orquestador-core/src/client.rs`); the proxy is a
/// standalone crate and deliberately does not depend on the core, so the
/// literal is repeated here.
const DIRECTORY_HEADER: &str = "x-opencode-directory";

/// Shared server state: the parsed config plus the injected upstream credentials.
pub struct AppState {
    pub config: Config,
    pub upstream_base: String,
    pub upstream_username: String,
    pub upstream_password: String,
    pub client: Client,
    /// Presence: member name → last-seen (monotonic instant). Written by
    /// `handle` after authentication and by the `/orquestador/present` poll
    /// itself; in-memory only, so a proxy restart clears presence.
    pub presence: Mutex<HashMap<String, Instant>>,
}

impl AppState {
    /// Build state from a validated config and the resolved upstream password
    /// (env-preferred; `None` becomes an empty password).
    pub fn new(config: Config, upstream_password: Option<String>) -> Self {
        let upstream_base = config.upstream.trim_end_matches('/').to_string();
        Self {
            upstream_username: config.upstream_username.clone(),
            upstream_password: upstream_password.unwrap_or_default(),
            config,
            upstream_base,
            client: Client::new(),
            presence: Mutex::new(HashMap::new()),
        }
    }
}

/// Build the proxy router. The proxy-native `/orquestador/` routes are
/// registered ahead of the fallback, which handles every other method and path
/// — this keeps SSE `/event` and any future upstream path on one code path.
pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/orquestador/whoami", get(whoami))
        .route("/orquestador/present", get(present))
        .fallback(handle)
        .with_state(state)
}

/// Handle one proxied request.
pub async fn handle(State(state): State<Arc<AppState>>, req: Request) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();

    // 1. Authenticate. Missing / malformed / unknown / wrong-token -> 401.
    let Some((name, role)) = authenticate(&state, &req) else {
        audit(
            "-",
            "-",
            &method,
            &path,
            StatusCode::UNAUTHORIZED,
            "unauthenticated",
        );
        return unauthorized();
    };

    // 2. Record presence before the role gate so a read-only viewer still
    //    counts as online.
    state
        .presence
        .lock()
        .unwrap()
        .insert(name.clone(), Instant::now());

    // 3. The `/orquestador/` prefix is reserved for proxy-native routes and is
    //    never forwarded upstream.
    if path.starts_with(RESERVED_PREFIX) {
        audit(
            &name,
            role.as_str(),
            &method,
            &path,
            StatusCode::NOT_FOUND,
            "reserved",
        );
        return (StatusCode::NOT_FOUND, "not found").into_response();
    }

    // 4. Role gate. A viewer write is denied before any upstream contact.
    if !authorize::authorize(method.as_str(), role) {
        audit(
            &name,
            role.as_str(),
            &method,
            &path,
            StatusCode::FORBIDDEN,
            "denied",
        );
        return forbidden();
    }

    // 5. Forward and stream the response back. The one exception is
    //    `GET /project` under a pin: that list is buffered and filtered so a
    //    client cannot enumerate the host's other projects.
    let pinned_project_list =
        state.config.project_directory.is_some() && method == Method::GET && path == "/project";

    let response = if pinned_project_list {
        forward_project(&state, req).await
    } else {
        match forward(&state, req).await {
            Ok(response) => response,
            Err(err) => upstream_error(err),
        }
    };

    audit(
        &name,
        role.as_str(),
        &method,
        &path,
        response.status(),
        "allowed",
    );
    response
}

/// `GET /orquestador/whoami` — the caller's own identity, doubling as a clean
/// credential check for the frontend. Also reports the pinned `project` (`null`
/// when the instance is unrestricted). Read-only; presence is recorded by the
/// `/present` poll (the heartbeat) and by proxied requests, not here.
async fn whoami(State(state): State<Arc<AppState>>, req: Request) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();

    let Some((name, role)) = authenticate(&state, &req) else {
        audit(
            "-",
            "-",
            &method,
            &path,
            StatusCode::UNAUTHORIZED,
            "unauthenticated",
        );
        return unauthorized();
    };

    audit(
        &name,
        role.as_str(),
        &method,
        &path,
        StatusCode::OK,
        "allowed",
    );
    (
        StatusCode::OK,
        Json(json!({
            "name": name,
            "role": role.as_str(),
            "project": state.config.project_directory.as_deref(),
        })),
    )
        .into_response()
}

/// `GET /orquestador/present` — who is online: one entry per user seen within
/// the presence window, sorted by name for deterministic output. The poll IS
/// the heartbeat: it refreshes the caller's own last-seen, so an idle-but-alive
/// client stays present.
async fn present(State(state): State<Arc<AppState>>, req: Request) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();

    let Some((name, role)) = authenticate(&state, &req) else {
        audit(
            "-",
            "-",
            &method,
            &path,
            StatusCode::UNAUTHORIZED,
            "unauthenticated",
        );
        return unauthorized();
    };

    let now = Instant::now();
    let entries: Vec<serde_json::Value> = {
        let mut presence = state.presence.lock().unwrap();
        presence.insert(name.clone(), now);

        let mut seen: Vec<(String, &'static str, u64)> = presence
            .iter()
            .filter_map(|(member, last_seen)| {
                let idle = now.saturating_duration_since(*last_seen);
                if idle > PRESENCE_WINDOW {
                    return None;
                }
                Some((
                    member.clone(),
                    state.config.lookup(member)?.role.as_str(),
                    idle.as_millis() as u64,
                ))
            })
            .collect();
        seen.sort_by(|a, b| a.0.cmp(&b.0));
        seen.into_iter()
            .map(|(member, role, idle_ms)| {
                json!({ "name": member, "role": role, "idleMs": idle_ms })
            })
            .collect()
    };

    audit(
        &name,
        role.as_str(),
        &method,
        &path,
        StatusCode::OK,
        "allowed",
    );
    (StatusCode::OK, Json(entries)).into_response()
}

/// Resolve the inbound `Authorization: Basic` header to `(name, role)`.
fn authenticate(state: &AppState, req: &Request) -> Option<(String, Role)> {
    let value = req.headers().get(header::AUTHORIZATION)?.to_str().ok()?;
    let (name, token) = auth::parse_basic(value)?;
    let user = auth::resolve_user(&state.config, &name, &token)?;
    Some((user.name.clone(), user.role))
}

/// Forward the request to upstream with injected credentials and stream back.
async fn forward(state: &AppState, req: Request) -> Result<Response, reqwest::Error> {
    let (parts, body) = req.into_parts();

    let body_bytes = match axum::body::to_bytes(body, MAX_REQUEST_BODY).await {
        Ok(bytes) => bytes,
        Err(_) => {
            return Ok((
                StatusCode::PAYLOAD_TOO_LARGE,
                "request body exceeds the proxy limit",
            )
                .into_response());
        }
    };

    let outgoing = build_upstream_request(state, parts.method, &parts.uri, &parts.headers);
    let upstream = outgoing.body(body_bytes).send().await?;

    let status = upstream.status();
    let headers = upstream.headers().clone();
    // `Body::from_stream` drives reqwest's byte stream directly, so SSE frames
    // are relayed as they arrive instead of being buffered.
    let stream = upstream.bytes_stream();
    Ok(build_response(status, &headers, Body::from_stream(stream)))
}

/// `GET /project` under a pin: buffer the upstream project list and keep only
/// the pinned project, so a client can never enumerate the host's others. This
/// is the only place the proxy buffers a response body — `/event` keeps
/// streaming. A non-2xx status, or a body that is not a JSON array of objects,
/// is passed through unchanged rather than replaced with an invented body.
async fn forward_project(state: &AppState, req: Request) -> Response {
    let Some(pin) = state.config.project_directory.as_deref() else {
        // Unreachable from `handle`'s guard; forward verbatim to be safe.
        return match forward(state, req).await {
            Ok(response) => response,
            Err(err) => upstream_error(err),
        };
    };

    let (parts, body) = req.into_parts();

    let body_bytes = match axum::body::to_bytes(body, MAX_REQUEST_BODY).await {
        Ok(bytes) => bytes,
        Err(_) => {
            return (
                StatusCode::PAYLOAD_TOO_LARGE,
                "request body exceeds the proxy limit",
            )
                .into_response();
        }
    };

    let outgoing = build_upstream_request(state, parts.method, &parts.uri, &parts.headers);
    let upstream = match outgoing.body(body_bytes).send().await {
        Ok(response) => response,
        Err(err) => return upstream_error(err),
    };

    let status = upstream.status();
    let headers = upstream.headers().clone();
    let bytes = match upstream.bytes().await {
        Ok(bytes) => bytes,
        Err(err) => return upstream_error(err),
    };

    if status.is_success() {
        if let Some(filtered) = filter_projects_json(&bytes, pin) {
            let body = serde_json::to_vec(&filtered).unwrap_or_else(|_| bytes.to_vec());
            return build_response(status, &headers, Body::from(body));
        }
    }

    build_response(status, &headers, Body::from(bytes))
}

/// Build the upstream request: URL, injected Basic auth, and the forwarded
/// headers with the project-directory pin rule applied. Shared by the streaming
/// [`forward`] path and the buffered [`forward_project`] path.
///
/// Without a pin the client's `x-opencode-directory` passes through verbatim and
/// nothing is injected; with a pin the client's value is dropped and the pinned
/// value is injected instead, so no client can leave the pinned project.
fn build_upstream_request(
    state: &AppState,
    method: Method,
    uri: &Uri,
    headers: &HeaderMap,
) -> reqwest::RequestBuilder {
    let mut url = format!("{}{}", state.upstream_base, uri.path());
    if let Some(query) = uri.query() {
        url.push('?');
        url.push_str(query);
    }

    let mut outgoing = state
        .client
        .request(method, &url)
        .basic_auth(&state.upstream_username, Some(&state.upstream_password));

    for (name, value) in headers.iter() {
        if strip_request_header(name) {
            continue;
        }
        if state.config.project_directory.is_some() && name.as_str() == DIRECTORY_HEADER {
            continue;
        }
        outgoing = outgoing.header(name, value);
    }

    if let Some(directory) = &state.config.project_directory {
        outgoing = outgoing.header(DIRECTORY_HEADER, directory);
    }

    outgoing
}

/// Build a client response from an upstream status, the upstream headers that
/// survive hop-by-hop stripping, and a body.
fn build_response(status: StatusCode, headers: &HeaderMap, body: Body) -> Response {
    let mut builder = Response::builder().status(status);
    for (name, value) in headers.iter() {
        if strip_response_header(name) {
            continue;
        }
        builder = builder.header(name, value);
    }
    builder
        .body(body)
        .unwrap_or_else(|_| Response::new(Body::empty()))
}

/// Keep only the `GET /project` entry whose `worktree` equals `pin`. Returns
/// `None` when the body is not a JSON array of objects, so the caller passes the
/// upstream response through unchanged instead of inventing one.
fn filter_projects_json(bytes: &[u8], pin: &str) -> Option<serde_json::Value> {
    let value: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    let projects = value.as_array()?;
    if !projects.iter().all(serde_json::Value::is_object) {
        return None;
    }
    let kept: Vec<serde_json::Value> = projects
        .iter()
        .filter(|project| project.get("worktree").and_then(|w| w.as_str()) == Some(pin))
        .cloned()
        .collect();
    Some(serde_json::Value::Array(kept))
}

/// The response sent when the upstream call itself fails.
fn upstream_error(err: reqwest::Error) -> Response {
    (
        StatusCode::BAD_GATEWAY,
        format!("upstream request failed: {err}"),
    )
        .into_response()
}

/// Headers the proxy never forwards from the client to upstream.
fn strip_request_header(name: &HeaderName) -> bool {
    matches!(
        name.as_str(),
        // Inbound credentials are replaced by the injected upstream ones.
        "authorization"
            // Let upstream choose identity encoding; safer for SSE.
            | "accept-encoding"
            // reqwest recomputes these from the body and URL.
            | "content-length"
            | "host"
    ) || is_hop_by_hop(name)
}

/// Headers the proxy never forwards from upstream to the client.
fn strip_response_header(name: &HeaderName) -> bool {
    matches!(name.as_str(), "content-length" | "transfer-encoding") || is_hop_by_hop(name)
}

/// Hop-by-hop headers (RFC 9110 §7.6.1) plus the `proxy-*` family.
fn is_hop_by_hop(name: &HeaderName) -> bool {
    let name = name.as_str();
    matches!(
        name,
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
    ) || name.starts_with("proxy-")
}

/// Emit one audit line per request. `user`/`role` are `-` when unknown.
fn audit(user: &str, role: &str, method: &Method, path: &str, status: StatusCode, outcome: &str) {
    eprintln!(
        "user={user} role={role} method={method} path={path} status={} {outcome}",
        status.as_u16()
    );
}

fn unauthorized() -> Response {
    let mut response = (StatusCode::UNAUTHORIZED, "unauthorized").into_response();
    response.headers_mut().insert(
        header::WWW_AUTHENTICATE,
        HeaderValue::from_static("Basic realm=\"orquestador-proxy\""),
    );
    response
}

fn forbidden() -> Response {
    (StatusCode::FORBIDDEN, "forbidden: viewer role is read-only").into_response()
}

#[cfg(test)]
mod tests {
    use super::filter_projects_json;

    #[test]
    fn keeps_only_the_pinned_project() {
        let body = br#"[
            { "id": "alpha", "worktree": "/home/dev/alpha" },
            { "id": "beta", "worktree": "/home/dev/beta" }
        ]"#;
        let filtered = filter_projects_json(body, "/home/dev/beta").expect("array of objects");
        assert_eq!(
            filtered,
            serde_json::json!([{ "id": "beta", "worktree": "/home/dev/beta" }])
        );
    }

    #[test]
    fn empty_array_stays_an_empty_array() {
        let filtered = filter_projects_json(b"[]", "/home/dev/beta").expect("array of objects");
        assert_eq!(filtered, serde_json::json!([]));
    }

    #[test]
    fn non_array_body_is_not_filtered() {
        assert_eq!(
            filter_projects_json(br#"{ "worktree": "/home/dev/beta" }"#, "/home/dev/beta"),
            None
        );
        assert_eq!(filter_projects_json(b"not json", "/home/dev/beta"), None);
    }

    #[test]
    fn array_with_non_object_entry_is_not_filtered() {
        assert_eq!(
            filter_projects_json(br#"[{ "worktree": "/x" }, "oops"]"#, "/x"),
            None
        );
    }
}
