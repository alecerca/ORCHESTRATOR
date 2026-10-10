import type { AgentUiStatus, SessionSnapshot } from "../types";
import { shortId } from "../utils";
import { AgentTimer } from "./AgentTimer";
import { StatusChip } from "./StatusChip";

const FINISHED: ReadonlySet<AgentUiStatus> = new Set(["done", "idle", "error"]);

interface Props {
  snapshot: SessionSnapshot;
  isOpen: boolean;
  isRoot?: boolean;
  onToggle: (sessionId: string) => void;
  onAbort: (sessionId: string) => void;
  /** Omitted for the root row — the structural guarantee that it can't hide. */
  onDismiss?: (sessionId: string) => void;
}

/** One sub-agent (or the orchestrator) in the agent panel. */
export function AgentCard({
  snapshot,
  isOpen,
  isRoot = false,
  onToggle,
  onAbort,
  onDismiss,
}: Props) {
  const { session, status, error, status_since, last_run_ms } = snapshot;
  const canAbort = status === "running" || status === "waiting_approval";
  const canDismiss = !isRoot && onDismiss !== undefined && FINISHED.has(status);

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
        <span className="agent-card__badges">
          <AgentTimer
            status={status}
            statusSince={status_since}
            lastRunMs={last_run_ms}
          />
          <StatusChip status={status} />
        </span>
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
          {canDismiss && (
            <button
              type="button"
              className="btn btn--ghost btn--xs"
              onClick={() => onDismiss?.(session.id)}
              title="Hide from panel"
            >
              Hide
            </button>
          )}
        </div>
      </div>
      {error && <p className="agent-card__error">{error}</p>}
    </article>
  );
}
