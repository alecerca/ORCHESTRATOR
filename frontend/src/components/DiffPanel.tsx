import { useCallback, useEffect, useRef, useState } from "react";
import { gitCheckout, gitLog, gitStatus } from "../commands";
import type { GitChange } from "../types";
import { describeError } from "../utils";
import { BranchSwitcher } from "./BranchSwitcher";
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
 */
export function DiffPanel({
  onOpenDiffFile,
}: {
  onOpenDiffFile: (path: string) => void;
}) {
  const [changes, setChanges] = useState<GitChange[] | null>(null);
  const [branch, setBranch] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

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
            <button
              key={change.path}
              type="button"
              className="diff-row"
              title={change.path}
              onClick={() => onOpenDiffFile(change.path)}
            >
              <span className={`diff-badge diff-badge--${change.kind}`}>
                {BADGES[change.kind]}
              </span>
              <span className="diff-row__path">{change.path}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}