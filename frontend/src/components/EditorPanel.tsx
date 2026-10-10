import { useState } from "react";
import { readFile } from "../commands";
import type { FileContent } from "../types";
import { describeError } from "../utils";
import { DiffViewer } from "./DiffViewer";
import { FileTree } from "./FileTree";
import { MonacoEditor } from "./MonacoEditor";

interface Props {
  theme: "dark" | "light";
  /** Effective orchestrator root (persisted pick, else `root_session_id`). */
  rootSessionId: string | null;
}

/** Right column: file browser + in-memory editor, or the session diff. */
export function EditorPanel({ theme, rootSessionId }: Props) {
  const [tab, setTab] = useState<"files" | "diff">("files");
  const [path, setPath] = useState<string | null>(null);
  const [content, setContent] = useState<FileContent | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function openFile(nextPath: string): Promise<void> {
    setPath(nextPath);
    setLoading(true);
    setError(null);
    try {
      const next = await readFile(nextPath);
      setContent(next);
    } catch (cause) {
      setError(describeError(cause));
      setContent(null);
    } finally {
      setLoading(false);
    }
  }

  return (
    <aside className="editor-panel">
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
        <div className="editor-panel__split">
          <FileTree onOpenFile={(p) => void openFile(p)} activePath={path} />
          <div className="editor-panel__editor">
            {error && <p className="editor-panel__error">{error}</p>}
            {loading && <p className="editor-panel__muted">Loading…</p>}
            {!loading && !path && (
              <p className="editor-panel__muted">Select a file to read it.</p>
            )}
            {!loading && path && content?.type === "binary" && (
              <p className="editor-panel__muted">
                Binary file ({content.mimeType ?? "unknown MIME"}). Preview not
                supported.
              </p>
            )}
            {!loading && path && content?.type === "text" && (
              <MonacoEditor
                path={path}
                value={content.content}
                theme={theme}
                onChange={(value) => setContent({ ...content, content: value })}
              />
            )}
          </div>
        </div>
      ) : (
        <div className="editor-panel__diff">
          <DiffViewer sessionId={rootSessionId} theme={theme} />
        </div>
      )}
    </aside>
  );
}