import { useState } from "react";
import { DiffViewer } from "./DiffViewer";
import { FileTree } from "./FileTree";

interface Props {
  /** Panel width in pixels, owned by the parent (drag/keyboard resizable). */
  width: number;
  theme: "dark" | "light";
  /** Effective orchestrator root (persisted pick, else `root_session_id`). */
  rootSessionId: string | null;
  /** Path of the file tab currently active in the workspace, if any. */
  activePath: string | null;
  /** Open a file as a workspace tab (never an inline viewer). */
  onOpenFile: (path: string) => void;
}

/** Right column: the file explorer (files open as workspace tabs), or the session diff. */
export function EditorPanel({ width, theme, rootSessionId, activePath, onOpenFile }: Props) {
  const [tab, setTab] = useState<"files" | "diff">("files");

  return (
    <aside className="editor-panel" style={{ width }}>
      <div className="editor-panel__tabs">
        <button
          type="button"
          className={`editor-panel__tab${tab === "files" ? " is-active" : ""}`}
          onClick={() => setTab("files")}
        >
          Files
        </button>
        <button
          type="button"
          className={`editor-panel__tab${tab === "diff" ? " is-active" : ""}`}
          onClick={() => setTab("diff")}
        >
          Diff
        </button>
      </div>

      {tab === "files" ? (
        <div className="editor-panel__browser">
          <FileTree onOpenFile={onOpenFile} activePath={activePath} />
        </div>
      ) : (
        <div className="editor-panel__diff">
          <DiffViewer sessionId={rootSessionId} theme={theme} />
        </div>
      )}
    </aside>
  );
}