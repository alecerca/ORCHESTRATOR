import { useEffect, useState } from "react";
import { gitBranch } from "../commands";

/**
 * Module-level cache keyed by directory. Every card and tab that shares a
 * directory issues at most one `git_branch` call per app run. The branch is
 * informational only, so it is fetched once and never re-polled — a branch
 * switched outside the app stays stale until the directory is re-mounted with a
 * cold cache.
 */
const cache = new Map<string, string | null>();

/**
 * The git branch of `directory`, or `null` when unknown (empty directory, not a
 * repository, git unavailable, or the request failed). Fetches once per
 * directory, caches the result, and ignores a stale in-flight response after
 * `directory` changes.
 */
export function useGitBranch(directory: string): string | null {
  const [branch, setBranch] = useState<string | null>(() =>
    directory ? cache.get(directory) ?? null : null,
  );

  useEffect(() => {
    if (!directory) {
      setBranch(null);
      return;
    }
    if (cache.has(directory)) {
      setBranch(cache.get(directory) ?? null);
      return;
    }

    let alive = true;
    // Clear immediately so a previous directory's branch is never shown for the
    // new one while its lookup is still in flight.
    setBranch(null);
    void (async () => {
      let result: string | null = null;
      try {
        result = await gitBranch(directory);
      } catch {
        result = null;
      }
      cache.set(directory, result);
      if (alive) setBranch(result);
    })();

    return () => {
      alive = false;
    };
  }, [directory]);

  return branch;
}
