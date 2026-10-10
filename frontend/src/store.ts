// Minimal store on `useSyncExternalStore` (no zustand/redux — SPEC §5).
//
// The Rust core owns the single source of truth. This store only caches:
//   * the latest `AppStateSnapshot` (state-changing events), and
//   * the latest raw `CoreMessage` (for the live message/part stream).
// High-frequency message/part events are display-only and are NOT pushed into
// `snapshot`; components that need the stream subscribe to the raw event.

import { useSyncExternalStore } from "react";
import type { AppStateSnapshot, CoreMessage } from "./types";

type Listener = () => void;

let snapshot: AppStateSnapshot | null = null;
let lastEvent: CoreMessage | null = null;
const listeners = new Set<Listener>();

function notify(): void {
  for (const listener of listeners) {
    listener();
  }
}

function subscribe(listener: Listener): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

function getSnapshot(): AppStateSnapshot | null {
  return snapshot;
}

function getLastEvent(): CoreMessage | null {
  return lastEvent;
}

export const store = {
  /** Replace the cached snapshot (e.g. from the `get_state` command). */
  setState(next: AppStateSnapshot): void {
    snapshot = next;
    notify();
  },
  /** Record an inbound core message and fold snapshots into the cache. */
  pushMessage(message: CoreMessage): void {
    lastEvent = message;
    if (message.kind === "state_snapshot") {
      snapshot = message.snapshot;
    }
    notify();
  },
};

export function useAppState(): AppStateSnapshot | null {
  return useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
}

export function useLastMessage(): CoreMessage | null {
  return useSyncExternalStore(subscribe, getLastEvent, getLastEvent);
}
