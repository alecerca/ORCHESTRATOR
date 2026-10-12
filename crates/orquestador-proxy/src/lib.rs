//! orquestador-proxy
//!
//! A standalone, role-enforcing reverse proxy that sits between clients and a
//! shared `opencode serve` instance. It authenticates each client with HTTP
//! Basic (reusing the client's existing `username` + `password` fields), maps
//! the username to a role, and injects the real upstream credentials so the
//! upstream password is never handed to clients.
//!
//! * [`config`]  — `proxy.json` shape, parsing, validation, user lookup.
//! * [`auth`]    — Basic-auth parsing, constant-time token comparison, Argon2id
//!   password verification.
//! * [`authorize`] — method-based role gate (`viewer` read-only, `editor` full).
//! * [`adduser`] — pure core for the `adduser` CLI: hash a password and append
//!   a user to the config document.
//! * [`proxy`]   — the axum server: auth, role gate, upstream forwarding (SSE
//!   passthrough), audit logging, and the router/app-state wiring.
//!
//! See `README.md` for run instructions and the security model (v0 assumes a
//! private network / tunnel; it does not terminate TLS).

pub mod adduser;
pub mod auth;
pub mod authorize;
pub mod config;
pub mod proxy;
