import { useCallback, useEffect, useState } from "react";
import { readFile, writeFile } from "../commands";
import {
  clearFileDraft,
  markFileSaved,
  setFileContent,
  setFileDraft,
} from "../fileDrafts";
import type { FileContent } from "../types";
import { describeError } from "../utils";
import { MonacoEditor } from "./MonacoEditor";

interface Props {
  path: string;
  theme: "dark" | "light";
  /** Bumped after a checkout so an open tab re-reads the file on the new branch. */
  filesVersion: number;
}

/**
 * One open file document in a group: loads through `readFile` and renders
 * `MonacoEditor` for text files, with the existing loading / binary / error
 * states. Text content is mirrored into the file-draft store so dirtiness is
 * visible in the tab strip; Ctrl/Cmd+S (via the editor) persists it back
 * through `write_file` and clears the dirty state.
 */
export function FileViewer({ path, theme, filesVersion }: Props) {
  const [content, setContent] = useState<FileContent | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);

  // Reload whenever the active file tab changes (the component instance is
  // reused across tabs within a group) or a checkout switched the working tree.
  useEffect(() => {
    let cancelled = false;
    setContent(null);
    setLoading(true);
    setError(null);
    setSaveError(null);
    void readFile(path)
      .then((next) => {
        if (cancelled) return;
        setContent(next);
        // Only text is editable: seed the draft so the tab strip and the close
        // prompt can see dirtiness. A binary load has nothing to save.
        if (next.type === "text") setFileDraft(path, next.content, next.content);
        else clearFileDraft(path);
      })
      .catch((cause) => {
        if (cancelled) return;
        setError(describeError(cause));
        clearFileDraft(path);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [path, filesVersion]);

  /** Persist the current text content; Ctrl/Cmd+S routes here. */
  const save = useCallback(async () => {
    if (content?.type !== "text") return;
    try {
      await writeFile(path, content.content);
      markFileSaved(path);
      setSaveError(null);
    } catch (cause) {
      setSaveError(describeError(cause));
    }
  }, [path, content]);

  return (
    <div className="file-viewer">
      {error && <p className="file-viewer__error">{error}</p>}
      {saveError && <p className="file-viewer__error">{saveError}</p>}
      {loading && <p className="file-viewer__muted">Loading…</p>}
      {!loading && !error && content?.type === "binary" && (
        <p className="file-viewer__muted">
          Binary file ({content.mimeType ?? "unknown MIME"}). Preview not
          supported.
        </p>
      )}
      {!loading && !error && content?.type === "text" && (
        <MonacoEditor
          path={path}
          value={content.content}
          theme={theme}
          onSave={save}
          onChange={(value) => {
            setFileContent(path, value);
            setContent((current) =>
              current && current.type === "text" ? { ...current, content: value } : current,
            );
          }}
        />
      )}
    </div>
  );
}
