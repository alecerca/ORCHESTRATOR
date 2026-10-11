import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import * as api from "./commands";
import { store, useAppState } from "./store";
import {
  CORE_EVENT,
  type AppStateSnapshot,
  type CoreMessage,
  type Effect,
  type ServerConfig,
  type SessionSnapshot,
} from "./types";
import { describeError } from "./utils";
import { setDirectory } from "./serverConfig";
import { TopBar } from "./components/TopBar";
import { ActivityBar } from "./components/ActivityBar";
import { AgentPanel } from "./components/AgentPanel";
import { PanelResizer } from "./components/PanelResizer";
import { PaneGrid } from "./components/PaneGrid";
import { Composer } from "./components/Composer";
import { ApprovalDialog } from "./components/ApprovalDialog";
import { NotificationsDrawer } from "./components/NotificationsDrawer";
import { ConnectionBanner } from "./components/ConnectionBanner";
import { EditorPanel } from "./components/EditorPanel";
import { SkillSavePrompt } from "./components/SkillSavePrompt";
import { SettingsDialog } from "./components/SettingsDialog";
import {
  EMPTY_LAYOUT,
  LAYOUT_KEY,
  activateGroup,
  activateTab,
  activeFilePath,
  closeAllTabs,
  closeSessionTabs,
  closeTab,
  getTab,
  hasSessionTab,
  openFileTab,
  openSessionTab,
  openSkillTab,
  parseLayout,
  pruneSessionTabs,
  sessionTabIds,
  splitGroup,
  updateSplitSizes,
  moveTab,
  type Layout,
} from "./tabs";
import { clearDraft, getDraft, isDirty, markSaved } from "./skillDrafts";

type Theme = "dark" | "light";

const THEME_KEY = "orquestador.theme";
const DISMISSED_KEY = "orquestador.dismissed";
const ROOT_PICK_KEY = "orquestador.rootPick";
const AGENT_PANEL_KEY = "orquestador.agentPanelWidth";
const EDITOR_PANEL_KEY = "orquestador.editorPanelWidth";
const AGENT_PANEL_MIN = 180;
const AGENT_PANEL_MAX = 560;
const AGENT_PANEL_DEFAULT = 270;
const EDITOR_PANEL_MIN = 280;
const EDITOR_PANEL_MAX = 720;
const EDITOR_PANEL_DEFAULT = 420;
const EFFECT_LIMIT = 20;

/** Restore a persisted panel width, clamped to its bounds; fall back when unset/invalid. */
function loadWidth(key: string, fallback: number, min: number, max: number): number {
  try {
    const raw = localStorage.getItem(key);
    if (raw === null) return fallback;
    const parsed = Number(raw);
    if (!Number.isFinite(parsed)) return fallback;
    return Math.min(Math.max(parsed, min), max);
  } catch {
    return fallback;
  }
}

function initialTheme(): Theme {
  try {
    const saved = localStorage.getItem(THEME_KEY);
    if (saved === "dark" || saved === "light") return saved;
  } catch {
    // localStorage unavailable — fall through to system preference.
  }
  return window.matchMedia("(prefers-color-scheme: light)").matches
    ? "light"
    : "dark";
}

/** Sessions without a parent are candidate orchestrator roots (see types.ts). */
function parentlessSessions(snapshot: AppStateSnapshot): SessionSnapshot[] {
  return snapshot.sessions.filter((entry) => !entry.session.parentID);
}

/**
 * Effective orchestrator root: the persisted pick when it still exists as a
 * parentless session, otherwise the snapshot's own `root_session_id`. Core
 * `pick_root` is untouched — this only overrides what the UI treats as root.
 */
function resolveEffectiveRoot(
  snapshot: AppStateSnapshot,
  pick: string | null,
): string | null {
  if (pick) {
    const picked = snapshot.sessions.find((entry) => entry.session.id === pick);
    if (picked && !picked.session.parentID) return pick;
  }
  return snapshot.root_session_id ?? null;
}

/** Dismissed rows are UI-local: the sessions stay alive on the server. */
function loadDismissed(): Set<string> {
  try {
    const raw = localStorage.getItem(DISMISSED_KEY);
    if (!raw) return new Set();
    const parsed: unknown = JSON.parse(raw);
    if (Array.isArray(parsed)) {
      return new Set(parsed.filter((id): id is string => typeof id === "string"));
    }
  } catch {
    // localStorage unavailable or corrupt — start with an empty set.
  }
  return new Set();
}

function loadRootPick(): string | null {
  try {
    return localStorage.getItem(ROOT_PICK_KEY);
  } catch {
    return null;
  }
}

export default function App() {
  const state = useAppState();
  const [theme, setTheme] = useState<Theme>(initialTheme);
  // Restore the persisted tab layout once; `null` means "no saved layout", which
  // lets the bootstrap open a tab for the effective root (today's behaviour).
  const [persistedLayout] = useState<Layout | null>(() => {
    try {
      return parseLayout(localStorage.getItem(LAYOUT_KEY));
    } catch {
      return null;
    }
  });
  const [layout, setLayout] = useState<Layout>(() => persistedLayout ?? EMPTY_LAYOUT);
  // Flips true once the first authoritative snapshot lands, gating persistence
  // and pruning so a restored layout is never wiped before bootstrap.
  const [storeReady, setStoreReady] = useState(false);
  const [effects, setEffects] = useState<Effect[]>([]);
  const [agentPanelOpen, setAgentPanelOpen] = useState(true);
  const [agentPanelWidth, setAgentPanelWidth] = useState(() =>
    loadWidth(AGENT_PANEL_KEY, AGENT_PANEL_DEFAULT, AGENT_PANEL_MIN, AGENT_PANEL_MAX),
  );
  const [editorOpen, setEditorOpen] = useState(true);
  const [editorPanelWidth, setEditorPanelWidth] = useState(() =>
    loadWidth(EDITOR_PANEL_KEY, EDITOR_PANEL_DEFAULT, EDITOR_PANEL_MIN, EDITOR_PANEL_MAX),
  );
  const [composerOpen, setComposerOpen] = useState(true);
  const [banner, setBanner] = useState<string | null>(null);
  const [dismissed, setDismissed] = useState<ReadonlySet<string>>(loadDismissed);
  const [rootPick, setRootPick] = useState<string | null>(loadRootPick);
  // Save-on-close gate: a dirty skill tab whose close was requested but not yet
  // granted. Only skill tabs ever reach it — every other tab closes directly.
  const [pendingSkillClose, setPendingSkillClose] = useState<{
    groupId: string;
    tabId: string;
    name: string;
  } | null>(null);
  const [savingSkill, setSavingSkill] = useState(false);
  const [saveSkillError, setSaveSkillError] = useState<string | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);

  const hadPersistedLayout = useRef(persistedLayout !== null);
  const previousSessionIds = useRef<string[]>([]);

  // Persist theme + tag the document for the palette.
  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    try {
      localStorage.setItem(THEME_KEY, theme);
    } catch {
      // ignore
    }
  }, [theme]);

  // Persist the UI-local hidden rows.
  useEffect(() => {
    try {
      localStorage.setItem(DISMISSED_KEY, JSON.stringify([...dismissed]));
    } catch {
      // ignore
    }
  }, [dismissed]);

  // Persist the orchestrator pick.
  useEffect(() => {
    try {
      if (rootPick) localStorage.setItem(ROOT_PICK_KEY, rootPick);
      else localStorage.removeItem(ROOT_PICK_KEY);
    } catch {
      // ignore
    }
  }, [rootPick]);

  // Persist the agents panel width.
  useEffect(() => {
    try {
      localStorage.setItem(AGENT_PANEL_KEY, String(agentPanelWidth));
    } catch {
      // ignore
    }
  }, [agentPanelWidth]);

  // Persist the editor panel width.
  useEffect(() => {
    try {
      localStorage.setItem(EDITOR_PANEL_KEY, String(editorPanelWidth));
    } catch {
      // ignore
    }
  }, [editorPanelWidth]);

  // Persist the tab layout once the core has been reached. Writing before the
  // first snapshot could clobber a restored layout with an empty pre-bootstrap
  // one and block the root bootstrap tab on the next launch.
  useEffect(() => {
    if (!storeReady) return;
    try {
      localStorage.setItem(LAYOUT_KEY, JSON.stringify(layout));
    } catch {
      // ignore
    }
  }, [storeReady, layout]);

  // Prune tabs whose session no longer exists (deleted server-side or absent
  // from the first snapshot). Gated on `storeReady` so a restored layout is not
  // pruned against the empty initial snapshot.
  useEffect(() => {
    if (!storeReady) return;
    const existing = new Set(state.sessions.map((entry) => entry.session.id));
    setLayout((current) => pruneSessionTabs(current, existing));
  }, [storeReady, state.sessions]);

  // A hidden row that becomes active again must reappear.
  useEffect(() => {
    setDismissed((current) => {
      if (current.size === 0) return current;
      let changed = false;
      const next = new Set<string>();
      for (const id of current) {
        const entry = state.sessions.find((s) => s.session.id === id);
        if (entry && (entry.status === "running" || entry.status === "waiting_approval")) {
          changed = true;
          continue;
        }
        next.add(id);
      }
      return changed ? next : current;
    });
  }, [state.sessions]);

  // Bootstrap: seed the snapshot and open the orchestrator pane.
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const snapshot = await api.getState();
        if (cancelled) return;
        store.setState(snapshot);
        setStoreReady(true);
        const rootId = resolveEffectiveRoot(snapshot, rootPick);
        if (rootId && !hadPersistedLayout.current) {
          setLayout((current) => openSessionTab(current, rootId));
        }
      } catch (cause) {
        if (!cancelled) setBanner(`Could not reach the core: ${describeError(cause)}`);
      }
    })();
    return () => {
      cancelled = true;
    };
    // The pick is read once at mount: the persisted value seeds the first pane.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Seed the local project-scope mirror from the shell's effective config, so
  // the picker shows the right project even when it was set via env or a
  // previous run (localStorage alone can be stale or empty).
  useEffect(() => {
    let cancelled = false;
    void api
      .getServerConfig()
      .then((cfg) => {
        if (!cancelled) setDirectory(cfg.directory ?? null);
      })
      .catch(() => {
        /* the picker falls back to the local mirror */
      });
    return () => {
      cancelled = true;
    };
  }, []);

  // Live traffic from the Rust shell.
  useEffect(() => {
    const unlistenPromise = listen<CoreMessage>(CORE_EVENT, (event) => {
      const message = event.payload;
      store.pushMessage(message);
      if (message.kind === "state_snapshot") setStoreReady(true);
      if (message.kind === "effect") {
        setEffects((current) => [message.effect, ...current].slice(0, EFFECT_LIMIT));
      }
    });
    return () => {
      void unlistenPromise.then((unlisten) => unlisten());
    };
  }, []);

  // Streams are keyed by session, so a session's stream is dropped only once no
  // tab anywhere still references it. `openSessionIds` is memoised to keep the
  // effect dependency stable across unrelated re-renders.
  const openSessionIds = useMemo(() => sessionTabIds(layout), [layout]);
  useEffect(() => {
    const current = new Set(openSessionIds);
    for (const sessionId of previousSessionIds.current) {
      if (!current.has(sessionId)) store.dropStream(sessionId);
    }
    previousSessionIds.current = openSessionIds;
  }, [openSessionIds]);

  // The file tree highlights the file tab that is active in the active group.
  const activePath = useMemo(() => activeFilePath(layout), [layout]);

  const togglePane = useCallback((sessionId: string) => {
    setLayout((current) =>
      hasSessionTab(current, sessionId)
        ? closeSessionTabs(current, sessionId)
        : openSessionTab(current, sessionId),
    );
  }, []);

  const closePane = useCallback((sessionId: string) => {
    setLayout((current) => closeSessionTabs(current, sessionId));
  }, []);

  const closeAllPanes = useCallback(() => setLayout(closeAllTabs()), []);

  /**
   * Close a tab, gating dirty skill tabs behind the save prompt. A clean skill
   * tab is closed straight away and its draft dropped; every other kind keeps
   * today's behaviour. Both the TabStrip ✕ and its context-menu Close tab route
   * through here.
   */
  const requestCloseTab = useCallback(
    (groupId: string, tabId: string) => {
      const tab = getTab(layout, groupId, tabId);
      if (tab?.kind === "skill" && isDirty(tab.id)) {
        setSaveSkillError(null);
        setPendingSkillClose({ groupId, tabId, name: tab.name });
        return;
      }
      if (tab?.kind === "skill") clearDraft(tab.id);
      setLayout((current) => closeTab(current, groupId, tabId));
    },
    [layout],
  );

  /** Save the pending draft to disk, then close its tab. */
  const savePendingSkill = useCallback(async () => {
    if (!pendingSkillClose) return;
    setSavingSkill(true);
    setSaveSkillError(null);
    try {
      const draft = getDraft(pendingSkillClose.tabId);
      await api.saveSkill(pendingSkillClose.name, draft?.content ?? "");
      markSaved(pendingSkillClose.tabId);
      clearDraft(pendingSkillClose.tabId);
      setLayout((current) =>
        closeTab(current, pendingSkillClose.groupId, pendingSkillClose.tabId),
      );
      setPendingSkillClose(null);
    } catch (cause) {
      // Keep the tab open and surface the failure so the content is not lost.
      setSaveSkillError(describeError(cause));
    } finally {
      setSavingSkill(false);
    }
  }, [pendingSkillClose]);

  /** Close without saving; the draft (and any scaffold) is dropped. */
  const discardPendingSkill = useCallback(() => {
    if (!pendingSkillClose) return;
    clearDraft(pendingSkillClose.tabId);
    setLayout((current) =>
      closeTab(current, pendingSkillClose.groupId, pendingSkillClose.tabId),
    );
    setPendingSkillClose(null);
    setSaveSkillError(null);
  }, [pendingSkillClose]);

  const cancelPendingSkill = useCallback(() => {
    setPendingSkillClose(null);
    setSaveSkillError(null);
  }, []);

  const handleAbort = useCallback((sessionId: string) => {
    void api.abortSession(sessionId).catch((cause) => {
      setBanner(`Abort failed: ${describeError(cause)}`);
    });
  }, []);

  const toggleTheme = useCallback(() => {
    setTheme((current) => (current === "dark" ? "light" : "dark"));
  }, []);

  const rootOptions = useMemo(() => parentlessSessions(state), [state]);
  const effectiveRootId = useMemo(
    () => resolveEffectiveRoot(state, rootPick),
    [state, rootPick],
  );

  // Hide a finished row: UI-local only (the server session stays alive) — it
  // also closes any tab for that session; once the tab is gone the layout effect
  // drops the stream. The root row never reaches this handler (AgentPanel does
  // not pass it the prop).
  const dismiss = useCallback(
    (sessionId: string) => {
      if (sessionId === effectiveRootId) return;
      setDismissed((current) => {
        if (current.has(sessionId)) return current;
        const next = new Set(current);
        next.add(sessionId);
        return next;
      });
      closePane(sessionId);
    },
    [effectiveRootId, closePane],
  );

  const pickRoot = useCallback((sessionId: string) => setRootPick(sessionId), []);

  /**
   * A project switch rebuilt the client on the new directory. Drop every
   * project-scoped UI trace (dismissed rows, layout, effects, root pick) and
   * re-adopt a fresh snapshot so no session or tab from the old project lingers.
   * The shell already reconnected and re-bootstrapped by the time this runs.
   */
  const handleProjectChanged = useCallback(async (_config: ServerConfig) => {
    setDismissed(new Set());
    setRootPick(null);
    setEffects([]);
    setLayout(EMPTY_LAYOUT);
    try {
      const snapshot = await api.getState();
      store.setState(snapshot);
      setStoreReady(true);
      const rootId = resolveEffectiveRoot(snapshot, null);
      if (rootId) setLayout(openSessionTab(EMPTY_LAYOUT, rootId));
    } catch (cause) {
      setBanner(`Could not load the project: ${describeError(cause)}`);
    }
  }, []);

  // Delete is destructive and server-side: only after the call resolves do we
  // drop the UI-local traces. Closing the pane drops its stream, and a persisted
  // root pick that pointed at the deleted session is cleared so
  // `resolveEffectiveRoot` falls back to `root_session_id` (the hub then pushes a
  // fresh snapshot — no getState() here, which could race the event).
  const deleteSession = useCallback(
    async (sessionId: string) => {
      await api.deleteSession(sessionId);
      closePane(sessionId);
      setRootPick((current) =>
        current === sessionId || sessionId === effectiveRootId ? null : current,
      );
      setDismissed((current) => {
        if (!current.has(sessionId)) return current;
        const next = new Set(current);
        next.delete(sessionId);
        return next;
      });
    },
    [closePane, effectiveRootId],
  );

  return (
    <div className="app">
      <TopBar
        rootSessionId={effectiveRootId}
        onOpenSkill={(name) => setLayout((current) => openSkillTab(current, name))}
        onOpenSettings={() => setSettingsOpen(true)}
        onProjectChanged={(config) => void handleProjectChanged(config)}
      />
      <ConnectionBanner ready={storeReady} />
      <div className="app__body">
        <ActivityBar
          theme={theme}
          agentPanelOpen={agentPanelOpen}
          editorOpen={editorOpen}
          composerOpen={composerOpen}
          onToggleAgentPanel={() => setAgentPanelOpen((value) => !value)}
          onToggleEditor={() => setEditorOpen((value) => !value)}
          onToggleComposer={() => setComposerOpen((value) => !value)}
          onCloseAllPanes={closeAllPanes}
          onToggleTheme={toggleTheme}
        />
        {agentPanelOpen && (
          <>
            <AgentPanel
              width={agentPanelWidth}
              openPanes={openSessionIds}
              effectiveRootId={effectiveRootId}
              parentless={rootOptions}
              dismissed={dismissed}
              onTogglePane={togglePane}
              onAbort={handleAbort}
              onDismiss={dismiss}
              onPickRoot={pickRoot}
              onDelete={deleteSession}
            />
            <PanelResizer
              width={agentPanelWidth}
              min={AGENT_PANEL_MIN}
              max={AGENT_PANEL_MAX}
              grow={1}
              onWidth={setAgentPanelWidth}
              label="Resize agents panel"
            />
          </>
        )}
        <main className="workspace">
          {banner && (
            <div className="banner" role="alert">
              <span className="banner__text">{banner}</span>
              <button
                type="button"
                className="banner__close"
                onClick={() => setBanner(null)}
                title="Dismiss"
              >
                ✕
              </button>
            </div>
          )}
          <PaneGrid
            layout={layout}
            sessions={state.sessions}
            theme={theme}
            onActivateTab={(groupId, tabId) =>
              setLayout((current) => activateTab(current, groupId, tabId))
            }
            onActivateGroup={(groupId) =>
              setLayout((current) => activateGroup(current, groupId))
            }
            onCloseTab={(groupId, tabId) => requestCloseTab(groupId, tabId)}
            onAbort={handleAbort}
            onSplit={(groupId, direction, tabId) =>
              setLayout((current) => splitGroup(current, groupId, direction, tabId))
            }
            onMoveTab={(sourceGroupId, tabId, targetGroupId, targetSide) =>
              setLayout((current) =>
                moveTab(current, sourceGroupId, tabId, targetGroupId, targetSide),
              )
            }
            onUpdateSplitSizes={(splitId, sizes) =>
              setLayout((current) => updateSplitSizes(current, splitId, sizes))
            }
          />
        </main>
        {editorOpen && (
          <>
            <PanelResizer
              width={editorPanelWidth}
              min={EDITOR_PANEL_MIN}
              max={EDITOR_PANEL_MAX}
              grow={-1}
              onWidth={setEditorPanelWidth}
              label="Resize editor panel"
            />
            <EditorPanel
              width={editorPanelWidth}
              theme={theme}
              rootSessionId={effectiveRootId}
              activePath={activePath}
              onOpenFile={(path) => setLayout((current) => openFileTab(current, path))}
            />
          </>
        )}
      </div>
      {composerOpen && <Composer sessionId={effectiveRootId} />}
      <ApprovalDialog />
      <SettingsDialog open={settingsOpen} onClose={() => setSettingsOpen(false)} />
      {pendingSkillClose && (
        <SkillSavePrompt
          name={pendingSkillClose.name}
          busy={savingSkill}
          error={saveSkillError}
          onSave={() => void savePendingSkill()}
          onDiscard={discardPendingSkill}
          onCancel={cancelPendingSkill}
        />
      )}
      <NotificationsDrawer
        effects={effects}
        onDismiss={(index) =>
          setEffects((current) => current.filter((_, i) => i !== index))
        }
        onDismissAll={() => setEffects([])}
      />
    </div>
  );
}