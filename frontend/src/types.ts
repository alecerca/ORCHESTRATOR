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
    };

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

/** A stored skill as returned by the `list_skills` / `save_skill` commands. */
export interface SkillInfo {
  name: string;
  path: string;
}

// ---------------------------------------------------------------------------
// Files / diffs
// ---------------------------------------------------------------------------

export interface FileDiff {
  file: string;
  before: string;
  after: string;
  additions: number;
  deletions: number;
}

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
