import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { store, useAppState, useLastMessage } from "./store";
import { CORE_EVENT, type AppStateSnapshot, type CoreMessage } from "./types";

/**
 * Scaffold shell.
 *
 * It only proves the end-to-end contract (command `get_state` + the
 * `core://message` event) so the real UI work can build on a verified base.
 * AGENT 2 replaces the body with the component tree defined in `docs/PLAN.md`.
 */
export default function App() {
  const state = useAppState();
  const lastMessage = useLastMessage();
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;

    void (async () => {
      try {
        const initial = await invoke<AppStateSnapshot>("get_state");
        store.setState(initial);
      } catch (cause) {
        setError(String(cause));
      }

      const off = await listen<CoreMessage>(CORE_EVENT, (event) => {
        store.pushMessage(event.payload);
      });
      if (disposed) {
        off();
      } else {
        unlisten = off;
      }
    })();

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  const rootId = state?.root_session_id ?? "—";
  const connected = state?.connected ? "connected" : "disconnected";

  return (
    <main className="shell">
      <header className="shell__header">
        <h1>Orquestador</h1>
        <span className={`status status--${state?.connected ? "up" : "down"}`}>
          {connected}
        </span>
      </header>

      <section className="shell__body">
        <dl className="facts">
          <dt>Root session</dt>
          <dd>{rootId}</dd>
          <dt>Sessions</dt>
          <dd>{state?.sessions.length ?? 0}</dd>
          <dt>Pending approvals</dt>
          <dd>{state?.permissions.length ?? 0}</dd>
          <dt>Server version</dt>
          <dd>{state?.server_version ?? "—"}</dd>
        </dl>

        <pre className="stream">
          {lastMessage
            ? JSON.stringify(lastMessage, null, 2)
            : "No core messages yet."}
        </pre>
      </section>

      {error && <p className="error">{error}</p>}
    </main>
  );
}
