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
  FileDiff,
  FileNode,
  FileStatus,
  Health,
  MessageBundle,
  PermissionResponse,
  Project,
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
export const getTodos = (sessionId: string) =>
  invoke<Todo[]>("get_todos", { sessionId });

// -- instructions ------------------------------------------------------------
export const sendMessage = (
  sessionId: string,
  text: string,
  agent?: string,
  skills?: string[],
) => invoke<MessageBundle>("send_message", { sessionId, text, agent, skills });
export const sendMessageAsync = (
  sessionId: string,
  text: string,
  agent?: string,
  skills?: string[],
) => invoke<void>("send_message_async", { sessionId, text, agent, skills });
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

// -- diffs / files -----------------------------------------------------------
export const getDiff = (sessionId: string, messageId?: string) =>
  invoke<FileDiff[]>("get_diff", { sessionId, messageId });
export const listFiles = (path?: string) =>
  invoke<FileNode[]>("list_files", { path });
export const readFile = (path: string) =>
  invoke<FileContent>("read_file", { path });
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
export const setServerConfig = (baseUrl: string, username?: string, password?: string) =>
  invoke<ServerConfig>("set_server_config", { baseUrl, username, password });

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

