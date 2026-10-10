import { useEffect, useState } from "react";
import type { AgentUiStatus } from "../types";

/** Statuses that are actively executing, so their timer ticks live. */
const LIVE: ReadonlySet<AgentUiStatus> = new Set(["running", "waiting_approval"]);
/** Statuses that are finished, so their timer shows the frozen last-run duration. */
const FROZEN: ReadonlySet<AgentUiStatus> = new Set(["done", "error", "idle"]);

/**
 * Compact duration from milliseconds: `0:42`, `1:05`, `12:30`, `1:02:07`.
 * Minutes are unpadded under an hour (matching the `12:30` example) and padded
 * once hours appear, so every field keeps a stable width.
 */
export function formatDurationMs(ms: number): string {
  const totalSeconds = Math.max(0, Math.floor(ms / 1000));
  const seconds = String(totalSeconds % 60).padStart(2, "0");
  const totalMinutes = Math.floor(totalSeconds / 60);
  const minutes = totalMinutes % 60;
  const hours = Math.floor(totalMinutes / 60);
  if (hours > 0) {
    return `${hours}:${String(minutes).padStart(2, "0")}:${seconds}`;
  }
  return `${minutes}:${seconds}`;
}

interface Props {
  status: AgentUiStatus;
  /** Epoch ms of the last status change (from the snapshot). */
  statusSince?: number;
  /** Duration in ms of the previous run (from the snapshot). */
  lastRunMs?: number;
}

/**
 * Per-row execution timer. The 1 s interval lives HERE, inside a leaf, so only
 * this node re-renders on each tick instead of the whole `App` tree.
 *
 * - `running` / `waiting_approval` → live elapsed from `statusSince`.
 * - `done` / `error` / `idle` (with a recorded run) → frozen `lastRunMs`.
 * - `pending` or missing data → renders nothing.
 */
export function AgentTimer({ status, statusSince, lastRunMs }: Props) {
  const live = LIVE.has(status) && statusSince !== undefined;
  const frozen = !live && FROZEN.has(status) && lastRunMs !== undefined;
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    if (!live) return;
    // Resync immediately so a transition into a live status never shows a stale
    // (larger) elapsed value from the previous mount.
    setNow(Date.now());
    const id = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(id);
  }, [live, statusSince]);

  if (live && statusSince !== undefined) {
    const blocked = status === "waiting_approval";
    return (
      <span
        className={`agent-timer agent-timer--live${
          blocked ? " agent-timer--blocked" : ""
        }`}
        title={
          blocked
            ? "Running — blocked awaiting approval"
            : "Elapsed since this run started"
        }
      >
        {formatDurationMs(now - statusSince)}
      </span>
    );
  }

  if (frozen && lastRunMs !== undefined) {
    return (
      <span
        className="agent-timer agent-timer--frozen"
        title="Duration of the last run"
      >
        {formatDurationMs(lastRunMs)}
      </span>
    );
  }

  return null;
}
