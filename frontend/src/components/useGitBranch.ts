import { useEffect, useState, useSyncExternalStore } from "react";
import { gitBranch } from "../commands";

/**
 * Module-level cache keyed by directory. Every card and tab that shares a
 * directory issues at most one `git_branch` call per read. The branch is
 * informational only, so it is never polled: it is re-read when
 * [`invalidateGitBranches`] drops the cache after a checkout switched it, and by
 * [`refreshGitBranches`] whenever the window regains focus, because a checkout
 * run outside the app (a terminal, another tool) is otherwise invisible to it.
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
  // Focus is the only signal that a checkout happened outside the app: the cache
  // cannot observe `.git/HEAD`, and the server's file-watcher event is
  // display-only. Tied to the subscriber lifetime so no listener lingers once
  // the last chip unmounts.
  if (listeners.size === 1) {
    window.addEventListener("focus", onWindowFocus);
  }
  return () => {
    listeners.delete(listener);
    if (listeners.size === 0) {
      window.removeEventListener("focus", onWindowFocus);
    }
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
 * Re-read every directory already in the cache and notify only when a value
 * actually changed. The known values stay in place, so a chip is never blanked
 * while the re-read is in flight — unlike [`invalidateGitBranches`], which drops
 * them and makes every mounted hook re-fetch.
 *
 * Bound to window focus: a checkout run outside the app (a terminal, another
 * tool) cannot be observed directly, because the cache cannot see `.git/HEAD`
 * and the server's file-watcher event is display-only.
 */
export async function refreshGitBranches(): Promise<void> {
  const directories = [...cache.keys()];
  const results = await Promise.all(
    directories.map(async (directory) => {
      let next: string | null = null;
      try {
        next = await gitBranch(directory);
      } catch {
        next = null;
      }
      return [directory, next] as const;
    }),
  );

  let changed = false;
  for (const [directory, next] of results) {
    if (cache.get(directory) !== next) {
      cache.set(directory, next);
      changed = true;
    }
  }
  // Notify only on a real change: an unchanged re-read must not re-render every
  // chip on every focus.
  if (changed) notify();
}

/** Window-focus handler: re-read the branches that are already on screen. */
function onWindowFocus(): void {
  void refreshGitBranches();
}

/**
 * The git branch of `directory`, or `null` when unknown (empty directory, not a
 * repository, git unavailable, or the request failed). Fetches once per
 * directory, caches the result, and ignores a stale in-flight response after
 * `directory` changes. Re-reads after [`invalidateGitBranches`] and on window
 * focus, so a checkout made outside the app shows up.
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
