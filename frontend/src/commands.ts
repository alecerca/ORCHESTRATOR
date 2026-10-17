// Typed Tauri command bindings (S4).
//
// Every mutation goes through the Rust shell — the webview never talks to the
// OpenCode server directly. Tauri 2 converts the camelCase JS keys below to the
// snake_case Rust parameter names (`sessionId` <-> `session_id`).

import { invoke } from "@tauri-apps/api/core";
import type {
  Agent,
  AppStateSnapshot,
  FileContent,
  FileNode,
  FileStatus,
  GitBranch,
  GitChange,
  GitCommitFile,
  GitLog,
  HeadFile,
  Health,
  MessageBundle,
  ModelRef,
  PermissionResponse,
  PresenceEntry,
  Project,
  ProviderAuthMethod,
  ProviderList,
  ProxyMember,
  ServerConfig,
  Session,
  SessionStatus,
  SkillInfo,
  Todo,
} from "./types";

export const getState = () => invoke<AppStateSnapshot>("get_state");

// -- health / discovery ------------------------------------------------------
export const getHealth = () => invoke<Health>("get_health");
export const getAgents = () => invoke<Agent[]>("get_agents");
export const getProviders = () => invoke<ProviderList>("get_providers");
export const getProviderAuth = () =>
  invoke<Record<string, ProviderAuthMethod[]>>("get_provider_auth");
/**
 * Store an API key for a provider (`PUT /auth/:providerID`) so it can serve
 * requests. The key is write-only: it is sent straight to the shell, never
 * persisted in the webview, and never returned. Rejects with the server's
 * reason when the key is refused.
 */
export const setProviderApiKey = (providerId: string, key: string) =>
  invoke<boolean>("set_provider_api_key", { providerId, key });
export const sessionStatuses = () =>
  invoke<Record<string, SessionStatus>>("session_statuses");

// -- sessions ----------------------------------------------------------------
export const listSessions = () => invoke<Session[]>("list_sessions");
export const createSession = (parentId?: string, title?: string) =>
  invoke<Session>("create_session", { parentId, title });
export const listChildren = (sessionId: string) =>
  invoke<Session[]>("list_children", { sessionId });
export const deleteSession = (sessionId: string) =>
  invoke<boolean>("delete_session", { sessionId });
export const renameSession = (sessionId: string, title: string) =>
  invoke<Session>("rename_session", { sessionId, title });
/**
 * Set the session's effective model (`POST /api/session/:id/model`). The session
 * is the only authoritative source of the model a request runs, so a choice must
 * be recorded here to stay sticky — and the server default is restored by
 * re-applying a known baseline, not by sending nothing.
 */
export const setSessionModel = (sessionId: string, model: ModelRef) =>
  invoke<void>("set_session_model", { sessionId, model });
export const getTodos = (sessionId: string) =>
  invoke<Todo[]>("get_todos", { sessionId });

// -- instructions ------------------------------------------------------------
export const sendMessage = (
  sessionId: string,
  text: string,
  agent?: string,
  skills?: string[],
  model?: ModelRef | null,
) =>
  invoke<MessageBundle>("send_message", {
    sessionId,
    text,
    agent,
    skills,
    model,
  });
export const sendMessageAsync = (
  sessionId: string,
  text: string,
  agent?: string,
  skills?: string[],
  model?: ModelRef | null,
) =>
  invoke<void>("send_message_async", {
    sessionId,
    text,
    agent,
    skills,
    model,
  });
export const sendCommand = (
  sessionId: string,
  command: string,
  args?: string,
) => invoke<MessageBundle>("send_command", { sessionId, command, arguments: args });
export const abortSession = (sessionId: string) =>
  invoke<boolean>("abort_session", { sessionId });

// -- messages ----------------------------------------------------------------
export const getMessages = (sessionId: string, limit?: number) =>
  invoke<MessageBundle[]>("get_messages", { sessionId, limit });
export const getMessage = (sessionId: string, messageId: string) =>
  invoke<MessageBundle>("get_message", { sessionId, messageId });

// -- approvals ---------------------------------------------------------------
export const respondPermission = (
  sessionId: string,
  permissionId: string,
  response: PermissionResponse,
  remember?: boolean,
) =>
  invoke<boolean>("respond_permission", {
    sessionId,
    permissionId,
    response,
    remember,
  });

// -- files -------------------------------------------------------------------
export const listFiles = (path?: string) =>
  invoke<FileNode[]>("list_files", { path });
export const readFile = (path: string) =>
  invoke<FileContent>("read_file", { path });
/** Write a text file back to the active project directory (editor save). */
export const writeFile = (path: string, content: string) =>
  invoke<void>("write_file", { path, content });
export const fileStatus = () => invoke<FileStatus[]>("file_status");
export const findFiles = (query: string, limit?: number) =>
  invoke<string[]>("find_files", { query, limit });
export const findSymbols = (query: string) => invoke<unknown[]>("find_symbols", { query });

// -- skills ------------------------------------------------------------------
export const listSkills = () => invoke<SkillInfo[]>("list_skills");
export const readSkill = (name: string) => invoke<string>("read_skill", { name });
export const saveSkill = (name: string, content: string) =>
  invoke<SkillInfo>("save_skill", { name, content });
export const deleteSkill = (name: string) =>
  invoke<boolean>("delete_skill", { name });

// -- server config ------------------------------------------------------------
export const testConnection = (baseUrl: string, username?: string, password?: string) =>
  invoke<Health>("test_connection", { baseUrl, username, password });
export const getServerConfig = () => invoke<ServerConfig>("get_server_config");
export const setServerConfig = (
  baseUrl: string,
  username?: string,
  password?: string,
  memberName?: string,
  proxyConfigPath?: string,
) =>
  invoke<ServerConfig>("set_server_config", {
    baseUrl,
    username,
    password,
    memberName,
    proxyConfigPath,
  });

// -- proxy whitelist (host side) ---------------------------------------------
//
// The host console edits the owner's `proxy.json` at a caller-supplied path.
// These only read/write that file; the proxy stays the enforcement boundary.
export const getProxyMembers = (path: string) =>
  invoke<ProxyMember[]>("get_proxy_members", { path });
export const addProxyMember = (
  path: string,
  name: string,
  role: string,
  password: string,
) => invoke<void>("add_proxy_member", { path, name, role, password });
export const removeProxyMember = (path: string, name: string) =>
  invoke<void>("remove_proxy_member", { path, name });

// -- presence ----------------------------------------------------------------
/**
 * The members currently connected through the proxy. Empty when presence is
 * unavailable (a direct, proxy-less connection) — that is a normal feature-off
 * result, never an error. Each poll also keeps this client visible to others.
 */
export const getPresence = () => invoke<PresenceEntry[]>("get_presence");

/**
 * The project the workspace is pinned to (absolute path), or `null` when the
 * app decides. This value comes from the proxy (`GET /orquestador/whoami`) and
 * never rejects; a transport error, 404, 401, malformed body, or `project: null`
 * all return `null`.
 */
export const getWorkspaceScope = () => invoke<string | null>("get_workspace_scope");

// -- projects -----------------------------------------------------------------
/** Every project the running server knows about. */
export const listProjects = () => invoke<Project[]>("list_projects");
/** The project the app is currently scoped to. */
export const currentProject = () => invoke<Project>("current_project");
/**
 * Scope the app to a project directory (absolute path) and persist the choice.
 * Pass `null` to fall back to the server's own cwd project. Returns the updated
 * server config; the client reconnects and re-bootstraps on the new project.
 */
export const setProjectDirectory = (directory: string | null) =>
  invoke<ServerConfig>("set_project_directory", { directory });

// -- git ---------------------------------------------------------------------
/**
 * The git branch of a session's working directory, or `null` when it cannot be
 * determined (empty directory, not a repository, or git unavailable). Read-only:
 * the shell only ever runs `git rev-parse`.
 */
export const gitBranch = (directory: string) =>
  invoke<string | null>("git_branch", { directory });

/**
 * The active project's branch and recent commits (newest first). Read-only.
 * Rejects with a message when no project directory has been selected, the
 * directory is not a git repository, or it has no commits yet.
 */
export const gitLog = (limit?: number) =>
  invoke<GitLog>("git_log", { limit });

/** The files changed by one commit, each with its status letter. Read-only. */
export const gitCommitFiles = (hash: string) =>
  invoke<GitCommitFile[]>("git_commit_files", { hash });

/**
 * The active project's local branches, the current one marked and returned
 * first. Read-only: the shell only runs `git for-each-ref`.
 */
export const gitBranches = () => invoke<GitBranch[]>("git_branches");

/**
 * Switch the active project to local branch `name`. This is the app's only
 * mutating git operation; a dirty worktree that conflicts rejects with a
 * readable message instead of losing work.
 */
export const gitCheckout = (name: string) =>
  invoke<void>("git_checkout", { name });

/**
 * The active project's working-tree changes: every path that differs from HEAD,
 * each with its change kind. Read-only.
 */
export const gitStatus = () => invoke<GitChange[]>("git_status");

/**
 * The HEAD revision of one repository-relative path: its kind and, for text,
 * its content. Read-only. Rejects for empty, absolute or `..`-containing paths.
 */
export const gitShowFile = (path: string) =>
  invoke<HeadFile>("git_show_file", { path });

/**
 * Discard the working-tree changes to one repository-relative path, restoring
 * it to HEAD. Mutating: untracked files are removed, staged additions are
 * unstaged and removed, and modified/deleted files are checked out from HEAD.
 * Rejects for empty, absolute or `..`-containing paths.
 */
export const gitDiscardFile = (path: string) =>
  invoke<void>("git_discard_file", { path });

