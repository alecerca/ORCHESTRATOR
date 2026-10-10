import type { SessionSnapshot } from "../types";
import { shortId } from "../utils";
import { StatusChip } from "./StatusChip";

interface Props {
  snapshot: SessionSnapshot;
  isOpen: boolean;
  isRoot?: boolean;
  onToggle: (sessionId: string) => void;
  onAbort: (sessionId: string) => void;
}

/** One sub-agent (or the orchestrator) in the agent panel. */
export function AgentCard({
  snapshot,
  isOpen,
  isRoot = false,
  onToggle,
  onAbort,
}: Props) {
  const { session, status, error } = snapshot;
  const canAbort = status === "running" || status === "waiting_approval";

  return (
    <article
      className={`agent-card${isOpen ? " agent-card--open" : ""}${
        isRoot ? " agent-card--root" : ""
      }`}
    >
      <button
        type="button"
        className="agent-card__main"
        onClick={() => onToggle(session.id)}
        title={isOpen ? "Close this pane" : "Open this agent in a pane"}
      >
        <span className="agent-card__title">
          {session.title || shortId(session.id)}
        </span>
        <StatusChip status={status} />
      </button>
      <div className="agent-card__meta">
        <span className="agent-card__id">{shortId(session.id)}</span>
        <div className="agent-card__actions">
          {canAbort && (
            <button
              type="button"
              className="btn btn--danger btn--xs"
              onClick={() => onAbort(session.id)}
              title="Abort this agent"
            >
              Abort
            </button>
          )}
          <button
            type="button"
            className="btn btn--ghost btn--xs"
            onClick={() => onToggle(session.id)}
          >
            {isOpen ? "Close" : "Open"}
          </button>
        </div>
      </div>
      {error && <p className="agent-card__error">{error}</p>}
    </article>
  );
}
