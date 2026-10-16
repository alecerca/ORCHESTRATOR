// Wire types mirroring the `orquestador-core` serde shapes.
//
// The authoritative definitions live in `crates/orquestador-core/src/types.rs`
// and `src/state.rs`. Keep the JSON keys EXACTLY as the Rust side renames them
// (note the capitalised abbreviations: `sessionID`, `messageID`, `projectID`,
// `parentID`) — a naive camelCase pass would be wrong.

export type AgentUiStatus =
  | "pending"
  | "idle"
  | "done"
  | "running"
  | "waiting_approval"
  | "error";

export interface SessionTime {
  created: number;
  updated: number;
  compacting?: number;
}

export interface Session {
  id: string;
  projectID: string;
  directory: string;
  parentID?: string;
  title: string;
  version: string;
  time: SessionTime;
  /**
   * Effective model for this session, as the server reports it. This is the
   * only authoritative source of the model the session uses (the app-level
   * config carries none). Note the id field is `id`, NOT `modelID`.
   */
  model?: SessionModel | null;
}

/** The effective model of a {@link Session}: `{ id, providerID, variant? }`. */
export interface SessionModel {
  id: string;
  providerID: string;
  variant?: string | null;
}

export type SessionStatus =
  | { type: "idle" }
  | { type: "busy" }
  | { type: "retry"; attempt: number; message: string; next: number };

export interface ModelRef {
  providerID: string;
  modelID: string;
}

export interface Permission {
  id: string;
  type: string;
  sessionID: string;
  messageID: string;
  callID?: string;
  title: string;
  metadata?: unknown;
}

/** The set of answers the server accepts (NOT allow/deny). */
export type PermissionResponse = "once" | "always" | "reject";

export interface Agent {
  name: string;
  description?: string;
  type: string;
  builtIn: boolean;
  color?: string;
  model?: ModelRef;
}

/** `GET /global/health` */
export interface Health {
  healthy: boolean;
  version: string;
}

export interface ProjectTime {
  created: number;
  initialized?: number;
}

/** `GET /project`, `GET /project/current` — a project the server can serve. */
export interface Project {
  id: string;
  /** Absolute directory of the project's worktree; used as the scope value. */
  worktree: string;
  vcsDir?: string;
  vcs?: string;
  time: ProjectTime;
}

export interface Todo {
  id: string;
  content: string;
  status: string;
  priority: string;
}

export interface SessionSnapshot {
  session: Session;
  status: AgentUiStatus;
  error?: string;
  server_status?: SessionStatus;
  /** Epoch ms when the derived status last changed (approximate on first observation). */
  status_since?: number;
  /** Duration in ms of the previous run (set when leaving running/waiting_approval). */
  last_run_ms?: number;
}

export interface AppStateSnapshot {
  connected: boolean;
  server_version?: string | null;
  root_session_id?: string | null;
  sessions: SessionSnapshot[];
  permissions: Permission[];
  agents: Agent[];
  todos: Record<string, Todo[]>;
}

// `Effect` is internally tagged on `effect` (serde `tag = "effect"`).
export type Effect =
  | { effect: "finished"; session_id: string; title: string }
  | { effect: "failed"; session_id: string; title: string; message: string }
  | {
      effect: "approval";
      session_id: string;
      permission_id: string;
      title: string;
    }
  | { effect: "usage_limit"; session_id: string; title: string; message: string };

// `CoreMessage` is internally tagged on `kind` (serde `tag = "kind"`).
export type CoreMessage =
  | { kind: "raw_event"; event: unknown }
  | { kind: "state_snapshot"; snapshot: AppStateSnapshot }
  | { kind: "effect"; effect: Effect };

// ---------------------------------------------------------------------------
// Messages / parts (stream UI)
// ---------------------------------------------------------------------------

export interface ToolTime {
  start: number;
  end?: number;
}

export type ToolState =
  | { status: "pending"; input?: unknown; raw?: string }
  | { status: "running"; input?: unknown; title?: string; time?: ToolTime }
  | {
      status: "completed";
      input?: unknown;
      output?: string;
      title?: string;
      metadata?: unknown;
      time?: ToolTime;
    }
  | {
      status: "error";
      input?: unknown;
      error?: string;
      metadata?: unknown;
      time?: ToolTime;
    };

/**
 * A message part. Kept deliberately loose (an index signature plus the common
 * optional fields) so a new part type on the server never breaks the stream
 * renderer — it simply renders its fallback shape.
 */
export interface Part {
  id: string;
  sessionID: string;
  messageID: string;
  type: string;
  text?: string;
  tool?: string;
  callID?: string;
  state?: ToolState;
  prompt?: string;
  description?: string;
  agent?: string;
  name?: string;
  filename?: string;
  reason?: string;
  [key: string]: unknown;
}

export interface MessageErrorInfo {
  name?: string;
  data?: { message?: string; statusCode?: number };
}

export interface Message {
  id: string;
  sessionID: string;
  role: "user" | "assistant" | string;
  time: { created: number; completed?: number };
  error?: MessageErrorInfo;
  agent?: string;
  [key: string]: unknown;
}

/** `{ info, parts }` — every message-returning endpoint. */
export interface MessageBundle {
  info: Message;
  parts: Part[];
}

// ---------------------------------------------------------------------------
// Skills (Orquestador-owned markdown instruction files)
// ---------------------------------------------------------------------------

/**
 * One available skill as returned by the `list_skills` / `save_skill` commands.
 * `source` is `"app"` for Orquestador-owned skills, otherwise the connected
 * backend id (e.g. `"opencode"`); `editable` is true only for app-owned skills
 * — native backend skills are read-only.
 */
export interface SkillInfo {
  name: string;
  path: string;
  source: string;
  editable: boolean;
}

// ---------------------------------------------------------------------------
// Providers (OpenCode `GET /provider` / `GET /provider/auth`)
// ---------------------------------------------------------------------------

/**
 * One model offered by a provider. Credential-bearing wire fields (`options`,
 * `headers`) are deliberately omitted by the shell and must never be shown.
 */
export interface ProviderModel {
  id: string;
  name: string;
  providerID: string;
  family?: string | null;
  /** Token pricing; `null` when the catalogue omits it. */
  cost?: ModelCost | null;
  /** `"active"` | `"deprecated"`. */
  status?: string | null;
  /** Computed in core: every cost component zero, or a `:free`/`-free` id. */
  free: boolean;
}

/** Per-million-token pricing; the nested `cache` object is kept nested. */
export interface ModelCost {
  input: number;
  output: number;
  cache: { read: number; write: number };
}

export interface ModelRef {
  providerID: string;
  modelID: string;
}

/**
 * One AI provider. The credential-bearing wire fields (`key`, `options`) are
 * deliberately omitted by the shell and must never be shown.
 */
export interface Provider {
  id: string;
  name: string;
  source: string;
  /** Environment variable names this provider can read a credential from. */
  env: string[];
  /** Models offered by this provider, keyed by model id. */
  models: Record<string, ProviderModel>;
  /** Declares at least one auth method (`GET /provider/auth`). */
  hasAuth: boolean;
  /** Can serve a model right now (already authenticated). */
  usable: boolean;
}

/**
 * Response of the `get_providers` command: `connected` holds the provider ids
 * that are already authenticated (the per-provider auth status), and `default`
 * maps `providerID` → configured default `modelID`.
 */
export interface ProviderList {
  all: Provider[];
  default: Record<string, string>;
  connected: string[];
}

/**
 * One supported auth method for a provider. `type` is `"oauth"` or `"api"`.
 * `prompts` are opaque server-side UI prompt definitions (never credential
 * values).
 */
export interface ProviderAuthMethod {
  type: string;
  label: string;
  prompts: unknown[];
}

// ---------------------------------------------------------------------------
// Server config
// ---------------------------------------------------------------------------

/** Remote OpenCode server configuration (stored in localStorage). */
export interface ServerConfig {
  baseUrl: string;
  username?: string;
  password?: string | null;
  /** Active project directory (`x-opencode-directory`); unset = server cwd. */
  directory?: string | null;
  /**
   * Optional display name used to attribute sessions this app creates
   * (`[<member>] <title>`); falls back to `username`, then `"opencode"`.
   */
  memberName?: string;
  /**
   * Optional path to the host's whitelist `proxy.json`, edited by the Members
   * dialog. UI-only: it is never sent to the OpenCode server.
   */
  proxyConfigPath?: string;
}

/**
 * One whitelist member from `proxy.json`, as returned by `get_proxy_members`.
 * Credentials (password hashes / legacy tokens) are deliberately omitted by the
 * shell and must never be shown.
 */
export interface ProxyMember {
  name: string;
  role: string;
}

// ---------------------------------------------------------------------------
// Presence (who is currently connected through the proxy)
// ---------------------------------------------------------------------------

/**
 * One currently-connected member from the `get_presence` command. Only clients
 * talking through the proxy appear; a direct (proxy-less) connection yields an
 * empty list — a normal "presence off" result, never an error.
 */
export interface PresenceEntry {
  name: string;
  role: "editor" | "viewer";
  /** Milliseconds since this member's last request through the proxy. */
  idleMs: number;
}

// ---------------------------------------------------------------------------
// Git (read-only history / changed files)
// ---------------------------------------------------------------------------

/** One commit as returned by the `git_log` command (newest first). */
export interface GitCommit {
  hash: string;
  short: string;
  subject: string;
  author: string;
  date: string;
}

/** One path changed by a commit, with its status letter (`A`/`M`/`D`/`R`/…). */
export interface GitCommitFile {
  path: string;
  status: string;
}

/** The active project's current branch and its commits. */
export interface GitLog {
  branch: string | null;
  commits: GitCommit[];
}

/** One local branch as returned by the `git_branches` command. */
export interface GitBranch {
  name: string;
  current: boolean;
}

/** How a working-tree path differs from HEAD, as returned by `git_status`. */
export type GitChangeKind = "modified" | "added" | "deleted" | "untracked";

/** One working-tree path that differs from HEAD, with its change kind. */
export interface GitChange {
  path: string;
  kind: GitChangeKind;
}

/** How the HEAD revision of a path reads, as returned by `git_show_file`. */
export type HeadFileKind = "text" | "binary" | "missing";

/** The HEAD revision of one path: its kind and, for text, its content. */
export interface HeadFile {
  kind: HeadFileKind;
  content: string | null;
}

// ---------------------------------------------------------------------------
// Files
// ---------------------------------------------------------------------------

export type FileNodeKind = "file" | "directory";

export interface FileNode {
  name: string;
  path: string;
  absolute: string;
  type: FileNodeKind;
  ignored?: boolean;
}

export interface FileContent {
  type: "text" | "binary";
  content: string;
  diff?: string;
  encoding?: string;
  mimeType?: string;
}

export type FileStatusKind = "added" | "deleted" | "modified";

export interface FileStatus {
  path: string;
  added: number;
  removed: number;
  status: FileStatusKind;
}

/** Tauri event name emitted by the Rust shell. */
export const CORE_EVENT = "core://message";
