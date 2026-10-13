import { useState } from "react";
import { FileTree } from "./FileTree";
import { GitViewer } from "./GitViewer";

interface Props {
  /** Panel width in pixels, owned by the parent (drag/keyboard resizable). */
  width: number;
  /** Path of the file tab currently active in the workspace, if any. */
  activePath: string | null;
  /** Open a file as a workspace tab (never an inline viewer). */
  onOpenFile: (path: string) => void;
  /** Bumped after a checkout so the file explorer remounts and re-reads the tree. */
  filesVersion: number;
  /** A branch switch happened; refresh the rest of the UI (tree, chips, tabs). */
  onBranchChanged: () => void;
}

/** Right column: the file explorer (files open as workspace tabs), or the git history. */
export function EditorPanel({
  width,
  activePath,
  onOpenFile,
  filesVersion,
  onBranchChanged,
}: Props) {
  const [tab, setTab] = useState<"files" | "git">("files");

  return (
    <aside className="editor-panel" style={{ width }}>
      <div className="editor-panel__tabs" role="tablist" aria-label="Editor panels">
        <button
          type="button"
          role="tab"
          id="editor-tab-files"
          aria-selected={tab === "files"}
          aria-controls="editor-panel-files"
          tabIndex={tab === "files" ? 0 : -1}
          className={`editor-panel__tab${tab === "files" ? " is-active" : ""}`}
          onClick={() => setTab("files")}
        >
          Files
        </button>
        <button
          type="button"
          role="tab"
          id="editor-tab-git"
          aria-selected={tab === "git"}
          aria-controls="editor-panel-git"
          tabIndex={tab === "git" ? 0 : -1}
          className={`editor-panel__tab${tab === "git" ? " is-active" : ""}`}
          onClick={() => setTab("git")}
        >
          Git
        </button>
      </div>

      {tab === "files" ? (
        <div
          className="editor-panel__browser"
          role="tabpanel"
          id="editor-panel-files"
          aria-labelledby="editor-tab-files"
        >
          <FileTree key={filesVersion} onOpenFile={onOpenFile} activePath={activePath} />
        </div>
      ) : (
        <div
          className="editor-panel__git"
          role="tabpanel"
          id="editor-panel-git"
          aria-labelledby="editor-tab-git"
        >
          <GitViewer onBranchChanged={onBranchChanged} />
        </div>
      )}
    </aside>
  );
}