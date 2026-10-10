import { useAppState } from "../store";
import { shortId } from "../utils";

/** Top strip: brand, root session, server version and the connection dot. */
export function TopBar() {
  const state = useAppState();
  const rootId = state.root_session_id ?? null;
  const rootSnapshot = rootId
    ? state.sessions.find((s) => s.session.id === rootId)
    : undefined;
  const rootTitle = rootSnapshot
    ? rootSnapshot.session.title || shortId(rootSnapshot.session.id)
    : null;
  const pending = state.permissions.length;

  return (
    <header className="topbar">
      <div className="topbar__brand">
        <span className="topbar__logo" aria-hidden>
          ◆
        </span>
        <h1>Orquestador</h1>
      </div>
      <div className="topbar__meta">
        {pending > 0 && (
          <span className="topbar__pending" title="Approvals waiting for you">
            {pending} approval{pending > 1 ? "s" : ""}
          </span>
        )}
        <span className="topbar__root" title={rootId ?? "no root session"}>
          orchestrator: {rootTitle ?? "—"}
        </span>
        <span className="topbar__version">
          {state.server_version ? `v${state.server_version}` : "—"}
        </span>
        <span
          className={`dot ${state.connected ? "dot--up" : "dot--down"}`}
          title={state.connected ? "connected" : "disconnected"}
        />
      </div>
    </header>
  );
}
