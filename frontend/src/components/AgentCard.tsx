import { useRef, useState } from "react";
import * as api from "../commands";
import type { AgentUiStatus, SessionSnapshot } from "../types";
import { describeError, shortId } from "../utils";
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
  /**
   * Server-side delete (destructive). The parent performs the call, closes the
   * pane/stream and clears a stale root pick; a rejection renders inline here.
   */
  onDelete: (sessionId: string) => Promise<void>;
}

/** One sub-agent (or the orchestrator) in the agent panel. */
export function AgentCard({
  snapshot,
  isOpen,
  isRoot = false,
  onToggle,
  onAbort,
  onDismiss,
  onDelete,
}: Props) {
  const { session, status, error, status_since, last_run_ms } = snapshot;
  const canAbort = status === "running" || status === "waiting_approval";
  const canDismiss = !isRoot && onDismiss !== undefined && FINISHED.has(status);

  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");
  const [renameError, setRenameError] = useState<string | null>(null);
  const [confirming, setConfirming] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  // Guards against a double commit when Enter collapses the input and the
  // browser still delivers a trailing blur.
  const renameFinished = useRef(false);

  const startRename = () => {
    renameFinished.current = false;
    setDraft(session.title ?? "");
    setRenameError(null);
    setConfirming(false);
    setEditing(true);
  };

  const commitRename = () => {
    if (renameFinished.current) return;
    renameFinished.current = true;
    setEditing(false);
    const value = draft.trim();
    if (!value || value === session.title) return; // empty or unchanged: cancel
    void (async () => {
      try {
        await api.renameSession(session.id, value);
      } catch (cause) {
        setRenameError(describeError(cause));
      }
    })();
  };

  const cancelRename = () => {
    renameFinished.current = true;
    setEditing(false);
    setRenameError(null);
  };

  const confirmDelete = () => {
    setConfirming(false);
    setDeleteError(null);
    void onDelete(session.id).catch((cause) => {
      setDeleteError(describeError(cause));
    });
  };

  return (
    <article
      className={`agent-card${isOpen ? " agent-card--open" : ""}${
        isRoot ? " agent-card--root" : ""
      }`}
    >
      {editing ? (
        <div className="agent-card__main">
          <input
            className="agent-card__rename-input"
            type="text"
            value={draft}
            autoFocus
            aria-label="Rename session"
            onChange={(event) => setDraft(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.preventDefault();
                commitRename();
              } else if (event.key === "Escape") {
                event.preventDefault();
                cancelRename();
              }
            }}
            onBlur={commitRename}
          />
        </div>
      ) : (
        <button
          type="button"
          className="agent-card__main"
          onClick={() => onToggle(session.id)}
          onDoubleClick={startRename}
          title={isOpen ? "Close this pane" : "Open this agent in a pane"}
        >
          <span className="agent-card__title" title="Double-click to rename">
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
      )}
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
          <button
            type="button"
            className="btn btn--danger btn--xs"
            onClick={() => setConfirming(true)}
            title="Delete this session"
            aria-label="Delete this session"
          >
            Delete
          </button>
        </div>
      </div>
      {confirming && (
        <div
          className="agent-card__confirm"
          role="group"
          aria-label="Confirm session deletion"
        >
          <span className="agent-card__confirm-text">Delete this session?</span>
          <button
            type="button"
            className="btn btn--danger btn--xs"
            onClick={confirmDelete}
          >
            Confirm
          </button>
          <button
            type="button"
            className="btn btn--ghost btn--xs"
            onClick={() => setConfirming(false)}
          >
            Cancel
          </button>
        </div>
      )}
      {renameError && <p className="agent-card__error">{renameError}</p>}
      {deleteError && <p className="agent-card__error">{deleteError}</p>}
      {error && <p className="agent-card__error">{error}</p>}
    </article>
  );
}
