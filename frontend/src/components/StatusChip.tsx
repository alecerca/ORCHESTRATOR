import type { AgentUiStatus } from "../types";

const LABELS: Record<AgentUiStatus, string> = {
  pending: "Pending",
  idle: "Idle",
  done: "Done",
  running: "Running",
  waiting_approval: "Approval",
  error: "Error",
};

/** Human-readable tooltip; never expose the raw status enum to the user. */
const TITLES: Record<AgentUiStatus, string> = {
  pending: "Pending",
  idle: "Idle",
  done: "Done",
  running: "Running",
  waiting_approval: "Waiting for approval",
  error: "Error",
};

export function StatusChip({ status }: { status: AgentUiStatus }) {
  return (
    <span className={`chip chip--${status}`} title={TITLES[status]}>
      {LABELS[status]}
    </span>
  );
}
