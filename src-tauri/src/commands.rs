//! Tauri commands: the complete frontend ↔ core surface.
//!
//! Every command is a thin, `Result<_, String>` wrapper around an
//! [`OpenCodeClient`](orquestador_core::client::OpenCodeClient) call or a read
//! of the derived [`AppState`](orquestador_core::AppState). Errors are mapped to
//! their `Display` string, which the webview can surface verbatim.

use orquestador_core::state::AppStateSnapshot;
use orquestador_core::types::{
    Agent, CommandBody, CreateSessionBody, FileContent, FileDiff, FileNode, FileStatus, Health,
    MessageBundle, PermissionResponse, PromptBody, Session, SessionStatus, Todo,
};
use orquestador_core::{build_preamble, slugify, SkillInfo};
use std::collections::HashMap;
use tauri::{Manager, State};

use crate::bridge::AppBridge;

fn stringify<E: std::fmt::Display>(error: E) -> String {
    error.to_string()
}

// -- derived state -----------------------------------------------------------

#[tauri::command]
pub async fn get_state(bridge: State<'_, AppBridge>) -> Result<AppStateSnapshot, String> {
    Ok(bridge.snapshot().await)
}

// -- health / discovery ------------------------------------------------------

#[tauri::command]
pub async fn get_health(bridge: State<'_, AppBridge>) -> Result<Health, String> {
    bridge.client().health().await.map_err(stringify)
}

#[tauri::command]
pub async fn get_agents(bridge: State<'_, AppBridge>) -> Result<Vec<Agent>, String> {
    bridge.client().agents().await.map_err(stringify)
}

#[tauri::command]
pub async fn session_statuses(
    bridge: State<'_, AppBridge>,
) -> Result<HashMap<String, SessionStatus>, String> {
    bridge.client().session_statuses().await.map_err(stringify)
}

// -- sessions ----------------------------------------------------------------

#[tauri::command]
pub async fn list_sessions(bridge: State<'_, AppBridge>) -> Result<Vec<Session>, String> {
    bridge.client().list_sessions().await.map_err(stringify)
}

#[tauri::command]
pub async fn create_session(
    bridge: State<'_, AppBridge>,
    parent_id: Option<String>,
    title: Option<String>,
) -> Result<Session, String> {
    let body = CreateSessionBody { parent_id, title };
    bridge.client().create_session(&body).await.map_err(stringify)
}

#[tauri::command]
pub async fn list_children(
    bridge: State<'_, AppBridge>,
    session_id: String,
) -> Result<Vec<Session>, String> {
    bridge
        .client()
        .session_children(&session_id)
        .await
        .map_err(stringify)
}

#[tauri::command]
pub async fn delete_session(
    bridge: State<'_, AppBridge>,
    session_id: String,
) -> Result<bool, String> {
    bridge
        .client()
        .delete_session(&session_id)
        .await
        .map_err(stringify)
}

#[tauri::command]
pub async fn rename_session(
    bridge: State<'_, AppBridge>,
    session_id: String,
    title: String,
) -> Result<Session, String> {
    bridge
        .client()
        .rename_session(&session_id, &title)
        .await
        .map_err(stringify)
}

#[tauri::command]
pub async fn get_todos(
    bridge: State<'_, AppBridge>,
    session_id: String,
) -> Result<Vec<Todo>, String> {
    bridge.client().todos(&session_id).await.map_err(stringify)
}

// -- instructions / messages -------------------------------------------------

#[tauri::command]
pub async fn send_message(
    app: tauri::AppHandle,
    bridge: State<'_, AppBridge>,
    session_id: String,
    text: String,
    agent: Option<String>,
    skills: Option<Vec<String>>,
) -> Result<MessageBundle, String> {
    let mut body = PromptBody::text(text);
    body.agent = agent;
    apply_skills(&app, &mut body, skills);
    bridge
        .client()
        .prompt(&session_id, &body)
        .await
        .map_err(stringify)
}

#[tauri::command]
pub async fn send_message_async(
    app: tauri::AppHandle,
    bridge: State<'_, AppBridge>,
    session_id: String,
    text: String,
    agent: Option<String>,
    skills: Option<Vec<String>>,
) -> Result<(), String> {
    let mut body = PromptBody::text(text);
    body.agent = agent;
    apply_skills(&app, &mut body, skills);
    bridge
        .client()
        .prompt_async(&session_id, &body)
        .await
        .map_err(stringify)
}

#[tauri::command]
pub async fn abort_session(
    bridge: State<'_, AppBridge>,
    session_id: String,
) -> Result<bool, String> {
    bridge.client().abort(&session_id).await.map_err(stringify)
}

#[tauri::command]
pub async fn get_messages(
    bridge: State<'_, AppBridge>,
    session_id: String,
    limit: Option<u32>,
) -> Result<Vec<MessageBundle>, String> {
    bridge
        .client()
        .messages(&session_id, limit)
        .await
        .map_err(stringify)
}

#[tauri::command]
pub async fn get_message(
    bridge: State<'_, AppBridge>,
    session_id: String,
    message_id: String,
) -> Result<MessageBundle, String> {
    bridge
        .client()
        .message(&session_id, &message_id)
        .await
        .map_err(stringify)
}

#[tauri::command]
pub async fn send_command(
    bridge: State<'_, AppBridge>,
    session_id: String,
    command: String,
    arguments: Option<String>,
) -> Result<MessageBundle, String> {
    let body = CommandBody {
        command,
        arguments,
    };
    bridge
        .client()
        .command(&session_id, &body)
        .await
        .map_err(stringify)
}

// -- approvals ---------------------------------------------------------------

#[tauri::command]
pub async fn respond_permission(
    bridge: State<'_, AppBridge>,
    session_id: String,
    permission_id: String,
    response: String,
    remember: Option<bool>,
) -> Result<bool, String> {
    let response = match response.as_str() {
        "once" => PermissionResponse::Once,
        "always" => PermissionResponse::Always,
        "reject" => PermissionResponse::Reject,
        other => return Err(format!("unknown permission response: {other}")),
    };
    let result = bridge
        .client()
        .respond_permission(&session_id, &permission_id, response, remember)
        .await;

    match result {
        Ok(answered) => {
            // Drop the approval locally and push the snapshot instead of waiting
            // for `permission.replied` (which can be missed) so the dialog closes
            // deterministically.
            bridge
                .state()
                .write()
                .await
                .remove_permission(&permission_id);
            bridge.push_snapshot().await;
            Ok(answered)
        }
        Err(err) if err.is_not_found() => {
            // The permission is gone server-side (answered elsewhere, withdrawn
            // or lost to a dead stream). Drop the phantom locally and treat the
            // reply as handled rather than surfacing the raw `404`.
            bridge
                .state()
                .write()
                .await
                .remove_permission(&permission_id);
            bridge.push_snapshot().await;
            Ok(false)
        }
        Err(err) => Err(stringify(err)),
    }
}

// -- diffs / files -----------------------------------------------------------

#[tauri::command]
pub async fn get_diff(
    bridge: State<'_, AppBridge>,
    session_id: String,
    message_id: Option<String>,
) -> Result<Vec<FileDiff>, String> {
    bridge
        .client()
        .diff(&session_id, message_id.as_deref())
        .await
        .map_err(stringify)
}

#[tauri::command]
pub async fn list_files(
    bridge: State<'_, AppBridge>,
    path: Option<String>,
) -> Result<Vec<FileNode>, String> {
    bridge
        .client()
        .file_tree(path.as_deref())
        .await
        .map_err(stringify)
}

#[tauri::command]
pub async fn read_file(
    bridge: State<'_, AppBridge>,
    path: String,
) -> Result<FileContent, String> {
    bridge.client().file_content(&path).await.map_err(stringify)
}

#[tauri::command]
pub async fn file_status(bridge: State<'_, AppBridge>) -> Result<Vec<FileStatus>, String> {
    bridge.client().file_status().await.map_err(stringify)
}

#[tauri::command]
pub async fn find_files(
    bridge: State<'_, AppBridge>,
    query: String,
    limit: Option<u32>,
) -> Result<Vec<String>, String> {
    bridge
        .client()
        .find_files(&query, limit)
        .await
        .map_err(stringify)
}

#[tauri::command]
pub async fn find_symbols(
    bridge: State<'_, AppBridge>,
    query: String,
) -> Result<Vec<serde_json::Value>, String> {
    bridge
        .client()
        .find_symbols(&query)
        .await
        .map_err(stringify)
}

// -- skills ------------------------------------------------------------------
//
// Orquestador owns a small store of user-authored markdown *skills* kept at
// `<app_config_dir>/skills/<slug>.md`. The file stem is the skill's slug, so a
// user-facing name is slugged consistently on every read/write/delete. The
// commands below are the only local-disk IO in the shell (the `list_files` /
// `read_file` pair above proxies the opencode server instead).
//
// When the user has skills switched on, `apply_skills` folds them into the
// outbound prompt's hidden `system` field. opencode appends that field after the
// agent prompt, so the skills reach the model on every message without ever
// showing up in the user's transcript.

/// Resolve the app-owned skills directory (`<app_config_dir>/skills`), creating
/// it on first use so callers can read/write without a separate mkdir.
fn skills_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(stringify)?
        .join("skills");
    std::fs::create_dir_all(&dir).map_err(stringify)?;
    Ok(dir)
}

/// Fold the selected skills into `body.system`.
///
/// Each name is slugged and its `<dir>/<slug>.md` read; empty slugs and
/// unreadable files are skipped so a bad or missing skill can never fail a send.
/// The rendered `<skills>` preamble is appended to any existing system prompt.
fn apply_skills(app: &tauri::AppHandle, body: &mut PromptBody, skills: Option<Vec<String>>) {
    let Some(names) = skills else { return };
    if names.is_empty() {
        return;
    }
    let Ok(dir) = skills_dir(app) else { return };

    let loaded: Vec<(String, String)> = names
        .iter()
        .filter_map(|name| {
            let slug = slugify(name);
            if slug.is_empty() {
                return None;
            }
            let content = std::fs::read_to_string(dir.join(format!("{slug}.md"))).ok()?;
            Some((slug, content))
        })
        .collect();

    let preamble = build_preamble(&loaded);
    if preamble.is_empty() {
        return;
    }

    let existing = body.system.take().unwrap_or_default();
    body.system = Some(if existing.is_empty() {
        preamble
    } else {
        format!("{existing}\n\n{preamble}")
    });
}

/// List every stored skill (`.md` files in the skills dir), sorted by name.
#[tauri::command]
pub async fn list_skills(app: tauri::AppHandle) -> Result<Vec<SkillInfo>, String> {
    let dir = skills_dir(&app)?;
    let entries = std::fs::read_dir(&dir).map_err(stringify)?;
    let mut skills = Vec::new();
    for entry in entries {
        let path = entry.map_err(stringify)?.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("md") || !path.is_file() {
            continue;
        }
        let Some(name) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        skills.push(SkillInfo {
            name: name.to_string(),
            path: path.to_string_lossy().into_owned(),
        });
    }
    skills.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(skills)
}

/// Read a skill's markdown by user-facing name (slugged to its file stem).
#[tauri::command]
pub async fn read_skill(app: tauri::AppHandle, name: String) -> Result<String, String> {
    let slug = slugify(&name);
    if slug.is_empty() {
        return Err(format!("invalid skill name: {name:?}"));
    }
    let path = skills_dir(&app)?.join(format!("{slug}.md"));
    std::fs::read_to_string(&path).map_err(stringify)
}

/// Write a skill's markdown, creating or overwriting `<dir>/<slug>.md`.
#[tauri::command]
pub async fn save_skill(
    app: tauri::AppHandle,
    name: String,
    content: String,
) -> Result<SkillInfo, String> {
    let slug = slugify(&name);
    if slug.is_empty() {
        return Err(format!("invalid skill name: {name:?}"));
    }
    let path = skills_dir(&app)?.join(format!("{slug}.md"));
    std::fs::write(&path, content).map_err(stringify)?;
    Ok(SkillInfo {
        name: slug,
        path: path.to_string_lossy().into_owned(),
    })
}

/// Delete a skill's markdown. A skill that does not exist reports `Ok(false)`.
#[tauri::command]
pub async fn delete_skill(app: tauri::AppHandle, name: String) -> Result<bool, String> {
    let slug = slugify(&name);
    if slug.is_empty() {
        return Err(format!("invalid skill name: {name:?}"));
    }
    let path = skills_dir(&app)?.join(format!("{slug}.md"));
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(true),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(stringify(err)),
    }
}
