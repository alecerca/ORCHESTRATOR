//! Desktop notifications derived from [`orquestador_core::state::Effect`].
//!
//! Decision (see `docs/PLAN.md`): notifications are produced in Rust with
//! `notify-rust` instead of the Tauri notification plugin, so the same
//! `Effect` values that the core already computes can be shown without the
//! webview. Every effect notifies, whether or not the main window is focused.

use std::io::Write;

use notify_rust::Notification;
use orquestador_core::state::Effect;

/// TEMPORARY diagnostic (notification investigation). Remove once the path is
/// confirmed. Appends one line per event to a file the agent can read.
pub(crate) fn debug_log(line: &str) {
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("/tmp/orquestador-notify.log")
    {
        let _ = writeln!(file, "{line}");
    }
}

fn effect_kind(effect: &Effect) -> &'static str {
    match effect {
        Effect::Finished { .. } => "finished",
        Effect::Failed { .. } => "failed",
        Effect::Approval { .. } => "approval",
        Effect::UsageLimit { .. } => "usage_limit",
    }
}

/// Show a desktop notification for an `Effect`.
pub fn handle_effect(effect: &Effect) {
    let (summary, body) = match effect {
        Effect::Finished { title, .. } => (
            "Sub-agent finished".to_string(),
            format!("{title} finished its run."),
        ),
        Effect::Failed { title, message, .. } => (
            "Sub-agent failed".to_string(),
            format!("{title}: {message}"),
        ),
        Effect::Approval { title, .. } => (
            "Approval required".to_string(),
            format!("{title} is waiting for your approval."),
        ),
        Effect::UsageLimit { title, message, .. } => (
            "Usage limit reached".to_string(),
            if title.trim().is_empty() {
                format!("Waiting before retrying: {message}")
            } else {
                format!("{title}: waiting before retrying ({message}).")
            },
        ),
    };

    debug_log(&format!(
        "handle_effect kind={} summary={summary:?}",
        effect_kind(effect)
    ));

    // A missing notification daemon (no D-Bus) must never break the app.
    match Notification::new()
        .appname("Orquestador")
        .summary(&summary)
        .body(&body)
        .show()
    {
        Ok(_) => debug_log("  -> shown (daemon accepted)"),
        Err(error) => {
            debug_log(&format!("  -> show FAILED: {error}"));
            eprintln!("[orquestador] desktop notification failed: {error}");
        }
    }
}
