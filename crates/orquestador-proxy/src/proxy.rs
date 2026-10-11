//! The axum server: authentication, role gate, upstream forwarding, audit.
//!
//! Every request flows through the same fallback handler:
//! 1. authenticate the inbound Basic credentials against `proxy.json`;
//! 2. authorize the method against the user's role (viewer = safe methods only);
//! 3. forward to `opencode serve` with the injected upstream credentials and
//!    stream the response back (SSE included).
//!
//! The proxy is the trust boundary: client credentials never leave it, and an
//! unauthenticated request never reaches upstream.

use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Request, State},
    http::{header, HeaderName, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
    Router,
};
use reqwest::Client;

use crate::{
    auth, authorize,
    config::{Config, Role},
};

/// Upper bound on buffered request bodies. OpenCode writes are small JSON
/// documents; this only bounds the memory a single write can pin.
const MAX_REQUEST_BODY: usize = 16 * 1024 * 1024;

/// Shared server state: the parsed config plus the injected upstream credentials.
pub struct AppState {
    pub config: Config,
    pub upstream_base: String,
    pub upstream_username: String,
    pub upstream_password: String,
    pub client: Client,
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
        }
    }
}

/// Build the proxy router. A single `fallback` handles every method and path,
/// which keeps SSE `/event` and any future upstream path on one code path.
pub fn router(state: Arc<AppState>) -> Router {
    Router::new().fallback(handle).with_state(state)
}

/// Handle one proxied request.
pub async fn handle(State(state): State<Arc<AppState>>, req: Request) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();

    // 1. Authenticate. Missing / malformed / unknown / wrong-token -> 401.
    let Some((name, role)) = authenticate(&state, &req) else {
        audit("-", "-", &method, &path, StatusCode::UNAUTHORIZED, "unauthenticated");
        return unauthorized();
    };

    // 2. Role gate. A viewer write is denied before any upstream contact.
    if !authorize::authorize(method.as_str(), role) {
        audit(&name, role.as_str(), &method, &path, StatusCode::FORBIDDEN, "denied");
        return forbidden();
    }

    // 3. Forward and stream the response back.
    match forward(&state, req).await {
        Ok(response) => {
            audit(&name, role.as_str(), &method, &path, response.status(), "allowed");
            response
        }
        Err(err) => {
            audit(&name, role.as_str(), &method, &path, StatusCode::BAD_GATEWAY, "allowed");
            (
                StatusCode::BAD_GATEWAY,
                format!("upstream request failed: {err}"),
            )
                .into_response()
        }
    }
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
    (
        StatusCode::FORBIDDEN,
        "forbidden: viewer role is read-only",
    )
        .into_response()
}
