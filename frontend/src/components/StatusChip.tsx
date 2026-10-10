import type { AgentUiStatus } from "../types";

const LABELS: Record<AgentUiStatus, string> = {
  pending: "Pending",
  idle: "Idle",
  done: "Done",
  running: "Running",
  waiting_approval: "Approval",
  error: "Error",
};

export function StatusChip({ status }: { status: AgentUiStatus }) {
  return (
    <span className={`chip chip--${status}`} title={status}>
      {LABELS[status]}
    </span>
  );
}
