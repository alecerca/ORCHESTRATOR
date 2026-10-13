import { useCallback, useEffect, useRef, useState } from "react";
import { gitBranches } from "../commands";
import type { GitBranch } from "../types";
import { describeError } from "../utils";

interface Props {
  /** The repository's current branch, or `null` before it is known. */
  branch: string | null;
  /**
   * Switch to `name`. The parent performs the actual checkout (and any reload
   * of its own data) and rejects when it fails, which is shown inline. The
   * branch list always reloads after a successful switch.
   */
  onSelect: (name: string) => Promise<void> | void;
  /** Disable the control (e.g. before the parent's first load has resolved). */
  disabled?: boolean;
}

/** Human label for a branch value: detached HEAD and unknown both get one. */
function branchLabel(branch: string | null): string {
  if (!branch) return "—";
  return branch === "HEAD" ? "detached" : branch;
}

/**
 * Branch dropdown shared by the git history panel and the working-tree diff
 * panel: the current branch, a lazily-loaded local branch list, and the
 * in-flight/error state of a switch. The parent owns the actual `git checkout`
 * (the app's only mutating git op) through `onSelect`.
 */
export function BranchSwitcher({ branch, onSelect, disabled = false }: Props) {
  const menuRef = useRef<HTMLDivElement>(null);
  const [menuOpen, setMenuOpen] = useState(false);
  const [branches, setBranches] = useState<GitBranch[]>([]);
  const [branchesLoading, setBranchesLoading] = useState(false);
  const [switching, setSwitching] = useState(false);
  const [switchError, setSwitchError] = useState<string | null>(null);

  // Guards late async writes so a checkout that resolves after unmount never
  // updates state.
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  // The local branch list is loaded lazily when the menu opens (and refreshed
  // again after a successful checkout).
  const loadBranches = useCallback(async () => {
    setBranchesLoading(true);
    try {
      const result = await gitBranches();
      if (!mounted.current) return;
      setBranches(result);
    } catch (cause) {
      if (!mounted.current) return;
      setBranches([]);
      setSwitchError(describeError(cause));
    } finally {
      if (mounted.current) setBranchesLoading(false);
    }
  }, []);

  // While the menu is open: Escape and outside pointerdown close it, and the
  // branch list is fetched on first open.
  useEffect(() => {
    if (!menuOpen) return;
    void loadBranches();
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") setMenuOpen(false);
    };
    const onPointerDown = (event: PointerEvent) => {
      if (menuRef.current && !menuRef.current.contains(event.target as Node)) {
        setMenuOpen(false);
      }
    };
    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("pointerdown", onPointerDown);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("pointerdown", onPointerDown);
    };
  }, [menuOpen, loadBranches]);

  const select = useCallback(
    async (name: string) => {
      // Picking the branch already checked out is a no-op: just close.
      if (name === branch) {
        setMenuOpen(false);
        return;
      }
      setSwitching(true);
      setSwitchError(null);
      try {
        await onSelect(name);
        if (!mounted.current) return;
        await loadBranches();
        if (!mounted.current) return;
        setMenuOpen(false);
      } catch (cause) {
        if (mounted.current) setSwitchError(describeError(cause));
      } finally {
        if (mounted.current) setSwitching(false);
      }
    },
    [branch, loadBranches, onSelect],
  );

  return (
    <div className="gitviewer__branchwrap" ref={menuRef}>
      <button
        type="button"
        className="gitviewer__branch"
        title={branchLabel(branch)}
        aria-haspopup="menu"
        aria-expanded={menuOpen}
        onClick={() => setMenuOpen((open) => !open)}
        disabled={switching || disabled}
      >
        <span className="gitviewer__branchname">{branchLabel(branch)}</span>
        <span className="gitviewer__chevron" aria-hidden="true">
          ▾
        </span>
      </button>
      {menuOpen && (
        <div className="gitviewer__menu" role="menu" aria-label="Switch branch">
          {branchesLoading && branches.length === 0 ? (
            <p className="gitviewer__muted">Loading branches…</p>
          ) : branches.length === 0 ? (
            <p className="gitviewer__muted">No local branches.</p>
          ) : (
            branches.map((menuBranch) => (
              <button
                key={menuBranch.name}
                type="button"
                role="menuitem"
                className={`gitviewer__menuitem${
                  menuBranch.current ? " is-current" : ""
                }`}
                aria-disabled={menuBranch.current || switching}
                disabled={switching}
                onClick={() => void select(menuBranch.name)}
                title={menuBranch.name}
              >
                <span className="gitviewer__branchname">{menuBranch.name}</span>
                {menuBranch.current && (
                  <span className="gitviewer__currentmark">current</span>
                )}
              </button>
            ))
          )}
        </div>
      )}
      {switchError && (
        <span className="gitviewer__switch-error" role="alert">
          {switchError}
        </span>
      )}
    </div>
  );
}