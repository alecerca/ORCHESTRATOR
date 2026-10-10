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

export interface Agent {
  name: string;
  description?: string;
  type: string;
  builtIn: boolean;
  color?: string;
  model?: ModelRef;
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

/** Tauri event name emitted by the Rust shell. */
export const CORE_EVENT = "core://message";
