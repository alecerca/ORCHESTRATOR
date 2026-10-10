import { useAppState } from "../store";
import type { SessionSnapshot } from "../types";
import { shortId } from "../utils";
import { AgentCard } from "./AgentCard";

interface Props {
  openPanes: string[];
  /** Effective orchestrator root (persisted pick, else `root_session_id`). */
  effectiveRootId: string | null;
  /** Parentless sessions offered by the root picker. */
  parentless: SessionSnapshot[];
  /** UI-local hidden rows (never the effective root). */
  dismissed: ReadonlySet<string>;
  onTogglePane: (sessionId: string) => void;
  onAbort: (sessionId: string) => void;
  onDismiss: (sessionId: string) => void;
  onPickRoot: (sessionId: string) => void;
}

/** Truncated label for the root picker option. */
function rootOptionLabel(snapshot: SessionSnapshot): string {
  const { session } = snapshot;
  const title = session.title
    ? session.title.length > 32
      ? `${session.title.slice(0, 32)}…`
      : session.title
    : "Untitled";
  return `${title} · ${shortId(session.id)}`;
}

/**
 * Left column: the orchestrator card plus every sub-agent, all fed live from the
 * cached snapshot (the bootstrap already lists every session, so no extra polling
 * is needed — see IMPLEMENTATION_NOTES).
 *
 * The root row is always rendered from the `isRoot` branch, which never receives
 * `onDismiss` — the orchestrator cannot be hidden even by state accident.
 */
export function AgentPanel({
  openPanes,
  effectiveRootId,
  parentless,
  dismissed,
  onTogglePane,
  onAbort,
  onDismiss,
  onPickRoot,
}: Props) {
  const state = useAppState();

  const rootSnapshot = effectiveRootId
    ? state.sessions.find((s) => s.session.id === effectiveRootId)
    : undefined;

  // Options show every parentless session, plus the effective root if it is not
  // itself parentless, so the select always reflects the real root.
  const rootChoices = effectiveRootId && !parentless.some((s) => s.session.id === effectiveRootId)
    ? rootSnapshot
      ? [...parentless, rootSnapshot]
      : parentless
    : parentless;

  const subAgents: SessionSnapshot[] = state.sessions.filter((snapshot) => {
    if (dismissed.has(snapshot.session.id)) return false;
    const parent = snapshot.session.parentID;
    if (!parent) return false;
    return effectiveRootId ? parent === effectiveRootId : true;
  });

  return (
    <aside className="agent-panel">
      <div className="panel-heading">
        <span>Orchestrator</span>
      </div>
      <select
        className="composer__input"
        style={{ width: "100%", marginBottom: "8px" }}
        aria-label="Orchestrator session"
        title="Choose which session acts as the orchestrator"
        value={effectiveRootId ?? ""}
        onChange={(event) => {
          if (event.target.value) onPickRoot(event.target.value);
        }}
      >
        {rootChoices.length === 0 ? (
          <option value="">No orchestrator session…</option>
        ) : (
          rootChoices.map((snapshot) => (
            <option key={snapshot.session.id} value={snapshot.session.id}>
              {rootOptionLabel(snapshot)}
            </option>
          ))
        )}
      </select>

      {rootSnapshot ? (
        <AgentCard
          snapshot={rootSnapshot}
          isRoot
          isOpen={openPanes.includes(rootSnapshot.session.id)}
          onToggle={onTogglePane}
          onAbort={onAbort}
        />
      ) : (
        <p className="agent-panel__empty">Connecting to the orchestrator…</p>
      )}

      <div className="panel-heading panel-heading--spaced">
        <span>Sub-agents</span>
        <span className="panel-heading__count">{subAgents.length}</span>
      </div>
      {subAgents.length === 0 ? (
        <p className="agent-panel__empty">
          No sub-agents yet. Ask the orchestrator to spawn some.
        </p>
      ) : (
        <div className="agent-panel__list">
          {subAgents.map((snapshot) => (
            <AgentCard
              key={snapshot.session.id}
              snapshot={snapshot}
              isOpen={openPanes.includes(snapshot.session.id)}
              onToggle={onTogglePane}
              onAbort={onAbort}
              onDismiss={onDismiss}
            />
          ))}
        </div>
      )}
    </aside>
  );
}
