// In-memory drafts for open skill tabs.
//
// A skill tab names a skill but holds no content itself: its markdown lives in
// this module until the user saves, at which point the shell writes
// `<app_config_dir>/skills/<name>.md`. Keeping the drafts here (keyed by tab
// id) decouples the content from the layout and lets the save-on-close prompt
// in App.tsx read the dirtiness without reaching into components.
//
// Dirtiness is defined against `baseline` — the content last loaded from disk
// or saved. A draft whose content differs from its baseline is dirty (closing
// still prompts only when dirty). A brand-new skill never had a baseline, so
// its scaffold is dirty by construction until the first save.
//
// Loading is lazy and async: `ensureDraft` seeds a loading entry, resolves it
// through `readSkill`, and falls back to a scaffold when the file is missing,
// mirroring the store.ts pattern of stable references for
// `useSyncExternalStore` consumers.

import { readSkill } from "./commands";

interface Draft {
  name: string;
  content: string;
  /** Last content loaded from disk or saved; `null` means never saved. */
  baseline: string | null;
  loading: boolean;
}

type Listener = () => void;

const drafts = new Map<string, Draft>();
const listeners = new Set<Listener>();

/** Rooted scaffold for a brand-new skill (nothing on disk yet). */
export function scaffoldSkill(name: string): string {
  return `# ${name}\n\nDescribe...\n\n## Instructions\n\n- \n`;
}

function notify(): void {
  for (const listener of listeners) {
    listener();
  }
}

/**
 * Create `tabId`'s draft if missing and kick off its load. With `initial`
 * (a scaffold or imported content) the draft is seeded directly without
 * reading the disk. Safe to call repeatedly — later calls for an existing
 * draft are no-ops.
 */
export function ensureDraft(tabId: string, name: string, initial?: string): void {
  if (drafts.has(tabId)) return;
  if (initial !== undefined) {
    drafts.set(tabId, { name, content: initial, baseline: null, loading: false });
    notify();
    return;
  }
  const created: Draft = { name, content: "", baseline: null, loading: true };
  drafts.set(tabId, created);
  notify();
  void readSkill(name)
    .then((content) => {
      // Only land the disk content when the user has not already replaced the
      // draft (typed over it or saved) while the read was in flight.
      if (drafts.get(tabId) !== created) return;
      drafts.set(tabId, { ...created, content, baseline: content, loading: false });
      notify();
    })
    .catch(() => {
      if (drafts.get(tabId) !== created) return;
      // A missing skill file is how new skills are born: scaffold instead.
      drafts.set(tabId, { ...created, content: scaffoldSkill(name), baseline: null, loading: false });
      notify();
    });
}

/** The draft for `tabId`, or `undefined` when it has not been ensured yet. */
export function getDraft(tabId: string): Draft | undefined {
  return drafts.get(tabId);
}

/** Replace the draft's content and mark it dirty when it differs from baseline. */
export function setContent(tabId: string, content: string): void {
  const current = drafts.get(tabId);
  if (!current || current.content === content) return;
  drafts.set(tabId, { ...current, content, loading: false });
  notify();
}

/** Record that the current content just landed on disk (dirty state clears). */
export function markSaved(tabId: string): void {
  const current = drafts.get(tabId);
  if (!current) return;
  drafts.set(tabId, { ...current, baseline: current.content, loading: false });
  notify();
}

/** Drop the draft (tab closed, content saved or discarded). */
export function clearDraft(tabId: string): void {
  if (!drafts.delete(tabId)) return;
  notify();
}

/** True when the draft has content not yet on disk (nothing saved = dirty). */
export function isDirty(tabId: string): boolean {
  const draft = drafts.get(tabId);
  if (!draft) return false;
  return draft.baseline === null || draft.content !== draft.baseline;
}

/** Subscribe to draft changes; returns an unsubscribe function. */
export function subscribe(cb: Listener): () => void {
  listeners.add(cb);
  return () => {
    listeners.delete(cb);
  };
}