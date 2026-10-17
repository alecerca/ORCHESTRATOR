//! Tauri commands: the complete frontend ↔ core surface.
//!
//! Every command is a thin, `Result<_, String>` wrapper around an
//! [`OpenCodeClient`](orquestador_core::client::OpenCodeClient) call or a read
//! of the derived [`AppState`](orquestador_core::AppState). Errors are mapped to
//! their `Display` string, which the webview can surface verbatim.

use orquestador_core::state::AppStateSnapshot;
use orquestador_core::state::Effect;
use orquestador_core::client::ClientConfig;
use orquestador_core::types::{
    Agent, CommandBody, CreateSessionBody, FileContent, FileNode, FileStatus, Health,
    MessageBundle, PermissionResponse, PresenceEntry, Project, PromptBody, ProviderAuthMethod,
    ProviderList, Session, SessionStatus, Todo,
};
use orquestador_core::skills::merge_skills;
use orquestador_core::{build_preamble, slugify, CoreError, SkillInfo};
use std::collections::HashMap;
use tauri::{Manager, State};

use crate::bridge::AppBridge;

fn stringify<E: std::fmt::Display>(error: E) -> String {
    error.to_string()
}

/// When a send fails because of a usage/rate limit, publish a distinct
/// [`Effect::UsageLimit`] so the webview shows a prominent warning and the
/// desktop notification fires — not just a terse command error. Non-limit
/// errors are left to the command error string (already shown by the caller).
async fn report_send_error(bridge: &AppBridge, session_id: &str, error: &CoreError) {
    if !error.is_usage_limit() {
        return;
    }
    let title = bridge
        .state()
        .read()
        .await
        .session(session_id)
        .map(|session| session.title.clone())
        .unwrap_or_default();
    bridge.push_effect(Effect::UsageLimit {
        session_id: session_id.to_string(),
        title,
        message: error.to_string(),
    });
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

/// Probe an arbitrary server configuration without touching the running
/// connection. Returns the health payload so the frontend can show the version.
#[tauri::command]
pub async fn test_connection(
    base_url: String,
    username: Option<String>,
    password: Option<String>,
) -> Result<Health, String> {
    let config = crate::server_config::to_client_config(&crate::server_config::ServerConfigFile {
        base_url: Some(base_url),
        username,
        password,
        directory: None,
        member_name: None,
        proxy_config_path: None,
    });
    // Probe fast; a wrong host should fail quickly instead of hanging 30s.
    let config = ClientConfig {
        request_timeout: std::time::Duration::from_secs(5),
        ..config
    };
    let client = orquestador_core::OpenCodeClient::new(config).map_err(stringify)?;
    client.health().await.map_err(stringify)
}

/// Server settings as exposed to the webview.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerConfigDto {
    pub base_url: String,
    pub username: String,
    pub password: Option<String>,
    /// Active project directory, or `None` for the server's own cwd project.
    pub directory: Option<String>,
    /// Optional display name used to attribute sessions this app creates.
    pub member_name: Option<String>,
    /// Optional path to the host's whitelist `proxy.json` edited by the Members
    /// dialog. UI-only; not a connection field.
    pub proxy_config_path: Option<String>,
}

impl ServerConfigDto {
    /// Build the DTO from the connection config plus the UI-only proxy path,
    /// which lives on [`ServerConfigFile`](crate::server_config::ServerConfigFile)
    /// rather than [`ClientConfig`].
    fn from_client_config(cfg: &ClientConfig, proxy_config_path: Option<String>) -> Self {
        Self {
            base_url: cfg.base_url.clone(),
            username: cfg.username.clone(),
            password: cfg.password.clone(),
            directory: cfg.directory.clone(),
            member_name: cfg.member_name.clone(),
            proxy_config_path,
        }
    }
}

/// Current effective server settings (persisted file when present, else the
/// environment / built-in defaults).
#[tauri::command]
pub async fn get_server_config(app: tauri::AppHandle) -> Result<ServerConfigDto, String> {
    let dir = app.path().app_config_dir().map_err(stringify)?;
    let cfg = crate::server_config::load_from_dir(&dir)
        .unwrap_or_else(ClientConfig::from_env_or_default);
    // The proxy config path is stored on the raw file (it is not a connection
    // field), so read it back separately from the collapsed client config.
    let proxy_config_path = crate::server_config::load_file(&dir)
        .and_then(|file| crate::server_config::normalize_proxy_config_path(file.proxy_config_path));
    Ok(ServerConfigDto::from_client_config(&cfg, proxy_config_path))
}

/// Persist new server settings and reconnect the live client to them.
///
/// The active project directory is preserved: this command never touches the
/// scoping choice, so changing the base URL does not silently reset the project.
#[tauri::command]
pub async fn set_server_config(
    app: tauri::AppHandle,
    bridge: State<'_, AppBridge>,
    base_url: String,
    username: Option<String>,
    password: Option<String>,
    member_name: Option<String>,
    proxy_config_path: Option<String>,
) -> Result<ServerConfigDto, String> {
    let dir = app.path().app_config_dir().map_err(stringify)?;
    let mut file = crate::server_config::effective_file(&dir);
    file.base_url = Some(crate::server_config::normalize_base_url(&base_url));
    file.username = username;
    file.password = password;
    file.member_name = crate::server_config::normalize_member_name(member_name);
    file.proxy_config_path = crate::server_config::normalize_proxy_config_path(proxy_config_path);
    crate::server_config::save_to_dir(&dir, &file).map_err(stringify)?;

    let cfg = crate::server_config::to_client_config(&file);
    let dto = ServerConfigDto::from_client_config(&cfg, file.proxy_config_path.clone());
    bridge.reconnect(cfg).await.map_err(stringify)?;
    Ok(dto)
}

// -- projects ----------------------------------------------------------------

/// Every project the running server knows about (`GET /project`).
#[tauri::command]
pub async fn list_projects(bridge: State<'_, AppBridge>) -> Result<Vec<Project>, String> {
    bridge.client().projects().await.map_err(stringify)
}

/// The project the current client is scoped to (`GET /project/current`).
#[tauri::command]
pub async fn current_project(bridge: State<'_, AppBridge>) -> Result<Project, String> {
    bridge.client().current_project().await.map_err(stringify)
}

/// Scope the app to a project directory and persist the choice.
///
/// `directory` is an absolute path; `None` clears the choice so the server's own
/// cwd project is used. The switch rebuilds the client through the existing
/// reconnect path and re-runs bootstrap, so sessions/files reflect the new
/// project without restarting the server.
#[tauri::command]
pub async fn set_project_directory(
    app: tauri::AppHandle,
    bridge: State<'_, AppBridge>,
    directory: Option<String>,
) -> Result<ServerConfigDto, String> {
    let dir = app.path().app_config_dir().map_err(stringify)?;
    let mut file = crate::server_config::effective_file(&dir);
    file.directory = crate::server_config::normalize_directory(directory);
    crate::server_config::save_to_dir(&dir, &file).map_err(stringify)?;

    let cfg = crate::server_config::to_client_config(&file);
    let dto = ServerConfigDto::from_client_config(&cfg, file.proxy_config_path.clone());
    bridge.reconnect(cfg).await.map_err(stringify)?;
    Ok(dto)
}

#[tauri::command]
pub async fn get_agents(bridge: State<'_, AppBridge>) -> Result<Vec<Agent>, String> {
    bridge.client().agents().await.map_err(stringify)
}

/// Available AI providers, their models, the configured default model per
/// provider, and the ids already authenticated. The typed DTO drops the
/// credential-bearing `key`/`options` fields, so nothing secret reaches the
/// webview.
///
/// Each provider is stamped with `usable` (already authenticated) and `hasAuth`
/// (declares an auth method) so the model picker can mark — and refuse to offer
/// — models from providers that cannot serve a request.
#[tauri::command]
pub async fn get_providers(bridge: State<'_, AppBridge>) -> Result<ProviderList, String> {
    let client = bridge.client();
    let mut list = client.providers().await.map_err(stringify)?;
    // Best-effort: if `GET /provider/auth` fails, degrade to "no auth methods"
    // so only the already-connected providers stay usable.
    let auth = client.provider_auth().await.unwrap_or_default();
    list.classify_usability(auth.keys().map(String::as_str));
    Ok(list)
}

/// Supported auth methods per provider (`GET /provider/auth`), keyed by
/// provider id.
#[tauri::command]
pub async fn get_provider_auth(
    bridge: State<'_, AppBridge>,
) -> Result<HashMap<String, Vec<ProviderAuthMethod>>, String> {
    bridge.client().provider_auth().await.map_err(stringify)
}

/// Online members of the shared workspace. Backends without presence (a direct
/// `opencode` connection) answer with an empty list, never an error.
#[tauri::command]
pub async fn get_presence(bridge: State<'_, AppBridge>) -> Result<Vec<PresenceEntry>, String> {
    bridge.client().presence().await.map_err(stringify)
}

#[tauri::command]
pub async fn get_workspace_scope(bridge: State<'_, AppBridge>) -> Result<Option<String>, String> {
    bridge.client().workspace_scope().await.map_err(stringify)
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

/// Record `model` as the session's effective model
/// (`POST /api/session/:id/model`). The session is the only authoritative
/// source of the model a request runs, so this is what makes a choice sticky —
/// and what lets the picker go back to the server default by re-applying a
/// known baseline instead of sending nothing.
#[tauri::command]
pub async fn set_session_model(
    bridge: State<'_, AppBridge>,
    session_id: String,
    model: orquestador_core::types::ModelRef,
) -> Result<(), String> {
    bridge
        .client()
        .set_session_model(&session_id, &model)
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
    model: Option<orquestador_core::types::ModelRef>,
) -> Result<MessageBundle, String> {
    let mut body = PromptBody::text(text);
    body.agent = agent;
    body.model = model;
    apply_skills(&app, &mut body, skills);
    let result = bridge.client().prompt(&session_id, &body).await;
    if let Err(error) = &result {
        report_send_error(&bridge, &session_id, error).await;
    }
    result.map_err(stringify)
}

#[tauri::command]
pub async fn send_message_async(
    app: tauri::AppHandle,
    bridge: State<'_, AppBridge>,
    session_id: String,
    text: String,
    agent: Option<String>,
    skills: Option<Vec<String>>,
    model: Option<orquestador_core::types::ModelRef>,
) -> Result<(), String> {
    let mut body = PromptBody::text(text);
    body.agent = agent;
    body.model = model;
    apply_skills(&app, &mut body, skills);
    let result = bridge.client().prompt_async(&session_id, &body).await;
    if let Err(error) = &result {
        report_send_error(&bridge, &session_id, error).await;
    }
    result.map_err(stringify)
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

// -- files -------------------------------------------------------------------

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

/// The id of the currently connected agent backend (e.g. `"opencode"`). This is
/// the source of truth for which native skills the panel surfaces; there is no
/// separate config field.
fn backend_id(app: &tauri::AppHandle) -> String {
    app.state::<AppBridge>().client().id().to_string()
}

/// Native skill roots for the active backend, in shadowing order (earlier roots
/// win). An unknown backend has no native roots.
fn native_skill_roots(app: &tauri::AppHandle, backend_id: &str) -> Vec<std::path::PathBuf> {
    let Ok(home) = app.path().home_dir() else {
        return Vec::new();
    };
    match backend_id {
        "opencode" => vec![
            home.join(".config").join("opencode").join("skills"),
            home.join(".agents").join("skills"),
        ],
        "claude" => vec![home.join(".claude").join("skills")],
        "codex" => vec![home.join(".codex").join("skills")],
        _ => Vec::new(),
    }
}

/// Resolve one named skill inside a native `root`: `<root>/<name>/SKILL.md` (the
/// directory layout) or, defensively, `<root>/<name>.md` (the file layout).
/// Returns the readable markdown path or `None` when neither exists.
fn native_skill_path(root: &std::path::Path, name: &str) -> Option<std::path::PathBuf> {
    let nested = root.join(name).join("SKILL.md");
    if nested.is_file() {
        return Some(nested);
    }
    let file = root.join(format!("{name}.md"));
    if file.is_file() {
        return Some(file);
    }
    None
}

/// The app-owned skills: every `.md` file in the app skills dir.
fn app_skill_list(app: &tauri::AppHandle) -> Result<Vec<SkillInfo>, String> {
    let dir = skills_dir(app)?;
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
            source: "app".to_string(),
            editable: true,
        });
    }
    Ok(skills)
}

/// The active backend's native skills, scanned root by root in order. A native
/// skill is a directory containing `SKILL.md` (name = directory name) or,
/// defensively, a `.md` file (name = file stem). Always read-only.
fn native_skill_list(app: &tauri::AppHandle, backend_id: &str) -> Vec<SkillInfo> {
    let mut skills = Vec::new();
    for root in native_skill_roots(app, backend_id) {
        let Ok(entries) = std::fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let markdown = path.join("SKILL.md");
                let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                    continue;
                };
                if !markdown.is_file() {
                    continue;
                }
                skills.push(SkillInfo {
                    name: name.to_string(),
                    path: markdown.to_string_lossy().into_owned(),
                    source: backend_id.to_string(),
                    editable: false,
                });
            } else if path.extension().and_then(|ext| ext.to_str()) == Some("md") && path.is_file() {
                let Some(name) = path.file_stem().and_then(|stem| stem.to_str()) else {
                    continue;
                };
                skills.push(SkillInfo {
                    name: name.to_string(),
                    path: path.to_string_lossy().into_owned(),
                    source: backend_id.to_string(),
                    editable: false,
                });
            }
        }
    }
    skills
}

/// Resolve a selected name to a readable markdown file: the app store first
/// (`<dir>/<slug>.md`), then each native root in order (`<name>/SKILL.md` or
/// `<name>.md`). Returns `None` when nothing readable matches.
fn resolve_skill_path(app: &tauri::AppHandle, name: &str) -> Option<std::path::PathBuf> {
    let slug = slugify(name);
    if !slug.is_empty() {
        if let Ok(dir) = skills_dir(app) {
            let app_path = dir.join(format!("{slug}.md"));
            if app_path.is_file() {
                return Some(app_path);
            }
        }
    }
    let backend = backend_id(app);
    for root in native_skill_roots(app, &backend) {
        if let Some(path) = native_skill_path(&root, name) {
            return Some(path);
        }
        // Defensive: a native root may be keyed by the slugified name.
        if slug.as_str() != name {
            if let Some(path) = native_skill_path(&root, &slug) {
                return Some(path);
            }
        }
    }
    None
}

/// Fold the selected skills into `body.system`.
///
/// Each name resolves app-first, then against the active backend's native
/// roots; empty names and unreadable/missing skills are skipped so a bad or
/// stale selection can never fail a send. The rendered `<skills>` preamble is
/// appended to any existing system prompt.
fn apply_skills(app: &tauri::AppHandle, body: &mut PromptBody, skills: Option<Vec<String>>) {
    let Some(names) = skills else { return };
    if names.is_empty() {
        return;
    }

    let loaded: Vec<(String, String)> = names
        .iter()
        .filter_map(|name| {
            let path = resolve_skill_path(app, name)?;
            let content = std::fs::read_to_string(&path).ok()?;
            let slug = slugify(name);
            let label = if slug.is_empty() {
                name.trim().to_string()
            } else {
                slug
            };
            if label.is_empty() {
                return None;
            }
            Some((label, content))
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

/// List every available skill: the app store plus the connected backend's
/// native skills, merged and deduped by name (app shadows native, earlier
/// native root shadows later) and sorted by name.
#[tauri::command]
pub async fn list_skills(app: tauri::AppHandle) -> Result<Vec<SkillInfo>, String> {
    let backend = backend_id(&app);
    Ok(merge_skills(
        app_skill_list(&app)?,
        native_skill_list(&app, &backend),
    ))
}

/// Read a skill's markdown by name: the app store first, then the active
/// backend's native roots.
#[tauri::command]
pub async fn read_skill(app: tauri::AppHandle, name: String) -> Result<String, String> {
    if name.trim().is_empty() {
        return Err(format!("invalid skill name: {name:?}"));
    }
    match resolve_skill_path(&app, &name) {
        Some(path) => std::fs::read_to_string(&path).map_err(stringify),
        None => Err(format!("skill not found: {name:?}")),
    }
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
        source: "app".to_string(),
        editable: true,
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

// -- proxy whitelist ---------------------------------------------------------
//
// The host console reads and edits the owner's whitelist `proxy.json` at a
// caller-supplied path. The app never runs the proxy: the proxy crate stays the
// trust boundary that enforces roles; these commands only manage its file. The
// module mirrors the proxy's format (Argon2id `password_hash`) without
// depending on it, so no axum/server code enters the desktop build.

/// A whitelist member as exposed to the webview: identity and role only. The
/// `password_hash` and any legacy `token` are deliberately not included.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyMemberDto {
    pub name: String,
    pub role: String,
}

impl From<crate::proxy_config::ProxyMember> for ProxyMemberDto {
    fn from(member: crate::proxy_config::ProxyMember) -> Self {
        Self {
            name: member.name,
            role: member.role,
        }
    }
}

/// Whitelist members from the `proxy.json` at `path`.
#[tauri::command]
pub async fn get_proxy_members(path: String) -> Result<Vec<ProxyMemberDto>, String> {
    crate::proxy_config::list_members(std::path::Path::new(&path))
        .map(|members| members.into_iter().map(ProxyMemberDto::from).collect())
}

/// Add a whitelist member with an Argon2id-hashed password to the config at
/// `path`. Rejects an empty name, an empty password, a duplicate name and an
/// unknown role.
#[tauri::command]
pub async fn add_proxy_member(
    path: String,
    name: String,
    role: String,
    password: String,
) -> Result<(), String> {
    crate::proxy_config::add_member(std::path::Path::new(&path), &name, &role, &password)
}

/// Remove the whitelist member `name` from the config at `path`.
#[tauri::command]
pub async fn remove_proxy_member(path: String, name: String) -> Result<(), String> {
    crate::proxy_config::remove_member(std::path::Path::new(&path), &name)
}

// -- git ---------------------------------------------------------------------
//
// The OpenCode API does not expose the git branch, so the UI derives it locally
// from each session's `directory`. This is read-only introspection: the command
// runs `git rev-parse` (never a mutating command) on a blocking thread so a slow
// or missing `git` cannot stall the async runtime. There is nothing the UI must
// surface as an error — an unknown branch is simply `None`.

/// The current branch of the git repository at `directory`, or `None` when the
/// directory is empty, is not a repository, or git fails.
#[tauri::command]
pub async fn git_branch(directory: String) -> Option<String> {
    if directory.trim().is_empty() {
        return None;
    }
    tauri::async_runtime::spawn_blocking(move || {
        orquestador_core::git::branch_for_dir(std::path::Path::new(&directory))
    })
    .await
    .ok()
    .flatten()
}

/// The active project directory (the persisted `x-opencode-directory` choice),
/// or a clear error when no project has been picked yet. Reads the same
/// `server.json` the Settings dialog writes; no server round-trip.
fn active_directory(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(stringify)?;
    let config = crate::server_config::to_client_config(&crate::server_config::effective_file(&dir));
    config
        .directory
        .map(std::path::PathBuf::from)
        .ok_or_else(|| {
            "No project directory selected — pick one to see its git history.".to_string()
        })
}

/// The current branch and recent commits of the active project directory.
/// Read-only: the core only runs `git log` / `git rev-parse`.
#[tauri::command]
pub async fn git_log(
    app: tauri::AppHandle,
    limit: Option<usize>,
) -> Result<orquestador_core::git::RepoLog, String> {
    let dir = active_directory(&app)?;
    let limit = limit.unwrap_or(100);
    tauri::async_runtime::spawn_blocking(move || orquestador_core::git::overview(&dir, limit))
        .await
        .map_err(stringify)?
}

/// The files changed by one commit of the active project directory. Read-only:
/// the core only runs `git show`.
#[tauri::command]
pub async fn git_commit_files(
    app: tauri::AppHandle,
    hash: String,
) -> Result<Vec<orquestador_core::git::CommitFile>, String> {
    let dir = active_directory(&app)?;
    tauri::async_runtime::spawn_blocking(move || orquestador_core::git::commit_files(&dir, &hash))
        .await
        .map_err(stringify)?
}

/// Every local branch of the active project directory, the current one marked
/// and returned first. Read-only: the core only runs `git for-each-ref`.
#[tauri::command]
pub async fn git_branches(
    app: tauri::AppHandle,
) -> Result<Vec<orquestador_core::git::Branch>, String> {
    let dir = active_directory(&app)?;
    tauri::async_runtime::spawn_blocking(move || orquestador_core::git::branches(&dir))
        .await
        .map_err(stringify)?
}

/// Switch the active project directory to local branch `name`. This is the
/// app's **only mutating** git operation: a plain `git checkout` with no force
/// or stash flags, so a dirty worktree that conflicts fails cleanly. The core
/// validates the name before spawning git.
#[tauri::command]
pub async fn git_checkout(app: tauri::AppHandle, name: String) -> Result<(), String> {
    let dir = active_directory(&app)?;
    tauri::async_runtime::spawn_blocking(move || orquestador_core::git::checkout(&dir, &name))
        .await
        .map_err(stringify)?
}

/// The working-tree paths that differ from `HEAD` in the active project
/// directory, each with its change kind. Read-only: the core only runs
/// `git status`.
#[tauri::command]
pub async fn git_status(
    app: tauri::AppHandle,
) -> Result<Vec<orquestador_core::git::Change>, String> {
    let dir = active_directory(&app)?;
    tauri::async_runtime::spawn_blocking(move || orquestador_core::git::status(&dir))
        .await
        .map_err(stringify)?
}

/// The `HEAD` revision of one path in the active project directory, for the
/// "before" side of a diff. Read-only: the core only runs `git cat-file` /
/// `git show`. `path` must be repository-relative and free of `..` traversal;
/// it is validated here before git is spawned.
#[tauri::command]
pub async fn git_show_file(
    app: tauri::AppHandle,
    path: String,
) -> Result<orquestador_core::git::HeadFile, String> {
    if path.trim().is_empty() {
        return Err("invalid file path: empty".to_string());
    }
    if path.starts_with('/') {
        return Err("invalid file path: must be repository-relative".to_string());
    }
    if path.contains("..") {
        return Err("invalid file path: must not contain '..'".to_string());
    }
    let dir = active_directory(&app)?;
    tauri::async_runtime::spawn_blocking(move || orquestador_core::git::show(&dir, &path))
        .await
        .map_err(stringify)?
}

/// Discard the working-tree changes to one path in the active project
/// directory, restoring it to `HEAD`. Mutating: the core runs `git status`,
/// then `git checkout HEAD -- <path>` / `git reset` / `fs::remove_file`
/// depending on the path's state. `path` must be repository-relative and free
/// of `..` traversal; it is validated here before git is spawned.
#[tauri::command]
pub async fn git_discard_file(app: tauri::AppHandle, path: String) -> Result<(), String> {
    if path.trim().is_empty() {
        return Err("invalid file path: empty".to_string());
    }
    if path.starts_with('/') {
        return Err("invalid file path: must be repository-relative".to_string());
    }
    if path.contains("..") {
        return Err("invalid file path: must not contain '..'".to_string());
    }
    let dir = active_directory(&app)?;
    tauri::async_runtime::spawn_blocking(move || orquestador_core::git::discard_file(&dir, &path))
        .await
        .map_err(stringify)?
}

/// Write `content` to a repository-relative text file in the active project
/// directory — the editor's Ctrl+S / save-prompt path. `path` passes the same
/// guard as `git_show_file` (non-empty, repository-relative, no `..`) before
/// the write runs on a blocking thread.
#[tauri::command]
pub async fn write_file(
    app: tauri::AppHandle,
    path: String,
    content: String,
) -> Result<(), String> {
    if path.trim().is_empty() {
        return Err("invalid file path: empty".to_string());
    }
    if path.starts_with('/') {
        return Err("invalid file path: must be repository-relative".to_string());
    }
    if path.contains("..") {
        return Err("invalid file path: must not contain '..'".to_string());
    }
    let dir = active_directory(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        orquestador_core::files::write_text_file(&dir, &path, &content)
    })
    .await
    .map_err(stringify)?
}
