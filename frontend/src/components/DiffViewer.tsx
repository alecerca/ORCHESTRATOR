import { DiffEditor } from "@monaco-editor/react";
import { useEffect, useState } from "react";
import { gitShowFile, readFile } from "../commands";
import type { FileContent, HeadFile } from "../types";
import { describeError, languageForPath } from "../utils";

/** How often the modified (working-tree) side is re-read. */
const POLL_MS = 3000;

const BINARY_PLACEHOLDER = "Binary file — no diff preview";

interface Props {
  path: string;
  theme: "dark" | "light";
}

/**
 * One working-tree diff document in a group: the path's HEAD revision (read
 * once) against its current working-tree content (polled), rendered with
 * Monaco's diff editor. The HEAD side is stable, so only the modified side is
 * re-read.
 */
export function DiffViewer({ path, theme }: Props) {
  const [head, setHead] = useState<HeadFile | null>(null);
  const [modified, setModified] = useState<FileContent | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Original (HEAD) side: fetched once per path.
  useEffect(() => {
    let cancelled = false;
    setHead(null);
    setError(null);
    void gitShowFile(path)
      .then((next) => {
        if (!cancelled) setHead(next);
      })
      .catch((cause) => {
        if (!cancelled) setError(describeError(cause));
      });
    return () => {
      cancelled = true;
    };
  }, [path]);

  // Modified (working-tree) side: polled while mounted so an edit made
  // elsewhere shows up in the diff.
  useEffect(() => {
    let cancelled = false;
    setModified(null);
    setLoading(true);
    const load = async () => {
      try {
        const next = await readFile(path);
        if (!cancelled) {
          setModified(next);
          setError(null);
        }
      } catch (cause) {
        if (!cancelled) setError(describeError(cause));
      } finally {
        if (!cancelled) setLoading(false);
      }
    };
    void load();
    const timer = window.setInterval(() => void load(), POLL_MS);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [path]);

  // The original side of a text diff is the HEAD content; binary and missing
  // (untracked) HEAD sides get a placeholder so the editor never shows a lie.
  const originalText =
    head?.kind === "text"
      ? head.content ?? ""
      : head?.kind === "binary"
        ? BINARY_PLACEHOLDER
        : "";

  return (
    <div className="diff-viewer">
      {error && <p className="diff-viewer__error">{error}</p>}
      {loading && !modified && <p className="diff-viewer__muted">Loading…</p>}
      {!loading && !error && modified?.type === "binary" && (
        <p className="diff-viewer__muted">{BINARY_PLACEHOLDER}</p>
      )}
      {!loading && !error && modified?.type === "text" && (
        <div className="diff-viewer__editor">
          <DiffEditor
            original={originalText}
            modified={modified.content}
            language={languageForPath(path)}
            theme={theme === "dark" ? "vs-dark" : "vs"}
            options={{
              readOnly: true,
              originalEditable: false,
              minimap: { enabled: false },
              fontSize: 13,
              scrollBeyondLastLine: false,
              wordWrap: "on",
              automaticLayout: true,
            }}
          />
        </div>
      )}
    </div>
  );
}