// Selected model store for sending messages with a specific model.
// Stores the chosen model as { providerID, modelID } | null (null = "use the server default").
// Persists to localStorage under `orquestador.model`. Mirrors the structure of skills.ts.

const STORAGE_KEY = "orquestador.model";

type Listener = () => void;

const EMPTY: { providerID: string; modelID: string } | null = null;

let loaded = false;
let selected: { providerID: string; modelID: string } | null = EMPTY;
const listeners = new Set<Listener>();

function load(): { providerID: string; modelID: string } | null {
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

function persist(choice: { providerID: string; modelID: string } | null): void {
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

function commit(next: { providerID: string; modelID: string } | null): void {
  selected = next;
  loaded = true;
  persist(next);
  for (const listener of listeners) {
    listener();
  }
}

export function getModel(): { providerID: string; modelID: string } | null {
  ensureLoaded();
  return selected;
}

export function getSnapshot(): { providerID: string; modelID: string } | null {
  ensureLoaded();
  return selected;
}

export function setModel(choice: { providerID: string; modelID: string } | null): void {
  commit(choice);
}

export function subscribe(cb: () => void): () => void {
  listeners.add(cb);
  return () => {
    listeners.delete(cb);
  };
}
