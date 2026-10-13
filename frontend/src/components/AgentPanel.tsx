import { useState } from "react";
import * as api from "../commands";
import { useAppState } from "../store";
import type { SessionSnapshot } from "../types";
import { describeError, shortId } from "../utils";
import { attributedTitle, effectiveMember } from "../identity";
import { getConfig } from "../serverConfig";
import { AgentCard } from "./AgentCard";

interface Props {
  /** Panel width in pixels, owned by the parent (drag/keyboard resizable). */
  width: number;
  /** Session ids that currently have a tab anywhere in the workspace. */
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
  /** Server-side delete, owned by the parent (closes pane/stream, clears pick). */
  onDelete: (sessionId: string) => Promise<void>;
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
 * `onDismiss` — the orchestrator cannot be hidden even by state accident. It
 * still gets rename/delete, because a user who creates several orchestrator
 * sessions needs to be able to remove them.
 */
export function AgentPanel({
  width,
  openPanes,
  effectiveRootId,
  parentless,
  dismissed,
  onTogglePane,
  onAbort,
  onDismiss,
  onPickRoot,
  onDelete,
}: Props) {
  const state = useAppState();
  const [createError, setCreateError] = useState<string | null>(null);

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

  // Create a parentless orchestrator session and immediately make it the
  // effective root. The hub pushes a fresh snapshot on its own (no getState()),
  // so the new session appears and becomes selectable without extra steps.
  const handleCreate = () => {
    setCreateError(null);
    void (async () => {
      try {
        const title = attributedTitle(effectiveMember(getConfig()), "New session");
        const session = await api.createSession(undefined, title);
        onPickRoot(session.id);
      } catch (cause) {
        setCreateError(describeError(cause));
      }
    })();
  };

  return (
    <aside className="agent-panel" style={{ width }}>
      <div className="panel-heading">
        <span>Orchestrator</span>
        <button
          type="button"
          className="btn btn--ghost btn--xs"
          onClick={handleCreate}
          title="Create a new orchestrator session"
          aria-label="New orchestrator session"
        >
          New session
        </button>
      </div>
      <select
        className="agent-panel__select"
        aria-label="Orchestrator session"
        title="Choose which session acts as the orchestrator"
        value={effectiveRootId ?? ""}
        disabled={rootChoices.length === 0}
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
      {createError && (
        <p className="agent-panel__error" role="alert">
          {createError}
        </p>
      )}

      {rootSnapshot ? (
        <AgentCard
          snapshot={rootSnapshot}
          isRoot
          isOpen={openPanes.includes(rootSnapshot.session.id)}
          onToggle={onTogglePane}
          onAbort={onAbort}
          onDelete={onDelete}
        />
      ) : (
        <p className="agent-panel__empty">
          {state.connected
            ? "No orchestrator session yet. Use “New session” to create one."
            : "Not connected to a server yet — check the connection in the top bar."}
        </p>
      )}

      <div className="panel-heading panel-heading--spaced">
        <span>Sub-agents</span>
        <span className="panel-heading__count">{subAgents.length}</span>
      </div>
      {subAgents.length === 0 ? (
        <p className="agent-panel__empty">
          No sub-agents yet. Ask the orchestrator in the composer below.
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
              onDelete={onDelete}
            />
          ))}
        </div>
      )}
    </aside>
  );
}
