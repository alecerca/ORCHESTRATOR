import { useEffect, useState } from "react";
import { readFile } from "../commands";
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
 * states. No write API — the editor's in-memory `onChange` stays local to the
 * running window (the previous right-column viewer behaved the same way).
 */
export function FileViewer({ path, theme, filesVersion }: Props) {
  const [content, setContent] = useState<FileContent | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Reload whenever the active file tab changes (the component instance is
  // reused across tabs within a group) or a checkout switched the working tree.
  useEffect(() => {
    let cancelled = false;
    setContent(null);
    setLoading(true);
    setError(null);
    void readFile(path)
      .then((next) => {
        if (!cancelled) setContent(next);
      })
      .catch((cause) => {
        if (!cancelled) setError(describeError(cause));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [path, filesVersion]);

  return (
    <div className="file-viewer">
      {error && <p className="file-viewer__error">{error}</p>}
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
          onChange={(value) =>
            setContent((current) =>
              current && current.type === "text" ? { ...current, content: value } : current,
            )
          }
        />
      )}
    </div>
  );
}