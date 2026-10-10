//! Desktop notifications derived from [`orquestador_core::state::Effect`].
//!
//! Decision (see `docs/PLAN.md`): notifications are produced in Rust with
//! `notify-rust` instead of the Tauri notification plugin, so the same
//! `Effect` values that the core already computes can be shown without the
//! webview. They are suppressed while the main window is focused, **except**
//! approval requests, which always notify.

use notify_rust::Notification;
use orquestador_core::state::Effect;
use tauri::{AppHandle, Manager};

/// Show a desktop notification for an `Effect` unless the window is focused.
pub fn handle_effect(app: &AppHandle, effect: &Effect) {
    let focused = app
        .get_webview_window("main")
        .and_then(|window| window.is_focused().ok())
        .unwrap_or(false);

    let (summary, body, always) = match effect {
        Effect::Finished { title, .. } => (
            "Sub-agent finished".to_string(),
            format!("{title} finished its run."),
            false,
        ),
        Effect::Failed { title, message, .. } => (
            "Sub-agent failed".to_string(),
            format!("{title}: {message}"),
            false,
        ),
        Effect::Approval { title, .. } => (
            "Approval required".to_string(),
            format!("{title} is waiting for your approval."),
            true,
        ),
    };

    if focused && !always {
        return;
    }

    // A missing notification daemon (no D-Bus) must never break the app.
    if let Err(error) = Notification::new().summary(&summary).body(&body).show() {
        eprintln!("[orquestador] desktop notification failed: {error}");
    }
}
