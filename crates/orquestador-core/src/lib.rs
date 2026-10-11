//! orquestador-core
//!
//! Headless core of the Orquestador desktop app. It is deliberately free of
//! any `tauri` / `wry` / webkit dependency so that the whole business logic can
//! be compiled and unit-tested on machines without the GTK webview stack.
//!
//! Module layout (see `docs/PLAN.md` for the full contract):
//!
//! * [`types`]  — serde shapes extracted from the OpenCode server API (`/doc`).
//! * [`error`]  — single `CoreError` error type (`thiserror`).
//! * [`client`] — typed async HTTP client for the OpenCode server (reqwest + rustls).
//! * [`events`] — SSE subscription, typed event models, reconnect/backoff.
//! * [`state`]  — session tree, per-agent status derivation, permission registry.
//! * [`skills`] — app-owned markdown skills: name slugging + prompt preamble.

pub mod backend;
pub mod client;
pub mod error;
pub mod events;
pub mod skills;
pub mod state;
pub mod types;

pub use backend::{AgentBackend, BackendEvent, EventStream, BACKEND_OPENCODE};
pub use client::OpenCodeClient;
pub use error::CoreError;
pub use events::EventHub;
pub use skills::{build_preamble, slugify, SkillInfo};
pub use state::AppState;

/// Default base URL of a local `opencode serve`.
pub const DEFAULT_BASE_URL: &str = "http://127.0.0.1:4096";
