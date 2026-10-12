import { useCallback, useEffect, useState } from "react";
import { gitCommitFiles, gitLog } from "../commands";
import type { GitCommitFile, GitLog } from "../types";
import { describeError } from "../utils";

/** Format an ISO-8601 commit date for the compact commit row. */
function formatDate(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  return date.toLocaleString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

/**
 * Read-only git history for the active project: the current branch, its commits
 * (newest first) and the files each commit changed. Everything is derived by the
 * Rust shell with `git log` / `git show`; the UI never mutates the repository.
 */
export function GitViewer() {
  const [log, setLog] = useState<GitLog | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [files, setFiles] = useState<GitCommitFile[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [filesLoading, setFilesLoading] = useState(false);
  const [filesError, setFilesError] = useState<string | null>(null);

  // Load (or re-load) the log; keep the selection when that commit still
  // exists, otherwise fall back to the newest commit.
  const loadLog = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await gitLog();
      setLog(result);
      setSelected((current) =>
        current && result.commits.some((commit) => commit.hash === current)
          ? current
          : (result.commits[0]?.hash ?? null),
      );
    } catch (cause) {
      setLog(null);
      setSelected(null);
      setError(describeError(cause));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadLog();
  }, [loadLog]);

  // Load the changed files whenever the selected commit changes.
  useEffect(() => {
    if (!selected) {
      setFiles([]);
      setFilesError(null);
      return;
    }
    let cancelled = false;
    setFilesLoading(true);
    setFilesError(null);
    void (async () => {
      try {
        const result = await gitCommitFiles(selected);
        if (!cancelled) setFiles(result);
      } catch (cause) {
        if (!cancelled) setFilesError(describeError(cause));
      } finally {
        if (!cancelled) setFilesLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [selected]);

  const branchLabel = !log
    ? "—"
    : !log.branch || log.branch === "HEAD"
      ? "detached"
      : log.branch;

  return (
    <div className="gitviewer">
      <div className="gitviewer__header">
        <span className="gitviewer__branch" title={branchLabel}>
          {branchLabel}
        </span>
        <button
          type="button"
          className="gitviewer__refresh"
          onClick={() => void loadLog()}
          disabled={loading}
        >
          Refresh
        </button>
      </div>

      {error ? (
        <p className="gitviewer__error">{error}</p>
      ) : loading && !log ? (
        <p className="gitviewer__muted">Loading history…</p>
      ) : log && log.commits.length === 0 ? (
        <p className="gitviewer__muted">No commits on this branch.</p>
      ) : (
        <>
          <div className="gitviewer__commits">
            {log?.commits.map((commit) => (
              <button
                key={commit.hash}
                type="button"
                className={`tnode gitviewer__commit${
                  commit.hash === selected ? " tnode--active" : ""
                }`}
                onClick={() => setSelected(commit.hash)}
                title={commit.subject}
              >
                <span className="tnode__name">{commit.subject}</span>
                <span className="gitviewer__meta">
                  {commit.short} · {commit.author} · {formatDate(commit.date)}
                </span>
              </button>
            ))}
          </div>
          <div className="gitviewer__files">
            {filesError ? (
              <p className="gitviewer__error">{filesError}</p>
            ) : filesLoading ? (
              <p className="gitviewer__muted">Loading files…</p>
            ) : !selected ? (
              <p className="gitviewer__muted">Select a commit.</p>
            ) : files.length === 0 ? (
              <p className="gitviewer__muted">No files changed in this commit.</p>
            ) : (
              files.map((file) => (
                <div
                  key={file.path}
                  className="tnode gitviewer__file"
                  title={file.path}
                >
                  <span className="gitviewer__status">{file.status}</span>
                  <span className="tnode__name">{file.path}</span>
                </div>
              ))
            )}
          </div>
        </>
      )}
    </div>
  );
}
