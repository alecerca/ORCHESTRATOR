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
use std::collections::HashMap;
use tauri::State;

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
    bridge: State<'_, AppBridge>,
    session_id: String,
    text: String,
    agent: Option<String>,
) -> Result<MessageBundle, String> {
    let mut body = PromptBody::text(text);
    body.agent = agent;
    bridge
        .client()
        .prompt(&session_id, &body)
        .await
        .map_err(stringify)
}

#[tauri::command]
pub async fn send_message_async(
    bridge: State<'_, AppBridge>,
    session_id: String,
    text: String,
    agent: Option<String>,
) -> Result<(), String> {
    let mut body = PromptBody::text(text);
    body.agent = agent;
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
    bridge
        .client()
        .respond_permission(&session_id, &permission_id, response, remember)
        .await
        .map_err(stringify)
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
