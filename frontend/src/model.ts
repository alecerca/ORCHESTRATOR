// Selected model store for sending messages with a specific model.
// Stores the chosen model as { providerID, modelID } | null (null = "use the server default").
// Persists to localStorage under `orquestador.model`. Mirrors the structure of skills.ts.
//
// It also remembers the session's own effective model, observed the first time
// it is seen: choosing "use the server default" must send that baseline back,
// because the server keeps whatever model was last recorded on the session.

import type { ModelRef } from "./types";

const STORAGE_KEY = "orquestador.model";

type Listener = () => void;

const EMPTY: ModelRef | null = null;

let loaded = false;
let selected: ModelRef | null = EMPTY;
const listeners = new Set<Listener>();

/**
 * The session's effective model as first observed, or null when none has been
 * seen yet. Cached — later observations reflect the choice we just pushed and
 * must not overwrite it — so "use the server default" has a value to restore.
 * Sending nothing is exactly the bug: the session keeps the last recorded model.
 */
let baseline: ModelRef | null = null;

function load(): ModelRef | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return EMPTY;
    const parsed: unknown = JSON.parse(raw);
    if (parsed && typeof parsed === "object") {
      const p = parsed as Record<string, unknown>;
      if (
        typeof p.providerID === "string" &&
        typeof p.modelID === "string"
      ) {
        return { providerID: p.providerID, modelID: p.modelID };
      }
    }
  } catch {
    // localStorage unavailable or corrupt — start with empty selection
  }
  return EMPTY;
}

function ensureLoaded(): void {
  if (loaded) return;
  loaded = true;
  selected = load();
}

function persist(choice: ModelRef | null): void {
  try {
    if (choice === null) {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(null));
    } else {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(choice));
    }
  } catch {
    // ignore persistence errors
  }
}

function commit(next: ModelRef | null): void {
  selected = next;
  loaded = true;
  persist(next);
  for (const listener of listeners) {
    listener();
  }
}

export function getModel(): ModelRef | null {
  ensureLoaded();
  return selected;
}

export function getSnapshot(): ModelRef | null {
  ensureLoaded();
  return selected;
}

export function setModel(choice: ModelRef | null): void {
  commit(choice);
}

export function subscribe(cb: () => void): () => void {
  listeners.add(cb);
  return () => {
    listeners.delete(cb);
  };
}

/**
 * The session model to re-apply when the user asks for the server default, or
 * null when no session model has been observed yet.
 */
export function getBaseline(): ModelRef | null {
  return baseline;
}

/**
 * Record the session's effective model the first time one is observed. Later
 * observations are ignored: once the user has picked a model the session reports
 * that pick, which must not become the new "default".
 */
export function observeBaseline(model: ModelRef | null): void {
  if (baseline === null && model !== null) {
    baseline = { providerID: model.providerID, modelID: model.modelID };
  }
}
