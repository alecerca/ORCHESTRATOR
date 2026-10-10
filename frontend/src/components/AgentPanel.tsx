import { useAppState } from "../store";
import type { SessionSnapshot } from "../types";
import { AgentCard } from "./AgentCard";

interface Props {
  openPanes: string[];
  onTogglePane: (sessionId: string) => void;
  onAbort: (sessionId: string) => void;
}

/**
 * Left column: the orchestrator card plus every sub-agent, all fed live from the
 * cached snapshot (the bootstrap already lists every session, so no extra polling
 * is needed — see IMPLEMENTATION_NOTES).
 */
export function AgentPanel({ openPanes, onTogglePane, onAbort }: Props) {
  const state = useAppState();
  const rootId = state.root_session_id ?? null;

  const rootSnapshot = rootId
    ? state.sessions.find((s) => s.session.id === rootId)
    : undefined;

  const subAgents: SessionSnapshot[] = state.sessions.filter((snapshot) => {
    const parent = snapshot.session.parentID;
    if (!parent) return false;
    return rootId ? parent === rootId : true;
  });

  return (
    <aside className="agent-panel">
      <div className="panel-heading">
        <span>Orchestrator</span>
      </div>
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
            />
          ))}
        </div>
      )}
    </aside>
  );
}
