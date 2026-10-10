import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import * as api from "./commands";
import { store, useAppState } from "./store";
import { CORE_EVENT, type CoreMessage, type Effect } from "./types";
import { describeError } from "./utils";
import { TopBar } from "./components/TopBar";
import { ActivityBar } from "./components/ActivityBar";
import { AgentPanel } from "./components/AgentPanel";
import { PaneGrid } from "./components/PaneGrid";
import { Composer } from "./components/Composer";
import { ApprovalDialog } from "./components/ApprovalDialog";
import { NotificationsDrawer } from "./components/NotificationsDrawer";
import { EditorPanel } from "./components/EditorPanel";

type Theme = "dark" | "light";

const THEME_KEY = "orquestador.theme";
const EFFECT_LIMIT = 20;

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

export default function App() {
  const state = useAppState();
  const [theme, setTheme] = useState<Theme>(initialTheme);
  const [panes, setPanes] = useState<string[]>([]);
  const [effects, setEffects] = useState<Effect[]>([]);
  const [agentPanelOpen, setAgentPanelOpen] = useState(true);
  const [editorOpen, setEditorOpen] = useState(true);
  const [composerOpen, setComposerOpen] = useState(true);
  const [banner, setBanner] = useState<string | null>(null);

  const prevPanesRef = useRef<string[]>([]);

  // Persist theme + tag the document for the palette.
  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    try {
      localStorage.setItem(THEME_KEY, theme);
    } catch {
      // ignore
    }
  }, [theme]);

  // Bootstrap: seed the snapshot and open the orchestrator pane.
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const snapshot = await api.getState();
        if (cancelled) return;
        store.setState(snapshot);
        const rootId = snapshot.root_session_id;
        if (rootId) {
          setPanes((current) => (current.length === 0 ? [rootId] : current));
        }
      } catch (cause) {
        if (!cancelled) setBanner(`Could not reach the core: ${describeError(cause)}`);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  // Live traffic from the Rust shell.
  useEffect(() => {
    const unlistenPromise = listen<CoreMessage>(CORE_EVENT, (event) => {
      const message = event.payload;
      store.pushMessage(message);
      if (message.kind === "effect") {
        setEffects((current) => [message.effect, ...current].slice(0, EFFECT_LIMIT));
      }
    });
    return () => {
      void unlistenPromise.then((unlisten) => unlisten());
    };
  }, []);

  // Drop the stream cache of panes that were closed.
  useEffect(() => {
    const previous = prevPanesRef.current;
    for (const sessionId of previous) {
      if (!panes.includes(sessionId)) store.dropStream(sessionId);
    }
    prevPanesRef.current = panes;
  }, [panes]);

  const togglePane = useCallback((sessionId: string) => {
    setPanes((current) =>
      current.includes(sessionId)
        ? current.filter((id) => id !== sessionId)
        : [...current, sessionId],
    );
  }, []);

  const closePane = useCallback((sessionId: string) => {
    setPanes((current) => current.filter((id) => id !== sessionId));
  }, []);

  const closeAllPanes = useCallback(() => setPanes([]), []);

  const handleAbort = useCallback((sessionId: string) => {
    void api.abortSession(sessionId).catch((cause) => {
      setBanner(`Abort failed: ${describeError(cause)}`);
    });
  }, []);

  const toggleTheme = useCallback(() => {
    setTheme((current) => (current === "dark" ? "light" : "dark"));
  }, []);

  const rootSessionId = state.root_session_id ?? null;

  return (
    <div className="app">
      <TopBar />
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
          <AgentPanel
            openPanes={panes}
            onTogglePane={togglePane}
            onAbort={handleAbort}
          />
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
          <PaneGrid panes={panes} sessions={state.sessions} onClosePane={closePane} />
        </main>
        {editorOpen && <EditorPanel theme={theme} />}
      </div>
      {composerOpen && <Composer sessionId={rootSessionId} />}
      <ApprovalDialog />
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