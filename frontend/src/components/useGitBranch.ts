import { useEffect, useState, useSyncExternalStore } from "react";
import { gitBranch } from "../commands";

/**
 * Module-level cache keyed by directory. Every card and tab that shares a
 * directory issues at most one `git_branch` call per read. A checkout run
 * outside the app (a terminal, another tool) is invisible to the cache, so the
 * branch is re-read by [`refreshGitBranches`] on window focus and on a short
 * interval while at least one chip is mounted; [`invalidateGitBranches`] drops
 * the cache after a checkout switched it from inside the app.
 */
const cache = new Map<string, string | null>();

/**
 * How often a mounted chip re-reads the branch, in milliseconds. Matches the
 * diff panel's cadence so an external checkout shows up on its own.
 */
const POLL_MS = 3000;

/**
 * A version counter bumped on every cache change: a completed fetch, or an
 * explicit invalidation. `useSyncExternalStore` subscribes to it so an
 * invalidation re-runs every mounted hook's effect and re-reads its directory.
 */
let version = 0;
const listeners = new Set<() => void>();
/** The poll timer, alive only while at least one chip is subscribed. */
let pollTimer: number | null = null;

function notify(): void {
  version += 1;
  for (const listener of listeners) listener();
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  // A checkout run outside the app leaves the cached branch stale: the cache
  // cannot observe `.git/HEAD` and the server's file-watcher event is
  // display-only. Focus is the instant signal, but it never fires while the
  // window keeps it (a terminal checkout, another tool doing the switch), so the
  // interval is what makes the chip update on its own. Both are tied to the
  // subscriber lifetime, so nothing keeps running once the last chip unmounts.
  if (listeners.size === 1) {
    window.addEventListener("focus", onRefreshSignal);
    pollTimer = window.setInterval(onRefreshSignal, POLL_MS);
  }
  return () => {
    listeners.delete(listener);
    if (listeners.size === 0) {
      window.removeEventListener("focus", onRefreshSignal);
      if (pollTimer !== null) {
        window.clearInterval(pollTimer);
        pollTimer = null;
      }
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

/** Whether a re-read is already in flight, so a slow git call cannot stack. */
let refreshing = false;

/**
 * Re-read every directory already in the cache and notify only when a value
 * actually changed. The known values stay in place, so a chip is never blanked
 * while the re-read is in flight — unlike [`invalidateGitBranches`], which drops
 * them and makes every mounted hook re-fetch.
 *
 * Called on window focus and on the poll interval, because a checkout run
 * outside the app cannot be observed directly: the cache cannot see `.git/HEAD`
 * and the server's file-watcher event is display-only.
 */
export async function refreshGitBranches(): Promise<void> {
  // One poll must not stack on a previous one that is still waiting on git.
  if (refreshing) return;
  refreshing = true;
  try {
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
    // chip on every focus or poll.
    if (changed) notify();
  } finally {
    refreshing = false;
  }
}

/** Focus or poll tick: re-read the branches that are already on screen. */
function onRefreshSignal(): void {
  void refreshGitBranches();
}

/**
 * The git branch of `directory`, or `null` when unknown (empty directory, not a
 * repository, git unavailable, or the request failed). Fetches once per
 * directory, caches the result, and ignores a stale in-flight response after
 * `directory` changes. Re-reads after [`invalidateGitBranches`], on window focus
 * and on the poll interval, so a checkout made outside the app shows up on its
 * own.
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
