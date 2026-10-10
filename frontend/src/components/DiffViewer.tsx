import { useEffect, useState } from "react";
import { DiffEditor } from "@monaco-editor/react";
import { getDiff } from "../commands";
import type { FileDiff } from "../types";
import { describeError, languageForPath } from "../utils";

interface Props {
  sessionId: string | null;
  theme: "dark" | "light";
}

/** Side-by-side diff of a session's changes, driven by `get_diff`. */
export function DiffViewer({ sessionId, theme }: Props) {
  const [diffs, setDiffs] = useState<FileDiff[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!sessionId) return;
    let cancelled = false;
    setLoading(true);
    setError(null);
    void (async () => {
      try {
        const result = await getDiff(sessionId);
        if (cancelled) return;
        setDiffs(result);
        setSelected((current) => (current && result.some((d) => d.file === current) ? current : (result[0]?.file ?? null)));
      } catch (cause) {
        if (!cancelled) setError(describeError(cause));
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [sessionId]);

  const current = diffs.find((d) => d.file === selected) ?? null;

  if (!sessionId) {
    return <p className="diffviewer__muted">No orchestrator session — nothing to diff.</p>;
  }

  return (
    <div className="diffviewer">
      <div className="diffviewer__files">
        {error && <p className="diffviewer__error">{error}</p>}
        {loading && <p className="diffviewer__muted">Loading diff…</p>}
        {!loading && diffs.length === 0 && !error && (
          <p className="diffviewer__muted">No file changes on this session yet.</p>
        )}
        {!loading &&
          diffs.map((diff) => (
            <button
              key={diff.file}
              type="button"
              className={`tnode${diff.file === selected ? " tnode--active" : ""}`}
              onClick={() => setSelected(diff.file)}
              title={`+${diff.additions} / -${diff.deletions}`}
            >
              <span className="tnode__name">{diff.file}</span>
              <span className="tnode__count">
                +{diff.additions} −{diff.deletions}
              </span>
            </button>
          ))}
      </div>
      <div className="diffviewer__editor">
        {current ? (
          <DiffEditor
            theme={theme === "dark" ? "vs-dark" : "vs"}
            original={current.before}
            modified={current.after}
            language={languageForPath(current.file)}
            options={{
              readOnly: true,
              minimap: { enabled: false },
              fontSize: 13,
              scrollBeyondLastLine: false,
              wordWrap: "on",
              automaticLayout: true,
            }}
          />
        ) : (
          <p className="diffviewer__muted">Select a file to see its diff.</p>
        )}
      </div>
    </div>
  );
}