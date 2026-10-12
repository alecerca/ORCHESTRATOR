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
    http::{header, HeaderName, HeaderValue, Method, StatusCode},
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

    // 5. Forward and stream the response back.
    match forward(&state, req).await {
        Ok(response) => {
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
        Err(err) => {
            audit(
                &name,
                role.as_str(),
                &method,
                &path,
                StatusCode::BAD_GATEWAY,
                "allowed",
            );
            (
                StatusCode::BAD_GATEWAY,
                format!("upstream request failed: {err}"),
            )
                .into_response()
        }
    }
}

/// `GET /orquestador/whoami` — the caller's own identity, doubling as a clean
/// credential check for the frontend. Read-only; presence is recorded by the
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
        Json(json!({ "name": name, "role": role.as_str() })),
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
    let method = parts.method;
    let uri = parts.uri;

    let mut url = format!("{}{}", state.upstream_base, uri.path());
    if let Some(query) = uri.query() {
        url.push('?');
        url.push_str(query);
    }

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

    let mut outgoing = state
        .client
        .request(method, &url)
        .basic_auth(&state.upstream_username, Some(&state.upstream_password));
    for (name, value) in parts.headers.iter() {
        if strip_request_header(name) {
            continue;
        }
        outgoing = outgoing.header(name, value);
    }

    let upstream = outgoing.body(body_bytes).send().await?;

    let status = upstream.status();
    let mut builder = Response::builder().status(status);
    for (name, value) in upstream.headers().iter() {
        if strip_response_header(name) {
            continue;
        }
        builder = builder.header(name, value);
    }

    // `Body::from_stream` drives reqwest's byte stream directly, so SSE frames
    // are relayed as they arrive instead of being buffered.
    let stream = upstream.bytes_stream();
    Ok(builder
        .body(Body::from_stream(stream))
        .unwrap_or_else(|_| Response::new(Body::empty())))
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
