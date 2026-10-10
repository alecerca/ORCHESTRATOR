import type { SessionSnapshot } from "../types";
import { shortId } from "../utils";
import { AgentTimer } from "./AgentTimer";
import { StatusChip } from "./StatusChip";

interface Props {
  sessionId: string;
  snapshot?: SessionSnapshot;
  onAbort: (sessionId: string) => void;
}

/**
 * Thin bar under a group's tab strip. Replaces the old per-`AgentPane` header:
 * it carries the active tab's title, execution timer, status chip and the Abort
 * action (only while the agent is running or waiting for approval).
 */
export function GroupInfoBar({ sessionId, snapshot, onAbort }: Props) {
  const status = snapshot?.status ?? "idle";
  const canAbort = status === "running" || status === "waiting_approval";

  return (
    <div className="group-infobar">
      <span className="group-infobar__title" title={sessionId}>
        {snapshot?.session.title || shortId(sessionId)}
      </span>
      <AgentTimer
        status={status}
        statusSince={snapshot?.status_since}
        lastRunMs={snapshot?.last_run_ms}
      />
      <StatusChip status={status} />
      {canAbort && (
        <button
          type="button"
          className="btn btn--danger btn--xs"
          onClick={() => onAbort(sessionId)}
          title="Abort this agent"
        >
          Abort
        </button>
      )}
    </div>
  );
}
