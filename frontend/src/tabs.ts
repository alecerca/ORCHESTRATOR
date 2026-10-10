// Pure tab/group model for the workspace centre.
//
// One tab = one document, unique across the whole workspace. A document is an
// agent `session` or a `file` (discriminated union kinds). The layout is a
// nested split tree (T3); leaves are groups holding tabs.
//
// Every helper is pure: it takes a `Layout` and returns a new one (or the same
// reference when nothing changed). Generation of ids is the only module state,
// kept purely so newly created nodes/groups never collide with restored ones.

/** A session-backed document. `id` is derived from the session id. */
export interface SessionTab {
  id: string;
  kind: "session";
  sessionId: string;
}

/** A file-backed document. `id` is derived from the path. */
export interface FileTab {
  id: string;
  kind: "file";
  path: string;
}

/** Discriminated union: agent histories and files are both documents. */
export type Tab = SessionTab | FileTab;

export interface GroupNode {
  id: string;
  kind: "group";
  tabs: Tab[];
  activeTabId: string | null;
}

export interface SplitNode {
  id: string;
  kind: "split";
  orientation: "horizontal" | "vertical";
  children: Node[];
  sizes: number[];
}

export type Node = GroupNode | SplitNode;

export interface Layout {
  root: Node | null;
  activeGroupId: string | null;
}

/** localStorage key for the persisted layout. */
export const LAYOUT_KEY = "orquestador.layout";

export const EMPTY_LAYOUT: Layout = { root: null, activeGroupId: null };

/** Globally unique tab id for a session document. */
function sessionTabKey(sessionId: string): string {
  return `session:${sessionId}`;
}

/** Globally unique tab id for a file document. */
export function fileTabKey(path: string): string {
  return `file:${path}`;
}

/** The session a tab points at, or `null` for non-session kinds. */
export function tabSessionId(tab: Tab): string | null {
  return tab.kind === "session" ? tab.sessionId : null;
}

/** The path a tab points at, or `null` for non-file kinds. */
export function tabPath(tab: Tab): string | null {
  return tab.kind === "file" ? tab.path : null;
}

// Id sequences; bumped on restore so new ids cannot collide with saved ones.
let groupSequence = 0;
let splitSequence = 0;

function nextGroupId(): string {
  groupSequence += 1;
  return `group-${groupSequence}`;
}

function nextSplitId(): string {
  splitSequence += 1;
  return `split-${splitSequence}`;
}

/** The tab a group should render: its active one, else its first. */
export function activeTabOf(group: GroupNode): Tab | undefined {
  return group.tabs.find((tab) => tab.id === group.activeTabId) ?? group.tabs[0];
}

/** True when any group holds a session tab for `sessionId`. */
export function hasSessionTab(layout: Layout, sessionId: string): boolean {
  return groupsInTree(layout.root).some((group) =>
    group.tabs.some((tab) => tab.kind === "session" && tab.sessionId === sessionId),
  );
}

/** Every session id that has a tab anywhere in the layout. */
export function sessionTabIds(layout: Layout): string[] {
  const ids: string[] = [];
  const seen = new Set<string>();
  for (const group of groupsInTree(layout.root)) {
    for (const tab of group.tabs) {
      if (tab.kind === "session") {
        if (!seen.has(tab.sessionId)) {
          seen.add(tab.sessionId);
          ids.push(tab.sessionId);
        }
      }
    }
  }
  return ids;
}

/** True when any group holds a tab for `path`. */
export function hasFileTab(layout: Layout, path: string): boolean {
  return groupsInTree(layout.root).some((group) =>
    group.tabs.some((tab) => tab.kind === "file" && tab.path === path),
  );
}

/**
 * Open a tab or focus the existing one for the same document. Never
 * duplicates: if the document already has a tab anywhere, that tab and its
 * group become active. Otherwise the tab opens in the active group, creating
 * the first group when none exists.
 */
function openTab(
  layout: Layout,
  tab: Tab,
  isSameDocument: (candidate: Tab) => boolean,
): Layout {
  for (const group of groupsInTree(layout.root)) {
    const existing = group.tabs.find(isSameDocument);
    if (existing) {
      if (group.activeTabId === existing.id && layout.activeGroupId === group.id) {
        return layout;
      }
      const root = updateGroup(layout.root, group.id, (g) => ({ ...g, activeTabId: existing.id }));
      return { root, activeGroupId: group.id };
    }
  }

  if (layout.root === null) {
    const group: GroupNode = { id: nextGroupId(), kind: "group", tabs: [tab], activeTabId: tab.id };
    return { root: group, activeGroupId: group.id };
  }

  const target = layout.activeGroupId
    ? groupById(layout.root, layout.activeGroupId)
    : (groupsInTree(layout.root)[0] ?? null);

  if (!target) {
    const groups = groupsInTree(layout.root);
    const fallback = groups[0];
    if (!fallback) {
      const group: GroupNode = { id: nextGroupId(), kind: "group", tabs: [tab], activeTabId: tab.id };
      return { root: group, activeGroupId: group.id };
    }
    const root = updateGroup(layout.root, fallback.id, (g) => ({
      ...g,
      tabs: [...g.tabs, tab],
      activeTabId: tab.id,
    }));
    return { root, activeGroupId: fallback.id };
  }

  const root = updateGroup(layout.root, target.id, (g) => ({
    ...g,
    tabs: [...g.tabs, tab],
    activeTabId: tab.id,
  }));
  return { root, activeGroupId: target.id };
}

/**
 * Open a session tab or focus the existing one. Never duplicates: if the
 * session already has a tab anywhere, that tab and its group become active.
 * Otherwise the tab opens in the active group, creating the first group when
 * none exists.
 */
export function openSessionTab(layout: Layout, sessionId: string): Layout {
  return openTab(
    layout,
    { id: sessionTabKey(sessionId), kind: "session", sessionId },
    (tab) => tab.kind === "session" && tab.sessionId === sessionId,
  );
}

/**
 * Open a file tab or focus the existing one. Never duplicates: if the path
 * already has a tab anywhere, that tab and its group become active. Otherwise
 * the tab opens in the active group, creating the first group when none exists.
 */
export function openFileTab(layout: Layout, path: string): Layout {
  return openTab(
    layout,
    { id: fileTabKey(path), kind: "file", path },
    (tab) => tab.kind === "file" && tab.path === path,
  );
}

/** Path of the file tab active in the active group, or `null`. */
export function activeFilePath(layout: Layout): string | null {
  const activeGroup =
    (layout.activeGroupId ? groupById(layout.root, layout.activeGroupId) : null) ??
    groupsInTree(layout.root)[0];
  const tab = activeGroup ? activeTabOf(activeGroup) : undefined;
  return tab ? tabPath(tab) : null;
}

/** Activate a tab and its group. */
export function activateTab(layout: Layout, groupId: string, tabId: string): Layout {
  const group = groupById(layout.root, groupId);
  if (!group || !group.tabs.some((tab) => tab.id === tabId)) return layout;
  if (group.activeTabId === tabId && layout.activeGroupId === groupId) return layout;
  const root = updateGroup(layout.root, groupId, (g) => ({ ...g, activeTabId: tabId }));
  return { root, activeGroupId: groupId };
}

/** Make a group the active one (where new tabs open). */
export function activateGroup(layout: Layout, groupId: string): Layout {
  if (layout.activeGroupId === groupId) return layout;
  if (!groupById(layout.root, groupId)) return layout;
  return { root: layout.root, activeGroupId: groupId };
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
  return { root: null, activeGroupId: null };
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
  predicate: (tab: Tab, group: GroupNode) => boolean,
  group: GroupNode,
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

/**
 * Remove every tab matching `predicate`. When the active tab goes, a neighbour
 * is activated (prefer the tab to the right, else the left); when a group loses
 * its last tab, the group collapses and focus moves to another group.
 */
function removeTabsWhere(
  layout: Layout,
  predicate: (tab: Tab, group: GroupNode) => boolean,
): Layout {
  if (layout.root === null) return layout;
  let changed = false;
  const { root: nextRoot, removedGroups } = removeTabsFromNode(
    layout.root,
    predicate,
    () => {
      changed = true;
    },
  );
  if (!changed) return layout;
  const survivors = groupsInTree(nextRoot);
  let activeGroupId = layout.activeGroupId;
  if (!activeGroupId || !groupById(nextRoot, activeGroupId)) {
    const oldGroups = groupsInTree(layout.root).filter((g) => !removedGroups.has(g.id));
    const index = activeGroupId
      ? oldGroups.findIndex((g) => g.id === activeGroupId)
      : -1;
    let candidate: GroupNode | null = null;
    for (let i = index + 1; i < oldGroups.length; i += 1) {
      candidate = groupById(nextRoot, oldGroups[i].id) ?? candidate;
    }
    for (let i = index - 1; i >= 0 && !candidate; i -= 1) {
      candidate = groupById(nextRoot, oldGroups[i].id) ?? null;
    }
    activeGroupId = candidate?.id ?? survivors[0]?.id ?? null;
  }
  return { root: nextRoot, activeGroupId };
}

interface RemoveResult {
  root: Node | null;
  removedGroups: Set<string>;
}

function removeTabsFromNode(
  node: Node,
  predicate: (tab: Tab, group: GroupNode) => boolean,
  markChanged: () => void,
): RemoveResult {
  if (node.kind === "group") {
    const kept = node.tabs.filter((tab) => !predicate(tab, node));
    if (kept.length === node.tabs.length) {
      return { root: node, removedGroups: new Set() };
    }
    markChanged();
    if (kept.length === 0) {
      return { root: null, removedGroups: new Set([node.id]) };
    }
    let activeTabId = node.activeTabId;
    if (!activeTabId || !kept.some((tab) => tab.id === activeTabId)) {
      activeTabId = neighbourTabId(node.tabs, node.activeTabId, predicate, node) ?? kept[0].id;
    }
    return {
      root: { ...node, tabs: kept, activeTabId },
      removedGroups: new Set(),
    };
  }

  const children: Node[] = [];
  const removedGroups = new Set<string>();
  for (const child of node.children) {
    const res = removeTabsFromNode(child, predicate, markChanged);
    if (res.root) {
      children.push(res.root);
    }
    for (const id of res.removedGroups) removedGroups.add(id);
  }
  if (children.length === node.children.length && children.every((c, i) => c === node.children[i])) {
    return { root: node, removedGroups };
  }
  if (children.length < 2) {
    const collapsed = children.length === 1 ? children[0] : null;
    return { root: collapsed, removedGroups };
  }
  const sizes = normalizeSizes(node.sizes, children.length);
  return {
    root: { ...node, children, sizes },
    removedGroups,
  };
}

// -- tree utilities ----------------------------------------------------------

export function groupsInTree(root: Node | null): GroupNode[] {
  if (!root) return [];
  const result: GroupNode[] = [];
  walkGroups(root, (g) => result.push(g));
  return result;
}

function walkGroups(node: Node, fn: (g: GroupNode) => void): void {
  if (node.kind === "group") {
    fn(node);
    return;
  }
  for (const child of node.children) {
    walkGroups(child, fn);
  }
}

function groupById(root: Node | null, id: string): GroupNode | null {
  if (!root) return null;
  let found: GroupNode | null = null;
  walkGroups(root, (g) => {
    if (g.id === id) found = g;
  });
  return found;
}

function nodeById(root: Node | null, id: string): Node | null {
  if (!root || root.id === id) return root;
  if (root.kind === "group") return null;
  for (const child of root.children) {
    const n = nodeById(child, id);
    if (n) return n;
  }
  return null;
}

function findParentSplit(root: Node | null, childId: string): { split: SplitNode; index: number } | null {
  if (!root || root.kind === "group") return null;
  for (let i = 0; i < root.children.length; i += 1) {
    if (root.children[i].id === childId) return { split: root, index: i };
    if (root.children[i].kind === "split") {
      const p = findParentSplit(root.children[i], childId);
      if (p) return p;
    }
  }
  return null;
}

function updateGroup(root: Node | null, groupId: string, fn: (g: GroupNode) => GroupNode): Node | null {
  if (!root) return root;
  if (root.kind === "group") {
    if (root.id === groupId) return fn(root);
    return root;
  }
  let changed = false;
  const children: Node[] = root.children.map((child) => {
    const next = updateGroup(child, groupId, fn);
    if (next !== child) changed = true;
    return next === null ? child : next;
  });
  if (!changed) return root;
  return { ...root, children };
}

function replaceNode(root: Node | null, targetId: string, replacement: Node | null): Node | null {
  if (!root) return root;
  if (root.id === targetId) return replacement;
  if (root.kind === "group") return root;
  let changed = false;
  const children: Node[] = [];
  for (const child of root.children) {
    if (child.id === targetId) {
      if (replacement) children.push(replacement);
      changed = true;
      continue;
    }
    const next = replaceNode(child, targetId, replacement);
    if (next !== child) changed = true;
    if (next !== null) children.push(next);
  }
  if (!changed) return root;
  if (children.length < 2) return children.length === 1 ? children[0] : null;
  return { ...root, children, sizes: normalizeSizes(root.sizes, children.length) };
}

function normalizeSizes(sizes: number[] | undefined, count: number): number[] {
  if (count <= 0) return [];
  const out = Array(count).fill(1 / count);
  if (!sizes || sizes.length !== count) return out;
  let sum = 0;
  for (let i = 0; i < count; i += 1) {
    const v = Number(sizes[i]);
    out[i] = Number.isFinite(v) && v > 0 ? v : 1 / count;
    sum += out[i];
  }
  if (sum <= 0) return Array(count).fill(1 / count);
  for (let i = 0; i < count; i += 1) out[i] = out[i] / sum;
  return out;
}

function splitSizes(): number[] {
  return [0.5, 0.5];
}

function clampSizes(sizes: number[], count: number): number[] {
  if (count <= 1) return normalizeSizes(sizes, count);
  const out = Array(count).fill(1 / count);
  if (!sizes || sizes.length !== count) return out;
  let sum = 0;
  for (let i = 0; i < count; i += 1) {
    const v = Number(sizes[i]);
    out[i] = Number.isFinite(v) && v > 0 ? Math.max(0.01, v) : 1 / count;
    sum += out[i];
  }
  if (sum <= 0) return Array(count).fill(1 / count);
  for (let i = 0; i < count; i += 1) out[i] = out[i] / sum;
  return out;
}

// -- split/move helpers ------------------------------------------------------

/** Walk all groups in tree order. */
export function walkAllGroups(layout: Layout, fn: (g: GroupNode) => void): void {
  if (layout.root) walkGroups(layout.root, fn);
}

/** Find a group and its parent split. */
export function findGroupAndParent(layout: Layout, groupId: string): {
  group: GroupNode;
  parent: SplitNode | null;
  index: number;
} | null {
  const group = groupById(layout.root, groupId);
  if (!group) return null;
  const p = layout.root ? findParentSplit(layout.root, groupId) : null;
  if (p) return { group, parent: p.split, index: p.index };
  return { group, parent: null, index: -1 };
}

export function splitGroup(
  layout: Layout,
  groupId: string,
  direction: "right" | "down",
  tabId?: string,
): Layout {
  const group = groupById(layout.root, groupId);
  if (!group) return layout;
  const activeTab =
    (tabId ? group.tabs.find((t) => t.id === tabId) : undefined) ??
    group.tabs.find((t) => t.id === group.activeTabId) ??
    group.tabs[0];
  if (!activeTab) return layout;

  const newGroup: GroupNode = {
    id: nextGroupId(),
    kind: "group",
    tabs: [activeTab],
    activeTabId: activeTab.id,
  };
  const otherTabs = group.tabs.filter((t) => t.id !== activeTab.id);
  const updatedGroup: GroupNode = {
    ...group,
    tabs: otherTabs,
    activeTabId:
      otherTabs.length > 0
        ? (otherTabs.find((t) => t.id === group.activeTabId)?.id ?? otherTabs[0].id)
        : null,
  };

  const orientation = direction === "right" ? "horizontal" : "vertical";

  const root = layout.root;
  if (!root) {
    const split: SplitNode = {
      id: nextSplitId(),
      kind: "split",
      orientation,
      children: [updatedGroup, newGroup],
      sizes: splitSizes(),
    };
    return { root: split, activeGroupId: newGroup.id };
  }

  const parent = findParentSplit(root, groupId);
  if (parent && parent.split.orientation === orientation) {
    // Insert into the existing same-orientation split without wiping the other
    // panes' sizes: the new group takes half of the split group's own former
    // weight and the split group keeps the other half.
    const children = [...parent.split.children];
    const sizes = normalizeSizes(parent.split.sizes, parent.split.children.length);
    const half = sizes[parent.index] / 2;
    sizes[parent.index] = half;
    const at = parent.index + 1;
    children.splice(at, 0, newGroup);
    sizes.splice(at, 0, half);
    const split: SplitNode = {
      ...parent.split,
      children,
      sizes: normalizeSizes(sizes, children.length),
    };
    const newRoot = replaceNode(root, parent.split.id, split);
    const finalRoot = replaceNode(newRoot, groupId, updatedGroup);
    return { root: finalRoot, activeGroupId: newGroup.id };
  }

  // Different orientation or the group sits at the root: wrap it so the two
  // resulting siblings each take half of the group's former space.
  const wrapper: SplitNode = {
    id: nextSplitId(),
    kind: "split",
    orientation,
    children: [updatedGroup, newGroup],
    sizes: splitSizes(),
  };
  const newRoot = replaceNode(root, groupId, wrapper);
  return { root: newRoot, activeGroupId: newGroup.id };
}

export function moveTab(
  layout: Layout,
  sourceGroupId: string,
  tabId: string,
  targetGroupId: string,
  targetSide: "left" | "right" | "top" | "bottom",
): Layout {
  const sourceGroup = groupById(layout.root, sourceGroupId);
  if (!sourceGroup) return layout;
  const tab = sourceGroup.tabs.find((t) => t.id === tabId);
  if (!tab) return layout;

  const orientation =
    targetSide === "right" || targetSide === "left" ? "horizontal" : "vertical";
  const insertAfter = targetSide === "right" || targetSide === "bottom";
  const newGroup: GroupNode = {
    id: nextGroupId(),
    kind: "group",
    tabs: [tab],
    activeTabId: tab.id,
  };

  const root = layout.root;
  if (!root) return { root: newGroup, activeGroupId: newGroup.id };

  // `targetSide` is measured against the drop target's rect, so the new group
  // must be anchored to the HOVERED group, not to the group the tab came from.
  const targetGroup = groupById(root, targetGroupId);
  if (!targetGroup) return layout;

  // Insert the new group beside the target group first, then drop the tab from
  // the source. Removing last keeps a stable anchor when the source holds a
  // single tab and would collapse out from under the insert.
  const parent = findParentSplit(root, targetGroupId);
  let inserted: Node | null;
  if (parent && parent.split.orientation === orientation) {
    const children = [...parent.split.children];
    const sizes = normalizeSizes(parent.split.sizes, parent.split.children.length);
    const half = sizes[parent.index] / 2;
    sizes[parent.index] = half;
    const at = parent.index + (insertAfter ? 1 : 0);
    children.splice(at, 0, newGroup);
    sizes.splice(at, 0, half);
    const split: SplitNode = {
      ...parent.split,
      children,
      sizes: normalizeSizes(sizes, children.length),
    };
    inserted = replaceNode(root, parent.split.id, split);
  } else {
    const wrapper: SplitNode = {
      id: nextSplitId(),
      kind: "split",
      orientation,
      children: insertAfter ? [targetGroup, newGroup] : [newGroup, targetGroup],
      sizes: splitSizes(),
    };
    inserted = replaceNode(root, targetGroupId, wrapper);
  }
  if (!inserted) return layout;

  const moved = removeTabsWhere(
    { root: inserted, activeGroupId: newGroup.id },
    (x, g) => g.id === sourceGroupId && x.id === tabId,
  );
  return { root: moved.root, activeGroupId: moved.activeGroupId ?? newGroup.id };
}

export function updateSplitSizes(layout: Layout, splitId: string, sizes: number[]): Layout {
  const node = nodeById(layout.root, splitId);
  if (!node || node.kind !== "split") return layout;
  const clamped = clampSizes(sizes, node.children.length);
  const updated: SplitNode = { ...node, sizes: clamped };
  const root = replaceNode(layout.root, splitId, updated);
  return { root, activeGroupId: layout.activeGroupId };
}

// -- persistence -------------------------------------------------------------

function isTab(value: unknown): value is Tab {
  if (!value || typeof value !== "object") return false;
  const tab = value as Record<string, unknown>;
  if (typeof tab.id !== "string") return false;
  if (tab.kind === "session") return typeof tab.sessionId === "string";
  if (tab.kind === "file") return typeof tab.path === "string";
  return false;
}

function isGroup(value: unknown): value is GroupNode {
  if (!value || typeof value !== "object") return false;
  const group = value as Record<string, unknown>;
  if (group.kind !== "group") return false;
  if (typeof group.id !== "string") return false;
  if (!Array.isArray(group.tabs) || !group.tabs.every(isTab)) return false;
  if (group.activeTabId !== null && typeof group.activeTabId !== "string") return false;
  return true;
}

function isSplit(value: unknown): value is SplitNode {
  if (!value || typeof value !== "object") return false;
  const split = value as Record<string, unknown>;
  if (split.kind !== "split") return false;
  if (typeof split.id !== "string") return false;
  if (split.orientation !== "horizontal" && split.orientation !== "vertical") return false;
  if (!Array.isArray(split.children) || !split.children.every(isNode)) return false;
  if (!Array.isArray(split.sizes)) return false;
  return true;
}

function isNode(value: unknown): value is Node {
  if (!value || typeof value !== "object") return false;
  const v = value as Record<string, unknown>;
  if (v.kind === "group") return isGroup(value);
  if (v.kind === "split") return isSplit(value);
  return false;
}

function isLayoutT2(value: unknown): boolean {
  if (!value || typeof value !== "object") return false;
  const layout = value as Record<string, unknown>;
  return Array.isArray(layout.groups);
}

function isLayout(value: unknown): value is Layout {
  if (!value || typeof value !== "object") return false;
  const layout = value as Record<string, unknown>;
  if (layout.root !== null && !isNode(layout.root)) return false;
  if (layout.activeGroupId !== null && typeof layout.activeGroupId !== "string") return false;
  return true;
}

function normalizeTab(tab: Tab): Tab {
  if (tab.kind === "session") {
    return { id: sessionTabKey(tab.sessionId), kind: "session", sessionId: tab.sessionId };
  }
  return { id: fileTabKey(tab.path), kind: "file", path: tab.path };
}

function sanitizeGroup(group: GroupNode): GroupNode | null {
  const tabs: Tab[] = [];
  const seen = new Set<string>();
  for (const raw of group.tabs) {
    const tab = normalizeTab(raw);
    if (seen.has(tab.id)) continue;
    seen.add(tab.id);
    tabs.push(tab);
  }
  if (tabs.length === 0) return null;
  let activeTabId = group.activeTabId;
  if (!activeTabId || !tabs.some((tab) => tab.id === activeTabId)) {
    activeTabId = tabs[0].id;
  }
  return { ...group, tabs, activeTabId };
}

function sanitizeNode(node: Node): Node | null {
  if (node.kind === "group") return sanitizeGroup(node);
  const children: Node[] = [];
  for (const child of node.children) {
    const s = sanitizeNode(child);
    if (s) children.push(s);
  }
  if (children.length < 2) {
    return children.length === 1 ? children[0] : null;
  }
  const sizes = normalizeSizes(node.sizes, children.length);
  return { ...node, children, sizes };
}

function sanitizeLayout(layout: Layout): Layout {
  const root = layout.root ? sanitizeNode(layout.root) : null;
  let activeGroupId = layout.activeGroupId;
  const groups = groupsInTree(root);
  if (!activeGroupId || !groups.some((g) => g.id === activeGroupId)) {
    activeGroupId = groups.length > 0 ? groups[0].id : null;
  }
  return { root, activeGroupId };
}

function migrateT2(raw: any): Layout | null {
  const groups = Array.isArray(raw.groups) ? raw.groups : [];
  const sanitizedGroups: GroupNode[] = [];
  const seenTabs = new Set<string>();
  for (const g of groups) {
    if (!g || typeof g !== "object") continue;
    const group: GroupNode = {
      id: typeof g.id === "string" ? g.id : nextGroupId(),
      kind: "group",
      tabs: [],
      activeTabId: g.activeTabId === null ? null : (typeof g.activeTabId === "string" ? g.activeTabId : null),
    };
    if (Array.isArray(g.tabs)) {
      for (const t of g.tabs) {
        if (!isTab(t)) continue;
        const nt = normalizeTab(t);
        if (seenTabs.has(nt.id)) continue;
        seenTabs.add(nt.id);
        group.tabs.push(nt);
      }
    }
    const sg = sanitizeGroup(group);
    if (sg) sanitizedGroups.push(sg);
  }
  if (sanitizedGroups.length === 0) return null;
  const root: SplitNode = {
    id: nextSplitId(),
    kind: "split",
    orientation: "horizontal",
    children: sanitizedGroups,
    sizes: normalizeSizes(undefined, sanitizedGroups.length),
  };
  let activeGroupId = raw.activeGroupId;
  if (!activeGroupId || !sanitizedGroups.some((g) => g.id === activeGroupId)) {
    activeGroupId = sanitizedGroups[0].id;
  }
  return { root, activeGroupId };
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
  if (isLayout(parsed)) {
    for (const group of groupsInTree(parsed.root)) {
      const match = /^group-(\d+)$/.exec(group.id);
      if (match) groupSequence = Math.max(groupSequence, Number(match[1]));
    }
    const groups = groupsInTree(parsed.root);
    for (const group of groups) {
      const match = /^group-(\d+)$/.exec(group.id);
      if (match) groupSequence = Math.max(groupSequence, Number(match[1]));
    }
    // also count splits
    const walkSplits = (n: Node | null) => {
      if (!n || n.kind === "group") return;
      const m = /^split-(\d+)$/.exec(n.id);
      if (m) splitSequence = Math.max(splitSequence, Number(m[1]));
      for (const c of n.children) walkSplits(c);
    };
    walkSplits(parsed.root);
    return sanitizeLayout(parsed);
  }
  if (isLayoutT2(parsed)) {
    const migrated = migrateT2(parsed);
    if (!migrated) return null;
    for (const g of groupsInTree(migrated.root)) {
      const m = /^group-(\d+)$/.exec(g.id);
      if (m) groupSequence = Math.max(groupSequence, Number(m[1]));
    }
    const walkSplits = (n: Node | null) => {
      if (!n || n.kind === "group") return;
      const m2 = /^split-(\d+)$/.exec(n.id);
      if (m2) splitSequence = Math.max(splitSequence, Number(m2[1]));
      for (const c of n.children) walkSplits(c);
    };
    walkSplits(migrated.root);
    return sanitizeLayout(migrated);
  }
  return null;
}
