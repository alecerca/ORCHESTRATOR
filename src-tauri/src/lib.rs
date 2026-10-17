//! Orquestador desktop shell — Tauri glue only.
//!
//! The whole application logic lives in the `orquestador-core` crate; this
//! crate wires it to a window, exposes the `#[tauri::command]`s and forwards
//! core messages to the webview. It requires `webkit2gtk-4.1` to compile (see
//! `scripts/setup-system-deps.sh` and `docs/PLAN.md`).

mod bridge;
mod commands;
mod notify;
mod proxy_config;
mod server_config;

use bridge::AppBridge;
use orquestador_core::events::{BootstrapOutcome, CoreMessage};
use tauri::{Emitter, Manager};
use tokio::sync::broadcast;

/// Tauri event name used to forward every [`CoreMessage`] to the webview.
/// The frontend listens with `listen("core://message", ...)`.
pub const CORE_EVENT: &str = "core://message";

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Prefer the persisted server settings (written by the Settings
            // dialog); otherwise fall back to the environment / defaults.
            let config = app
                .path()
                .app_config_dir()
                .ok()
                .and_then(|dir| server_config::load_from_dir(&dir))
                .unwrap_or_else(orquestador_core::client::ClientConfig::from_env_or_default);
            let bridge = AppBridge::new_with_config(config);
            // Subscribe before the hub starts so no message is lost.
            let mut receiver = bridge.subscribe();

            // Forward core messages to the webview and turn effects into
            // desktop notifications.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    match receiver.recv().await {
                        Ok(message) => {
                            if let CoreMessage::Effect { effect } = &message {
                                notify::debug_log(&format!("forwarder: received {effect:?}"));
                                notify::handle_effect(effect);
                            }
                            let _ = handle.emit(CORE_EVENT, &message);
                        }
                        Err(broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
            });

            // Initial synchronous load, then the long-lived SSE hub.
            match tauri::async_runtime::block_on(bridge.bootstrap()) {
                BootstrapOutcome::Connected { root_session_id, .. } => {
                    eprintln!("[orquestador] connected (root session: {root_session_id:?})");
                }
                BootstrapOutcome::Disconnected { error } => {
                    eprintln!("[orquestador] initial connect failed: {error}");
                }
            }
            bridge.spawn_hub();

            // TEMP (notification investigation): fire one notification a few
            // seconds after startup so we can observe whether the app's own
            // notification path shows a banner. Remove after investigation.
            std::thread::spawn(|| {
                std::thread::sleep(std::time::Duration::from_secs(6));
                notify::handle_effect(&orquestador_core::state::Effect::Approval {
                    session_id: "startup-test".to_string(),
                    permission_id: "startup".to_string(),
                    title: "STARTUP TEST".to_string(),
                });
            });

            app.manage(bridge);
            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                if let Some(bridge) = window.app_handle().try_state::<AppBridge>() {
                    bridge.request_shutdown();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::get_health,
            commands::test_connection,
            commands::get_server_config,
            commands::set_server_config,
            commands::list_projects,
            commands::current_project,
            commands::set_project_directory,
            commands::get_agents,
            commands::get_providers,
            commands::get_provider_auth,
            commands::set_provider_api_key,
            commands::get_presence,
            commands::get_workspace_scope,
            commands::session_statuses,
            commands::list_sessions,
            commands::create_session,
            commands::list_children,
            commands::delete_session,
            commands::rename_session,
            commands::set_session_model,
            commands::get_todos,
            commands::send_message,
            commands::send_message_async,
            commands::send_command,
            commands::abort_session,
            commands::get_messages,
            commands::get_message,
            commands::respond_permission,
            commands::list_files,
            commands::read_file,
            commands::file_status,
            commands::find_files,
            commands::find_symbols,
            commands::list_skills,
            commands::read_skill,
            commands::save_skill,
            commands::delete_skill,
            commands::git_branch,
            commands::git_log,
            commands::git_commit_files,
            commands::git_branches,
            commands::git_checkout,
            commands::git_status,
            commands::git_show_file,
            commands::git_discard_file,
            commands::write_file,
            commands::get_proxy_members,
            commands::add_proxy_member,
            commands::remove_proxy_member,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the Orquestador shell");
}
