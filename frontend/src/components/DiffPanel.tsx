import { useCallback, useEffect, useRef, useState } from "react";
import { gitCheckout, gitDiscardFile, gitLog, gitStatus } from "../commands";
import type { GitChange } from "../types";
import { describeError } from "../utils";
import { BranchSwitcher } from "./BranchSwitcher";
import { DiscardIcon } from "./icons";
import { invalidateGitBranches } from "./useGitBranch";

/** How often the working-tree status and branch are re-checked. */
const POLL_MS = 3000;

/** Single-letter badge for each change kind (VS Code style). */
const BADGES: Record<GitChange["kind"], string> = {
  modified: "M",
  added: "A",
  deleted: "D",
  untracked: "U",
};

/**
 * The working-tree changes of the active project: every path that differs from
 * HEAD, each opening a diff tab on click. Polled every few seconds so the list
 * tracks edits made elsewhere, with the current branch switchable through the
 * shared `BranchSwitcher`. An absent project directory is shown as a quiet
 * idle state, never a crash.
 *
 * Each row also carries a trailing "discard changes" button: it opens a
 * confirmation modal before throwing the path's working-tree changes away
 * (`git_discard_file`), then re-polls. The confirmation is deliberate — the
 * action is destructive and cannot be undone.
 */
export function DiffPanel({
  onOpenDiffFile,
}: {
  onOpenDiffFile: (path: string) => void;
}) {
  const [changes, setChanges] = useState<GitChange[] | null>(null);
  const [branch, setBranch] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pendingDiscard, setPendingDiscard] = useState<string | null>(null);
  const [discardError, setDiscardError] = useState<string | null>(null);
  const [discarding, setDiscarding] = useState(false);

  // Guards late async writes so a poll that resolves after unmount never
  // updates state.
  const mounted = useRef(true);

  const refresh = useCallback(async () => {
    try {
      const [status, log] = await Promise.all([gitStatus(), gitLog()]);
      if (!mounted.current) return;
      setChanges(status);
      setBranch(log.branch);
      setError(null);
    } catch (cause) {
      if (!mounted.current) return;
      setChanges(null);
      setError(describeError(cause));
    }
  }, []);

  // Poll on mount, on window focus (the working tree may have changed while the
  // app was in the background) and on the interval.
  useEffect(() => {
    mounted.current = true;
    void refresh();
    const timer = window.setInterval(() => void refresh(), POLL_MS);
    const onFocus = () => void refresh();
    window.addEventListener("focus", onFocus);
    return () => {
      mounted.current = false;
      window.clearInterval(timer);
      window.removeEventListener("focus", onFocus);
    };
  }, [refresh]);

  // A checkout switches the whole project view, so the cached session chips
  // are dropped and the status is re-read immediately.
  const selectBranch = useCallback(
    async (name: string) => {
      await gitCheckout(name);
      invalidateGitBranches();
      await refresh();
    },
    [refresh],
  );

  const cancelDiscard = useCallback(() => {
    setPendingDiscard(null);
    setDiscardError(null);
  }, []);

  // Confirmed discard: throw the path's working-tree changes away, then
  // re-poll. A failure keeps the modal open so its message stays readable.
  const discardPending = useCallback(async () => {
    if (!pendingDiscard) return;
    setDiscarding(true);
    try {
      await gitDiscardFile(pendingDiscard);
      setPendingDiscard(null);
      setDiscardError(null);
      await refresh();
    } catch (cause) {
      setDiscardError(describeError(cause));
    } finally {
      setDiscarding(false);
    }
  }, [pendingDiscard, refresh]);

  // Escape cancels the confirmation, matching the other modals.
  useEffect(() => {
    if (!pendingDiscard) return;
    function onKey(event: KeyboardEvent) {
      if (event.key === "Escape") cancelDiscard();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [pendingDiscard, cancelDiscard]);

  const sorted = changes
    ? [...changes].sort((a, b) => a.path.localeCompare(b.path))
    : null;

  return (
    <div className="diff-panel">
      <div className="diff-panel__header">
        <BranchSwitcher branch={branch} onSelect={selectBranch} />
      </div>
      {error ? (
        <p className="diff-panel__error">{error}</p>
      ) : sorted === null ? (
        <p className="diff-panel__muted">Checking working tree…</p>
      ) : sorted.length === 0 ? (
        <p className="diff-panel__muted">No changes — working tree clean</p>
      ) : (
        <div className="diff-panel__list">
          {sorted.map((change) => (
            <div key={change.path} className="diff-row">
              <button
                type="button"
                className="diff-row__open"
                title={change.path}
                onClick={() => onOpenDiffFile(change.path)}
              >
                <span className={`diff-badge diff-badge--${change.kind}`}>
                  {BADGES[change.kind]}
                </span>
                <span className="diff-row__path">{change.path}</span>
              </button>
              <button
                type="button"
                className="diff-row__discard"
                title="Discard changes"
                aria-label={"Discard changes to " + change.path}
                onClick={() => setPendingDiscard(change.path)}
              >
                <DiscardIcon />
              </button>
            </div>
          ))}
        </div>
      )}
      {pendingDiscard && (
        <div
          className="modal-backdrop"
          onClick={(event) => {
            if (event.target === event.currentTarget && !discarding) {
              cancelDiscard();
            }
          }}
        >
          <div
            className="modal approval"
            role="dialog"
            aria-modal="true"
            aria-labelledby="discard-title"
          >
            <div className="approval__head">
              <span className="chip chip--pending">file</span>
              <span className="approval__type">working tree</span>
            </div>
            <h2 className="approval__title" id="discard-title">
              Discard changes
            </h2>
            <p className="skill-prompt__body">
              Discard changes to{" "}
              <span className="skill-prompt__name">{pendingDiscard}</span>? This
              cannot be undone.
            </p>
            {discardError && (
              <p className="approval__error" role="alert">
                {discardError}
              </p>
            )}
            <div className="approval__actions">
              <button
                type="button"
                className="btn btn--danger"
                disabled={discarding}
                onClick={() => void discardPending()}
              >
                {discarding ? "Discarding…" : "Discard"}
              </button>
              <button
                type="button"
                className="btn"
                disabled={discarding}
                autoFocus
                onClick={cancelDiscard}
              >
                Cancel
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
