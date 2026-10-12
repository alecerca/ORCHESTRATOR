import { useEffect, useState, useSyncExternalStore } from "react";
import { gitBranch } from "../commands";

/**
 * Module-level cache keyed by directory. Every card and tab that shares a
 * directory issues at most one `git_branch` call per app run. The branch is
 * informational only, so it is fetched once and never re-polled — except when
 * [`invalidateGitBranches`] drops the cache after a checkout switched it.
 */
const cache = new Map<string, string | null>();

/**
 * A version counter bumped on every cache change: a completed fetch, or an
 * explicit invalidation. `useSyncExternalStore` subscribes to it so an
 * invalidation re-runs every mounted hook's effect and re-reads its directory.
 */
let version = 0;
const listeners = new Set<() => void>();

function notify(): void {
  version += 1;
  for (const listener of listeners) listener();
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

function getVersion(): number {
  return version;
}

/**
 * Drop the whole branch cache and notify every mounted hook, so session chips
 * re-read the branch after a checkout switched it. The next read of a directory
 * fetches it again.
 */
export function invalidateGitBranches(): void {
  cache.clear();
  notify();
}

/**
 * The git branch of `directory`, or `null` when unknown (empty directory, not a
 * repository, git unavailable, or the request failed). Fetches once per
 * directory, caches the result, and ignores a stale in-flight response after
 * `directory` changes. Re-reads after [`invalidateGitBranches`].
 */
export function useGitBranch(directory: string): string | null {
  // Subscribe to the cache version so an invalidation re-runs the effect below.
  const cacheVersion = useSyncExternalStore(subscribe, getVersion);
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
      notify();
      if (alive) setBranch(result);
    })();

    return () => {
      alive = false;
    };
  }, [directory, cacheVersion]);

  return branch;
}
