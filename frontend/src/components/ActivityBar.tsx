interface Props {
  theme: "dark" | "light";
  agentPanelOpen: boolean;
  editorOpen: boolean;
  composerOpen: boolean;
  onToggleAgentPanel: () => void;
  onToggleEditor: () => void;
  onToggleComposer: () => void;
  onCloseAllPanes: () => void;
  onToggleTheme: () => void;
}

/** Shared stroke setup so every rail icon reads as one family. */
const ICON = {
  width: 16,
  height: 16,
  viewBox: "0 0 24 24",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.7,
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
  "aria-hidden": true,
};

function IconPanelLeft() {
  return (
    <svg {...ICON}>
      <rect x="3" y="4" width="18" height="16" rx="2" />
      <path d="M9 4v16" />
    </svg>
  );
}

function IconPanelRight() {
  return (
    <svg {...ICON}>
      <rect x="3" y="4" width="18" height="16" rx="2" />
      <path d="M15 4v16" />
    </svg>
  );
}

function IconTerminal() {
  return (
    <svg {...ICON}>
      <rect x="3" y="4" width="18" height="16" rx="2" />
      <path d="M7 9l3 3-3 3" />
      <path d="M12.5 15H17" />
    </svg>
  );
}

function IconCloseAll() {
  return (
    <svg {...ICON}>
      <path d="M6 6l12 12" />
      <path d="M18 6L6 18" />
    </svg>
  );
}

function IconSun() {
  return (
    <svg {...ICON}>
      <circle cx="12" cy="12" r="4" />
      <path d="M12 2v2M12 20v2M2 12h2M20 12h2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M19.1 4.9l-1.4 1.4M6.3 17.7l-1.4 1.4" />
    </svg>
  );
}

function IconMoon() {
  return (
    <svg {...ICON}>
      <path d="M20 14.5A8 8 0 1 1 9.5 4a6.5 6.5 0 0 0 10.5 10.5z" />
    </svg>
  );
}

/** Vertical icon rail: panel toggles and the theme switch. */
export function ActivityBar({
  theme,
  agentPanelOpen,
  editorOpen,
  composerOpen,
  onToggleAgentPanel,
  onToggleEditor,
  onToggleComposer,
  onCloseAllPanes,
  onToggleTheme,
}: Props) {
  return (
    <nav className="activitybar" aria-label="View controls">
      <button
        type="button"
        className={`activitybar__btn${agentPanelOpen ? " is-active" : ""}`}
        title={agentPanelOpen ? "Hide agents" : "Show agents"}
        aria-label={agentPanelOpen ? "Hide agents panel" : "Show agents panel"}
        aria-pressed={agentPanelOpen}
        onClick={onToggleAgentPanel}
      >
        <IconPanelLeft />
      </button>
      <button
        type="button"
        className={`activitybar__btn${editorOpen ? " is-active" : ""}`}
        title={editorOpen ? "Hide editor" : "Show editor"}
        aria-label={editorOpen ? "Hide editor panel" : "Show editor panel"}
        aria-pressed={editorOpen}
        onClick={onToggleEditor}
      >
        <IconPanelRight />
      </button>
      <button
        type="button"
        className={`activitybar__btn${composerOpen ? " is-active" : ""}`}
        title={composerOpen ? "Hide composer" : "Show composer"}
        aria-label={composerOpen ? "Hide composer" : "Show composer"}
        aria-pressed={composerOpen}
        onClick={onToggleComposer}
      >
        <IconTerminal />
      </button>
      <button
        type="button"
        className="activitybar__btn"
        title="Close all panes"
        aria-label="Close all panes"
        onClick={onCloseAllPanes}
      >
        <IconCloseAll />
      </button>
      <div className="activitybar__spacer" />
      <button
        type="button"
        className="activitybar__btn"
        title={theme === "dark" ? "Switch to light theme" : "Switch to dark theme"}
        aria-label={theme === "dark" ? "Switch to light theme" : "Switch to dark theme"}
        onClick={onToggleTheme}
      >
        {theme === "dark" ? <IconSun /> : <IconMoon />}
      </button>
    </nav>
  );
}
