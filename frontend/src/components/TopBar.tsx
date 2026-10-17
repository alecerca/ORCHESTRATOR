import {
  useCallback,
  useEffect,
  useRef,
  useState,
  useSyncExternalStore,
  type ChangeEvent,
} from "react";
import { listSkills, saveSkill } from "../commands";
import { getSnapshot, subscribe, toggle } from "../skills";
import { useServerConfig } from "../serverConfig";
import { useAppState } from "../store";
import type { ServerConfig, SkillInfo } from "../types";
import { describeError, shortId } from "../utils";
import { PresenceStrip } from "./PresenceStrip";
import { ProjectPicker } from "./ProjectPicker";
import { ModelPicker } from "./ModelPicker";
import { OpenCodeDialog } from "./OpenCodeDialog";
import { ClarificationDialog } from "./ClarificationDialog";

interface Props {
  /** Effective orchestrator root (persisted pick, else `root_session_id`). */
  rootSessionId: string | null;
  /** Open or focus the skill editor tab for `name`; App owns the layout. */
  onOpenSkill?: (name: string) => void;
  /** Open the server settings dialog; App owns the dialog instance. */
  onOpenSettings?: () => void;
  /** A project switch happened; App refreshes its project-scoped state. */
  onProjectChanged?: (config: ServerConfig) => void;
}

/**
 * Frontend mirror of the core `slugify`: trim, lowercase, collapse every run of
 * non `[a-z0-9]` into a `-`, then drop leading/trailing dashes. It gates new
 * names and matches import names against the list the shell returns; the shell
 * still slugs authoritatively on every read/write.
 */
function slugifyName(raw: string): string {
  return raw
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

/** Read a `File` as UTF-8 text through a `FileReader` (no server round-trip). */
function readText(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () =>
      resolve(typeof reader.result === "string" ? reader.result : "");
    reader.onerror = () =>
      reject(reader.error ?? new Error("Could not read the file."));
    reader.readAsText(file);
  });
}

/**
 * Top strip: brand, root session, server version, the connection dot, and the
 * Skills controls — a dropdown of saved skills (each a selection toggle) plus a
 * `+` menu to author a new skill or import a `.md`.
 */
export function TopBar({ rootSessionId, onOpenSkill, onOpenSettings, onProjectChanged }: Props) {
  const state = useAppState();
  const serverConfig = useServerConfig();
  const rootId = rootSessionId;
  const rootSnapshot = rootId
    ? state.sessions.find((s) => s.session.id === rootId)
    : undefined;
  const rootTitle = rootSnapshot
    ? rootSnapshot.session.title || shortId(rootSnapshot.session.id)
    : null;
  const pending = state.permissions.length;

  // Selected names live in the persisted skills store; the snapshot reference is
  // stable between mutations, so the highlighted toggles track it directly.
  const selected = useSyncExternalStore(subscribe, getSnapshot, getSnapshot);

  const [skills, setSkills] = useState<SkillInfo[]>([]);
  const [skillsOpen, setSkillsOpen] = useState(false);
  const [menuOpen, setMenuOpen] = useState(false);
  const [ocOpen, setOcOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const actionsRef = useRef<HTMLDivElement | null>(null);
  const fileInputRef = useRef<HTMLInputElement | null>(null);

  /** Re-read the on-disk skill list; also the shared error channel. */
  const refreshSkills = useCallback(async () => {
    try {
      setSkills(await listSkills());
      setError(null);
    } catch (cause) {
      setError(describeError(cause));
    }
  }, []);

  // Seed the list once so the button is accurate before the first open.
  useEffect(() => {
    void refreshSkills();
  }, [refreshSkills]);

  // While either surface is open: close on Escape and on any pointerdown
  // outside the actions region (mirrors the TabStrip context-menu behaviour).
  useEffect(() => {
    if (!skillsOpen && !menuOpen) return;
    function onKeyDown(event: KeyboardEvent): void {
      if (event.key !== "Escape") return;
      setSkillsOpen(false);
      setMenuOpen(false);
    }
    function onPointerDown(event: PointerEvent): void {
      const target = event.target as Node | null;
      if (actionsRef.current && target && actionsRef.current.contains(target)) return;
      setSkillsOpen(false);
      setMenuOpen(false);
    }
    document.addEventListener("keydown", onKeyDown);
    document.addEventListener("pointerdown", onPointerDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
      document.removeEventListener("pointerdown", onPointerDown);
    };
  }, [skillsOpen, menuOpen]);

  function toggleSkillsPanel(): void {
    const next = !skillsOpen;
    setMenuOpen(false);
    setSkillsOpen(next);
    // Refresh on open so skills saved from an editor tab show up immediately.
    if (next) void refreshSkills();
  }

  function togglePlusMenu(): void {
    setSkillsOpen(false);
    setMenuOpen((open) => !open);
  }

  function handleNewSkill(): void {
    setMenuOpen(false);
    const raw = window.prompt("Name for the new skill:");
    if (raw === null) return; // cancelled — nothing to do
    const slug = slugifyName(raw);
    if (!slug) {
      setError("Enter a name with at least one letter or number.");
      return;
    }
    setError(null);
    // Open the tab under the slug so it matches the on-disk stem and the list.
    onOpenSkill?.(slug);
  }

  function handleImportClick(): void {
    setMenuOpen(false);
    fileInputRef.current?.click();
  }

  async function handleFileChosen(event: ChangeEvent<HTMLInputElement>): Promise<void> {
    const input = event.currentTarget;
    const file = input.files?.[0];
    input.value = ""; // let the user re-pick the same file afterwards
    if (!file) return;

    const stem = file.name.replace(/\.(md|markdown)$/i, "");
    if (stem === file.name) {
      setError("Only .md or .markdown files can be imported.");
      return;
    }

    try {
      const content = await readText(file);
      const slug = slugifyName(stem) || "skill";
      // Confirm against a fresh list so a concurrent write is not clobbered blind.
      const current = await listSkills();
      if (
        current.some((skill) => skill.name === slug) &&
        !window.confirm(`A skill named "${slug}" already exists. Overwrite it?`)
      ) {
        return;
      }
      await saveSkill(slug, content);
      await refreshSkills();
      setSkillsOpen(true);
    } catch (cause) {
      setError(describeError(cause));
    }
  }

  // Count only selected skills that exist in the current list: the persisted
  // store can hold names from another backend or a since-deleted file, and the
  // badge must never over-report them.
  const availableCount = selected.filter((name) =>
    skills.some((skill) => skill.name === name),
  ).length;

  return (
    <header className="topbar">
      <div className="topbar__brand">
        <span className="topbar__logo" aria-hidden>
          ◆
        </span>
        <h1>Orquestador</h1>
        <a
          className="topbar__kofi"
          href="https://ko-fi.com/Z1W828S9J8"
          target="_blank"
          rel="noreferrer"
          title="Support me on Ko-fi"
        >
          <img
            src="https://storage.ko-fi.com/cdn/kofi6.png?v=6"
            alt="Support me on Ko-fi"
          />
        </a>
      </div>
      <div className="topbar__right">
        <PresenceStrip />
        <div className="topbar__actions" ref={actionsRef}>
          <ProjectPicker
            directory={serverConfig.directory ?? null}
            onChanged={(config) => onProjectChanged?.(config)}
          />
          <ModelPicker rootSessionId={rootId} />
          <button
            type="button"
            className="topbar__btn"
            title="OpenCode surface: providers, agents, MCP, commands, config"
            onClick={() => setOcOpen(true)}
          >
            OpenCode
          </button>
          <div className="skills">
            <button
              type="button"
              className={`topbar__btn${skillsOpen ? " is-active" : ""}`}
              aria-haspopup="true"
              aria-expanded={skillsOpen}
              title="Saved skills"
              onClick={toggleSkillsPanel}
            >
              Skills
              {availableCount > 0 ? ` (${availableCount})` : ""}
            </button>
            {skillsOpen && (
              <div className="skills-popover" role="group" aria-label="Saved skills">
                {skills.length === 0 ? (
                  <p className="skills-popover__empty">
                    No skills yet. Use + to add one.
                  </p>
                ) : (
                  <ul className="skills-list">
                    {skills.map((skill) => {
                      const active = selected.includes(skill.name);
                      return (
                        <li key={skill.name}>
                          <button
                            type="button"
                            className={`skills-item${active ? " is-selected" : ""}`}
                            aria-pressed={active}
                            title={skill.path}
                            onClick={() => toggle(skill.name)}
                          >
                            <span className="skills-item__check" aria-hidden>
                              {active ? "✓" : ""}
                            </span>
                            <span className="skills-item__name">{skill.name}</span>
                            <span
                              className="skills-item__source"
                              title={
                                skill.editable
                                  ? "App skill (editable)"
                                  : `${skill.source} skill (read-only)`
                              }
                              style={{
                                marginLeft: "auto",
                                flexShrink: 0,
                                color: "var(--muted)",
                                fontSize: "11px",
                              }}
                            >
                              {skill.source}
                            </span>
                          </button>
                        </li>
                      );
                    })}
                  </ul>
                )}
                {error && <p className="skills-popover__error">{error}</p>}
              </div>
            )}
            {!skillsOpen && error && (
              <span className="skills-error" role="alert">
                {error}
              </span>
            )}
          </div>
          <div className="skills-menu-wrap">
            <button
              type="button"
              className={`topbar__btn${menuOpen ? " is-active" : ""}`}
              aria-haspopup="menu"
              aria-expanded={menuOpen}
              title="Add a skill"
              onClick={togglePlusMenu}
            >
              +
            </button>
            {menuOpen && (
              <div className="skills-menu" role="menu">
                <button type="button" role="menuitem" onClick={handleNewSkill}>
                  New skill…
                </button>
                <button type="button" role="menuitem" onClick={handleImportClick}>
                  Import .md…
                </button>
              </div>
            )}
          </div>
          <input
            ref={fileInputRef}
            type="file"
            accept=".md,.markdown,text/markdown"
            className="skills-file-input"
            tabIndex={-1}
            aria-hidden
            onChange={(event) => void handleFileChosen(event)}
          />
        </div>
        <div className="topbar__meta">
          {pending > 0 && (
            <span className="topbar__pending" title="Approvals waiting for you">
              {pending} approval{pending > 1 ? "s" : ""}
            </span>
          )}
          <span
            className="topbar__root"
            title={rootId ? `Orchestrator session: ${rootId}` : "No orchestrator session yet"}
          >
            {rootTitle ? `orchestrator: ${rootTitle}` : "no orchestrator"}
          </span>
          <span className="topbar__version">
            {state.server_version ? `v${state.server_version}` : "—"}
          </span>
          <span
            className={`dot ${state.connected ? "dot--up" : "dot--down"}`}
            title={state.connected ? "connected" : "disconnected"}
          />
          <button
            className="btn btn--ghost btn--xs"
            onClick={() => onOpenSettings?.()}
            aria-label="Server settings"
            title="Server settings"
          >
            ⚙
          </button>
        </div>
      </div>
      {ocOpen && <OpenCodeDialog open onClose={() => setOcOpen(false)} />}
      <ClarificationDialog />
    </header>
  );
}
