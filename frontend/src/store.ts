// Minimal store on `useSyncExternalStore` (no zustand/redux — SPEC §5).
//
// The Rust core owns the single source of truth. This store only caches:
//   * the latest `AppStateSnapshot` (state-changing events),
//   * a bounded ring buffer of raw core messages (the live stream),
//   * a per-session reducer over `message.*` / `message.part.*` events so a
//     pane can render incrementally without re-folding the whole buffer.
//
// High-frequency message/part events are display-only and are NOT pushed into
// `snapshot`; components that need the stream use `useSessionStream`.

import { useSyncExternalStore } from "react";
import type {
  AppStateSnapshot,
  CoreMessage,
  Message,
  MessageBundle,
  Part,
  Permission,
  QuestionRequest,
  SessionSnapshot,
} from "./types";

type Listener = () => void;

/** Raw events kept for the "activity" view; the rest lives in per-session streams. */
const RAW_LIMIT = 500;

const EMPTY_SNAPSHOT: AppStateSnapshot = {
  connected: false,
  server_version: null,
  root_session_id: null,
  sessions: [],
  permissions: [],
  questions: [],
  agents: [],
  todos: {},
};
const EMPTY_SESSIONS: SessionSnapshot[] = [];
const EMPTY_PERMISSIONS: Permission[] = [];
const EMPTY_QUESTIONS: QuestionRequest[] = [];
const EMPTY_STREAM: StreamMessage[] = [];
const EMPTY_RAW_EVENTS: unknown[] = [];

export interface StreamMessage {
  info: Message;
  parts: Part[];
}

interface RawRecord {
  event: unknown;
  sessionId?: string;
}

interface InternalMessage {
  info: Message;
  parts: Map<string, Part>;
  order: string[];
}

interface InternalStream {
  messages: Map<string, InternalMessage>;
}

// -- module state ------------------------------------------------------------

let snapshot: AppStateSnapshot | null = null;
let lastEvent: CoreMessage | null = null;
let rawEvents: RawRecord[] = [];
const streams = new Map<string, InternalStream>();

// Derived caches. Cleared on every mutation; re-computed lazily so hooks can
// hand `useSyncExternalStore` a stable reference between notifications.
const streamCache = new Map<string, StreamMessage[]>();
const rawCache = new Map<string, RawRecord[]>();

const listeners = new Set<Listener>();

function subscribe(listener: Listener): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

function notify(): void {
  for (const listener of listeners) {
    listener();
  }
}

function invalidate(): void {
  streamCache.clear();
  rawCache.clear();
  notify();
}

// -- stream reducer ----------------------------------------------------------

function ensureStream(sessionId: string): InternalStream {
  let stream = streams.get(sessionId);
  if (!stream) {
    stream = { messages: new Map() };
    streams.set(sessionId, stream);
  }
  return stream;
}

function ensureMessage(stream: InternalStream, messageId: string, sessionId: string): InternalMessage {
  let message = stream.messages.get(messageId);
  if (!message) {
    message = {
      info: {
        id: messageId,
        sessionID: sessionId,
        role: "assistant",
        time: { created: Date.now() },
      },
      parts: new Map(),
      order: [],
    };
    stream.messages.set(messageId, message);
  }
  return message;
}

function upsertMessageInfo(stream: InternalStream, info: Message): void {
  const message = ensureMessage(stream, info.id, info.sessionID);
  message.info = info;
}

function upsertPart(stream: InternalStream, part: Part): void {
  const message = ensureMessage(stream, part.messageID, part.sessionID);
  if (!message.parts.has(part.id)) {
    message.order.push(part.id);
  }
  message.parts.set(part.id, part);
}

function removeMessage(stream: InternalStream, messageId: string): void {
  stream.messages.delete(messageId);
}

function removePart(stream: InternalStream, partId: string): void {
  for (const message of stream.messages.values()) {
    if (message.parts.delete(partId)) {
      message.order = message.order.filter((id) => id !== partId);
    }
  }
}

function sessionIdOf(event: unknown): string | undefined {
  if (!event || typeof event !== "object") return undefined;
  const props = (event as { properties?: Record<string, unknown> }).properties;
  if (!props) return undefined;
  if (typeof props.sessionID === "string") return props.sessionID;
  const part = props.part as { sessionID?: unknown } | undefined;
  if (part && typeof part.sessionID === "string") return part.sessionID;
  const info = props.info as { sessionID?: unknown } | undefined;
  if (info && typeof info.sessionID === "string") return info.sessionID;
  return undefined;
}

function applyRawEvent(event: unknown): void {
  if (!event || typeof event !== "object") return;
  const typed = event as { type?: string; properties?: Record<string, any> };
  if (!typed.type || !typed.properties) return;
  const props = typed.properties;
  switch (typed.type) {
    case "message.updated": {
      const info = props.info as Message | undefined;
      if (info?.sessionID) upsertMessageInfo(ensureStream(info.sessionID), info);
      break;
    }
    case "message.removed": {
      if (typeof props.sessionID === "string") {
        removeMessage(ensureStream(props.sessionID), String(props.messageID));
      }
      break;
    }
    case "message.part.updated": {
      const part = props.part as Part | undefined;
      if (part?.sessionID) upsertPart(ensureStream(part.sessionID), part);
      break;
    }
    case "message.part.removed": {
      if (typeof props.sessionID === "string") {
        removePart(ensureStream(props.sessionID), String(props.partID));
      }
      break;
    }
    default:
      break;
  }
}

function deriveStream(stream: InternalStream): StreamMessage[] {
  const messages = [...stream.messages.values()];
  messages.sort(
    (a, b) =>
      (a.info.time?.created ?? 0) - (b.info.time?.created ?? 0) ||
      a.info.id.localeCompare(b.info.id),
  );
  return messages.map((message) => ({
    info: message.info,
    parts: message.order
      .map((id) => message.parts.get(id))
      .filter((part): part is Part => part !== undefined),
  }));
}

function getStreamSnapshot(sessionId: string): StreamMessage[] {
  const cached = streamCache.get(sessionId);
  if (cached) return cached;
  const stream = streams.get(sessionId);
  const derived = stream ? deriveStream(stream) : EMPTY_STREAM;
  streamCache.set(sessionId, derived);
  return derived;
}

function getRawSnapshot(sessionId?: string): RawRecord[] {
  const key = sessionId ?? "__all__";
  const cached = rawCache.get(key);
  if (cached) return cached;
  const filtered = sessionId
    ? rawEvents.filter((record) => record.sessionId === sessionId)
    : rawEvents.slice();
  rawCache.set(key, filtered);
  return filtered;
}

// -- public store ------------------------------------------------------------

export const store = {
  /** Replace the cached snapshot (e.g. from the `get_state` command). */
  setState(next: AppStateSnapshot): void {
    snapshot = next;
    invalidate();
  },

  /** Record an inbound core message; folds snapshots and raw events in. */
  pushMessage(message: CoreMessage): void {
    lastEvent = message;
    if (message.kind === "state_snapshot") {
      snapshot = message.snapshot;
    } else if (message.kind === "raw_event") {
      const record: RawRecord = {
        event: message.event,
        sessionId: sessionIdOf(message.event),
      };
      rawEvents.push(record);
      if (rawEvents.length > RAW_LIMIT) {
        rawEvents = rawEvents.slice(rawEvents.length - RAW_LIMIT);
      }
      applyRawEvent(message.event);
    }
    invalidate();
  },

  /** Seed a pane's history (from `get_messages`) before live updates arrive. */
  hydrateStream(sessionId: string, bundles: MessageBundle[]): void {
    const stream = ensureStream(sessionId);
    for (const bundle of bundles) {
      upsertMessageInfo(stream, bundle.info);
      for (const part of bundle.parts) {
        upsertPart(stream, part);
      }
    }
    invalidate();
  },

  /** Forget a pane's stream (called when its pane is closed). */
  dropStream(sessionId: string): void {
    streams.delete(sessionId);
    streamCache.delete(sessionId);
    rawCache.delete("__all__");
    rawCache.delete(sessionId);
    notify();
  },
};

// -- hooks -------------------------------------------------------------------

function getAppSnapshot(): AppStateSnapshot {
  return snapshot ?? EMPTY_SNAPSHOT;
}

function getLastEvent(): CoreMessage | null {
  return lastEvent;
}

export function useAppState(): AppStateSnapshot {
  return useSyncExternalStore(subscribe, getAppSnapshot, getAppSnapshot);
}

export function useLastMessage(): CoreMessage | null {
  return useSyncExternalStore(subscribe, getLastEvent, getLastEvent);
}

export function useSessions(): SessionSnapshot[] {
  const state = useSyncExternalStore(subscribe, getAppSnapshot, getAppSnapshot);
  return state.sessions.length > 0 ? state.sessions : EMPTY_SESSIONS;
}

export function usePendingPermissions(): Permission[] {
  const state = useSyncExternalStore(subscribe, getAppSnapshot, getAppSnapshot);
  return state.permissions.length > 0 ? state.permissions : EMPTY_PERMISSIONS;
}

/** Clarification batches waiting for answers (the Rust core's registry). */
export function usePendingQuestions(): QuestionRequest[] {
  const state = useSyncExternalStore(subscribe, getAppSnapshot, getAppSnapshot);
  return state.questions.length > 0 ? state.questions : EMPTY_QUESTIONS;
}

export function useSessionStream(sessionId: string | null): StreamMessage[] {
  const get = (): StreamMessage[] => (sessionId ? getStreamSnapshot(sessionId) : EMPTY_STREAM);
  return useSyncExternalStore(subscribe, get, get);
}

export function useRawEvents(sessionId?: string): unknown[] {
  const get = (): unknown[] => {
    const records = getRawSnapshot(sessionId);
    if (records.length === 0) return EMPTY_RAW_EVENTS;
    return records.map((record) => record.event);
  };
  return useSyncExternalStore(subscribe, get, get);
}
