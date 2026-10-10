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
        onClick={onToggleAgentPanel}
      >
        ◧
      </button>
      <button
        type="button"
        className={`activitybar__btn${editorOpen ? " is-active" : ""}`}
        title={editorOpen ? "Hide editor" : "Show editor"}
        onClick={onToggleEditor}
      >
        ▤
      </button>
      <button
        type="button"
        className={`activitybar__btn${composerOpen ? " is-active" : ""}`}
        title={composerOpen ? "Hide composer" : "Show composer"}
        onClick={onToggleComposer}
      >
        ›_
      </button>
      <button
        type="button"
        className="activitybar__btn"
        title="Close all panes"
        onClick={onCloseAllPanes}
      >
        ▤
      </button>
      <div className="activitybar__spacer" />
      <button
        type="button"
        className="activitybar__btn"
        title={theme === "dark" ? "Switch to light theme" : "Switch to dark theme"}
        onClick={onToggleTheme}
      >
        {theme === "dark" ? "☀" : "☾"}
      </button>
    </nav>
  );
}