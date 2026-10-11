import { useCallback, useEffect, useRef, useState } from "react";
import { listProjects, setProjectDirectory } from "../commands";
import { setDirectory } from "../serverConfig";
import type { Project, ServerConfig } from "../types";
import { baseName, describeError } from "../utils";

interface Props {
  /** Active project directory, or `null` for the server's own cwd project. */
  directory: string | null;
  /** Called after a successful switch with the updated server config. */
  onChanged: (config: ServerConfig) => void;
}

/**
 * Top-bar project switcher. One `opencode serve` can host several projects; this
 * dropdown lists the ones the server reports and lets the user scope the app to
 * a different one (or to a custom absolute path) without restarting the server.
 * The switch persists the choice and reconnects the client, then reports the new
 * config so the app can refresh its project-scoped state.
 */
export function ProjectPicker({ directory, onChanged }: Props) {
  const [open, setOpen] = useState(false);
  const [projects, setProjects] = useState<Project[]>([]);
  const [custom, setCustom] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const rootRef = useRef<HTMLDivElement | null>(null);

  /** Re-read the server's project list; also the shared error channel. */
  const refresh = useCallback(async () => {
    try {
      setProjects(await listProjects());
      setError(null);
    } catch (cause) {
      setError(describeError(cause));
    }
  }, []);

  // Close on Escape and on any pointerdown outside (mirrors the Skills popover).
  useEffect(() => {
    if (!open) return;
    function onKeyDown(event: KeyboardEvent): void {
      if (event.key === "Escape") setOpen(false);
    }
    function onPointerDown(event: PointerEvent): void {
      const target = event.target as Node | null;
      if (rootRef.current && target && rootRef.current.contains(target)) return;
      setOpen(false);
    }
    document.addEventListener("keydown", onKeyDown);
    document.addEventListener("pointerdown", onPointerDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
      document.removeEventListener("pointerdown", onPointerDown);
    };
  }, [open]);

  function toggle(): void {
    const next = !open;
    setOpen(next);
    // Refresh on open so a project that appeared since launch shows up.
    if (next) void refresh();
  }

  /** Switch to `next` (absolute dir, or `null` for the server cwd project). */
  async function choose(next: string | null): Promise<void> {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      const cfg = await setProjectDirectory(next);
      // Mirror locally so the label updates immediately; Rust is authoritative.
      setDirectory(cfg.directory ?? null);
      setCustom("");
      setOpen(false);
      onChanged(cfg);
    } catch (cause) {
      setError(describeError(cause));
    } finally {
      setBusy(false);
    }
  }

  function handleCustom(): void {
    const value = custom.trim();
    if (!value) return;
    void choose(value);
  }

  const label = directory ? baseName(directory) : "Default project";

  return (
    <div className="project-picker" ref={rootRef}>
      <button
        type="button"
        className={`topbar__btn${open ? " is-active" : ""}`}
        aria-haspopup="menu"
        aria-expanded={open}
        title={directory ?? "Server default (its working directory)"}
        onClick={toggle}
        disabled={busy}
      >
        Project: {label}
      </button>
      {open && (
        <div
          className="skills-popover project-picker__popover"
          role="menu"
          aria-label="Projects"
        >
          <ul className="skills-list">
            <li>
              <button
                type="button"
                role="menuitemradio"
                aria-checked={directory === null}
                className={`skills-item${directory === null ? " is-selected" : ""}`}
                title="Use the server's own working directory"
                disabled={busy}
                onClick={() => void choose(null)}
              >
                <span className="skills-item__check" aria-hidden>
                  {directory === null ? "✓" : ""}
                </span>
                <span className="skills-item__name">Server default (cwd)</span>
              </button>
            </li>
            {projects.map((project) => {
              const active = project.worktree === directory;
              return (
                <li key={project.id}>
                  <button
                    type="button"
                    role="menuitemradio"
                    aria-checked={active}
                    className={`skills-item${active ? " is-selected" : ""}`}
                    title={project.worktree}
                    disabled={busy}
                    onClick={() => void choose(project.worktree)}
                  >
                    <span className="skills-item__check" aria-hidden>
                      {active ? "✓" : ""}
                    </span>
                    <span className="skills-item__name">{baseName(project.worktree)}</span>
                  </button>
                </li>
              );
            })}
          </ul>
          <div className="project-picker__custom">
            <input
              className="settings-input"
              value={custom}
              placeholder="/absolute/path/to/project"
              aria-label="Project path"
              disabled={busy}
              onChange={(event) => setCustom(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") handleCustom();
              }}
            />
            <button
              type="button"
              className="btn btn--ghost btn--xs"
              disabled={busy || !custom.trim()}
              onClick={handleCustom}
            >
              Open
            </button>
          </div>
          {error && <p className="skills-popover__error">{error}</p>}
        </div>
      )}
    </div>
  );
}
