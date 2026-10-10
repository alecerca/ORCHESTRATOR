// Pure tab/group model for the workspace centre.
//
// One tab = one document, unique across the whole workspace. A document is a
// `session` today; T4 adds a `file` kind to the same union without reshaping the
// model. `TabGroup`s sit side by side (VS Code pure); T3 adds the split gestures.
//
// Every helper is pure: it takes a `Layout` and returns a new one (or the same
// reference when nothing changed). Generation of group ids is the only module
// state, kept purely so newly created groups never collide with restored ones.

/** A session-backed document. `id` is derived from the session id. */
export interface SessionTab {
  id: string;
  kind: "session";
  sessionId: string;
}

/** Discriminated union so `{ kind: "file" }` can be added later. */
export type Tab = SessionTab;

export interface TabGroup {
  id: string;
  tabs: Tab[];
  activeTabId: string | null;
}

export interface Layout {
  groups: TabGroup[];
  activeGroupId: string | null;
}

/** localStorage key for the persisted layout. */
export const LAYOUT_KEY = "orquestador.layout";

export const EMPTY_LAYOUT: Layout = { groups: [], activeGroupId: null };

/** Globally unique tab id for a session document. */
function sessionTabKey(sessionId: string): string {
  return `session:${sessionId}`;
}

/** The session a tab points at, or `null` for non-session kinds (T4). */
export function tabSessionId(tab: Tab): string | null {
  return tab.kind === "session" ? tab.sessionId : null;
}

// Group-id sequence; bumped on restore so new ids cannot collide with saved ones.
let groupSequence = 0;

function nextGroupId(): string {
  groupSequence += 1;
  return `group-${groupSequence}`;
}

/** The tab a group should render: its active one, else its first. */
export function activeTabOf(group: TabGroup): Tab | undefined {
  return group.tabs.find((tab) => tab.id === group.activeTabId) ?? group.tabs[0];
}

/** True when any group holds a session tab for `sessionId`. */
export function hasSessionTab(layout: Layout, sessionId: string): boolean {
  return layout.groups.some((group) =>
    group.tabs.some((tab) => tab.kind === "session" && tab.sessionId === sessionId),
  );
}

/** Every session id that has a tab anywhere in the layout. */
export function sessionTabIds(layout: Layout): string[] {
  const ids: string[] = [];
  for (const group of layout.groups) {
    for (const tab of group.tabs) {
      if (tab.kind === "session") ids.push(tab.sessionId);
    }
  }
  return ids;
}

/**
 * Open a session tab or focus the existing one. Never duplicates: if the
 * session already has a tab anywhere, that tab and its group become active.
 * Otherwise the tab opens in the active group, creating the first group when
 * none exists.
 */
export function openSessionTab(layout: Layout, sessionId: string): Layout {
  for (const group of layout.groups) {
    const existing = group.tabs.find(
      (tab) => tab.kind === "session" && tab.sessionId === sessionId,
    );
    if (existing) {
      if (group.activeTabId === existing.id && layout.activeGroupId === group.id) {
        return layout;
      }
      return {
        groups: layout.groups.map((entry) =>
          entry.id === group.id ? { ...entry, activeTabId: existing.id } : entry,
        ),
        activeGroupId: group.id,
      };
    }
  }

  const tab: Tab = { id: sessionTabKey(sessionId), kind: "session", sessionId };
  const target =
    layout.groups.find((group) => group.id === layout.activeGroupId) ?? layout.groups[0];

  if (!target) {
    const group: TabGroup = { id: nextGroupId(), tabs: [tab], activeTabId: tab.id };
    return { groups: [group], activeGroupId: group.id };
  }

  return {
    groups: layout.groups.map((group) =>
      group.id === target.id
        ? { ...group, tabs: [...group.tabs, tab], activeTabId: tab.id }
        : group,
    ),
    activeGroupId: target.id,
  };
}

/** Activate a tab and its group. */
export function activateTab(layout: Layout, groupId: string, tabId: string): Layout {
  const group = layout.groups.find((entry) => entry.id === groupId);
  if (!group || !group.tabs.some((tab) => tab.id === tabId)) return layout;
  if (group.activeTabId === tabId && layout.activeGroupId === groupId) return layout;
  return {
    groups: layout.groups.map((entry) =>
      entry.id === groupId ? { ...entry, activeTabId: tabId } : entry,
    ),
    activeGroupId: groupId,
  };
}

/** Make a group the active one (where new tabs open). */
export function activateGroup(layout: Layout, groupId: string): Layout {
  if (layout.activeGroupId === groupId) return layout;
  if (!layout.groups.some((group) => group.id === groupId)) return layout;
  return { ...layout, activeGroupId: groupId };
}

/** Close one tab in a group. */
export function closeTab(layout: Layout, groupId: string, tabId: string): Layout {
  return removeTabsWhere(
    layout,
    (tab, group) => group.id === groupId && tab.id === tabId,
  );
}

/** Close every tab (in every group) belonging to a session. */
export function closeSessionTabs(layout: Layout, sessionId: string): Layout {
  return removeTabsWhere(
    layout,
    (tab) => tab.kind === "session" && tab.sessionId === sessionId,
  );
}

/** Close every tab and drop every group. */
export function closeAllTabs(): Layout {
  return { groups: [], activeGroupId: null };
}

/** Drop every tab whose session id is not in `existingSessionIds`. */
export function pruneSessionTabs(
  layout: Layout,
  existingSessionIds: ReadonlySet<string>,
): Layout {
  return removeTabsWhere(
    layout,
    (tab) => tab.kind === "session" && !existingSessionIds.has(tab.sessionId),
  );
}

// -- internals ---------------------------------------------------------------

function neighbourTabId(
  tabs: Tab[],
  activeId: string | null,
  predicate: (tab: Tab, group: TabGroup) => boolean,
  group: TabGroup,
): string | null {
  const index = activeId ? tabs.findIndex((tab) => tab.id === activeId) : -1;
  if (index < 0) return null;
  for (let i = index + 1; i < tabs.length; i += 1) {
    if (!predicate(tabs[i], group)) return tabs[i].id;
  }
  for (let i = index - 1; i >= 0; i -= 1) {
    if (!predicate(tabs[i], group)) return tabs[i].id;
  }
  return null;
}

function reconcileActiveGroup(
  original: TabGroup[],
  survivors: TabGroup[],
  activeGroupId: string | null,
): string | null {
  if (survivors.length === 0) return null;
  if (activeGroupId && survivors.some((group) => group.id === activeGroupId)) {
    return activeGroupId;
  }
  const index = activeGroupId ? original.findIndex((group) => group.id === activeGroupId) : -1;
  for (let i = index + 1; i < original.length; i += 1) {
    if (survivors.some((group) => group.id === original[i].id)) return original[i].id;
  }
  for (let i = index - 1; i >= 0; i -= 1) {
    if (survivors.some((group) => group.id === original[i].id)) return original[i].id;
  }
  return survivors[0].id;
}

/**
 * Remove every tab matching `predicate`. When the active tab goes, a neighbour
 * is activated (prefer the tab to the right, else the left); when a group loses
 * its last tab, the group collapses and focus moves to another group.
 */
function removeTabsWhere(
  layout: Layout,
  predicate: (tab: Tab, group: TabGroup) => boolean,
): Layout {
  let changed = false;
  const survivors: TabGroup[] = [];

  for (const group of layout.groups) {
    const kept = group.tabs.filter((tab) => !predicate(tab, group));
    if (kept.length === group.tabs.length) {
      survivors.push(group);
      continue;
    }
    changed = true;
    if (kept.length === 0) continue;
    let activeTabId = group.activeTabId;
    if (!activeTabId || !kept.some((tab) => tab.id === activeTabId)) {
      activeTabId =
        neighbourTabId(group.tabs, group.activeTabId, predicate, group) ?? kept[0].id;
    }
    survivors.push({ ...group, tabs: kept, activeTabId });
  }

  if (!changed) return layout;
  return {
    groups: survivors,
    activeGroupId: reconcileActiveGroup(layout.groups, survivors, layout.activeGroupId),
  };
}

// -- persistence -------------------------------------------------------------

function isTab(value: unknown): value is Tab {
  if (!value || typeof value !== "object") return false;
  const tab = value as Record<string, unknown>;
  if (typeof tab.id !== "string") return false;
  if (tab.kind === "session") return typeof tab.sessionId === "string";
  return false;
}

function isGroup(value: unknown): value is TabGroup {
  if (!value || typeof value !== "object") return false;
  const group = value as Record<string, unknown>;
  if (typeof group.id !== "string") return false;
  if (!Array.isArray(group.tabs) || !group.tabs.every(isTab)) return false;
  if (group.activeTabId !== null && typeof group.activeTabId !== "string") return false;
  return true;
}

function isLayout(value: unknown): value is Layout {
  if (!value || typeof value !== "object") return false;
  const layout = value as Record<string, unknown>;
  if (!Array.isArray(layout.groups) || !layout.groups.every(isGroup)) return false;
  if (layout.activeGroupId !== null && typeof layout.activeGroupId !== "string") return false;
  return true;
}

function normalizeTab(tab: Tab): Tab {
  return { id: sessionTabKey(tab.sessionId), kind: "session", sessionId: tab.sessionId };
}

/**
 * Repair invariants from persisted/corrupt input: normalize tab ids, drop
 * duplicate documents, drop empty groups and repoint the active tab/group.
 */
function sanitizeLayout(layout: Layout): Layout {
  const seen = new Set<string>();
  const groups: TabGroup[] = [];

  for (const group of layout.groups) {
    const tabs: Tab[] = [];
    for (const raw of group.tabs) {
      const tab = normalizeTab(raw);
      if (seen.has(tab.id)) continue;
      seen.add(tab.id);
      tabs.push(tab);
    }
    if (tabs.length === 0) continue;
    let activeTabId = group.activeTabId;
    if (!activeTabId || !tabs.some((tab) => tab.id === activeTabId)) {
      activeTabId = tabs[0].id;
    }
    groups.push({ id: group.id, tabs, activeTabId });
  }

  let activeGroupId = layout.activeGroupId;
  if (!activeGroupId || !groups.some((group) => group.id === activeGroupId)) {
    activeGroupId = groups.length > 0 ? groups[0].id : null;
  }
  return { groups, activeGroupId };
}

/**
 * Parse a persisted layout. Returns `null` for missing or malformed input so the
 * caller can fall back to the bootstrap tab.
 */
export function parseLayout(raw: string | null): Layout | null {
  if (!raw) return null;
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return null;
  }
  if (!isLayout(parsed)) return null;

  for (const group of parsed.groups) {
    const match = /^group-(\d+)$/.exec(group.id);
    if (match) groupSequence = Math.max(groupSequence, Number(match[1]));
  }
  return sanitizeLayout(parsed);
}
