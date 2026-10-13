// In-memory drafts for open file tabs.
//
// A file tab names a path; its editable content lives in this module until the
// user saves (Ctrl+S in the editor, or Save in the close prompt), at which point
// the shell writes it back through `write_file`. Keying by path (not tab id)
// means two groups that open the same file share one draft, and dirtiness is
// defined against `baseline` — the content last loaded from disk or saved.
//
// Files always exist when opened, so `baseline` is a plain string (never null):
// a draft is dirty exactly when its content differs from the last disk state.

interface FileDraft {
  path: string;
  content: string;
  /** Last content loaded from disk or saved; dirtiness is measured against it. */
  baseline: string;
  loading: boolean;
}

type Listener = () => void;

const drafts = new Map<string, FileDraft>();
const listeners = new Set<Listener>();

/**
 * A version counter bumped on every change. `useSyncExternalStore` consumers
 * (TabStrip) pair `subscribe` with this as their snapshot so the strip
 * re-renders whenever a draft's dirtiness changes.
 */
let version = 0;

function notify(): void {
  version += 1;
  for (const listener of listeners) {
    listener();
  }
}

/** The snapshot version, bumped on every draft mutation. */
export function getVersion(): number {
  return version;
}

/** Seed (or reset) `path`'s draft from a fresh disk load. */
export function setFileDraft(path: string, content: string, baseline: string): void {
  drafts.set(path, { path, content, baseline, loading: false });
  notify();
}

/** Replace the draft's content (an editor change); no-op when unchanged. */
export function setFileContent(path: string, content: string): void {
  const current = drafts.get(path);
  if (!current || current.content === content) return;
  drafts.set(path, { ...current, content, loading: false });
  notify();
}

/** Record that the current content just landed on disk (dirty state clears). */
export function markFileSaved(path: string): void {
  const current = drafts.get(path);
  if (!current) return;
  drafts.set(path, { ...current, baseline: current.content, loading: false });
  notify();
}

/** Drop the draft (tab closed, content saved or discarded). */
export function clearFileDraft(path: string): void {
  if (!drafts.delete(path)) return;
  notify();
}

/** The draft for `path`, or `undefined` when none has been seeded. */
export function getFileDraft(path: string): FileDraft | undefined {
  return drafts.get(path);
}

/** True when the draft has content not yet on disk. */
export function isFileDirty(path: string): boolean {
  const draft = drafts.get(path);
  if (!draft) return false;
  return draft.content !== draft.baseline;
}

/** Subscribe to draft changes; returns an unsubscribe function. */
export function subscribe(cb: Listener): () => void {
  listeners.add(cb);
  return () => {
    listeners.delete(cb);
  };
}
