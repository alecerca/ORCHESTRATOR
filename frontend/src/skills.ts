// Selected-skills store for the header Skills panel.
//
// A "skill" is an Orquestador-owned markdown instruction file kept by the Rust
// shell at `<app_config_dir>/skills/<name>.md` — distinct from opencode's own
// native skills, so the app never mutates the user's global opencode config.
// This module tracks which skill names are *selected* and persists them to
// `localStorage` under `orquestador.skills` (a JSON array of names), matching
// the app's existing persistence pattern.
//
// "Selected" skills are hidden and universal: the shell reads each one and
// folds them into the `system` field of every `send_message` /
// `send_message_async` prompt, so the agent can apply whichever skill is
// relevant on every turn without polluting the visible transcript.
//
// State lives in a module-level array that keeps a stable reference between
// changes, so `useSyncExternalStore` consumers can pair `subscribe` with
// `getSnapshot`.

const STORAGE_KEY = "orquestador.skills";

type Listener = () => void;

/** Frozen empty selection reused so consumers get a stable reference. */
const EMPTY: string[] = [];

let loaded = false;
let selected: string[] = EMPTY;
const listeners = new Set<Listener>();

/** Read the persisted names; a missing/corrupt/unavailable store degrades to `[]`. */
function load(): string[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return EMPTY;
    const parsed: unknown = JSON.parse(raw);
    if (Array.isArray(parsed)) {
      const names = parsed.filter((name): name is string => typeof name === "string");
      return names.length > 0 ? names : EMPTY;
    }
  } catch {
    // localStorage unavailable or corrupt — start with an empty selection.
  }
  return EMPTY;
}

/** Load from storage on first access only; later reads use the in-memory copy. */
function ensureLoaded(): void {
  if (loaded) return;
  loaded = true;
  selected = load();
}

/** Best-effort persist; an unavailable store leaves the in-memory copy intact. */
function persist(names: string[]): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(names));
  } catch {
    // localStorage unavailable or full — the selection stays in memory only.
  }
}

/** Swap the selection, persist it, and notify every subscriber. */
function commit(next: string[]): void {
  selected = next;
  loaded = true;
  persist(next);
  for (const listener of listeners) {
    listener();
  }
}

/** Names currently selected, as a fresh copy safe for callers to mutate. */
export function getSelectedNames(): string[] {
  ensureLoaded();
  return selected.slice();
}

/** Stable snapshot reference for `useSyncExternalStore` (changes only on mutation). */
export function getSnapshot(): string[] {
  ensureLoaded();
  return selected;
}

export function isSelected(name: string): boolean {
  ensureLoaded();
  return selected.includes(name);
}

/** Replace the whole selection with `names`. */
export function setSelected(names: string[]): void {
  commit(names.slice());
}

/** Add `name` if absent, remove it if present. */
export function toggle(name: string): void {
  ensureLoaded();
  commit(
    selected.includes(name)
      ? selected.filter((current) => current !== name)
      : [...selected, name],
  );
}

/** Remove `name`; a name that is not selected is a no-op (no notification). */
export function remove(name: string): void {
  ensureLoaded();
  if (!selected.includes(name)) return;
  commit(selected.filter((current) => current !== name));
}

/** Subscribe to selection changes. */
export function subscribe(cb: () => void): () => void {
  listeners.add(cb);
  return () => {
    listeners.delete(cb);
  };
}
