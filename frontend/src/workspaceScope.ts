import { useSyncExternalStore } from "react";
import { getWorkspaceScope } from "./commands";

/**
 * The project the workspace is pinned to (absolute path), or `null` when the
 * app decides its own scope. The pin belongs to the proxy, which reads it from
 * `proxy.json` at startup — so one read serves the whole session, and a changed
 * pin needs a proxy restart and an app reload anyway.
 *
 * The read is kicked off by the first subscriber and degrades to `null` on any
 * failure, so a direct (proxy-less) connection simply leaves the app in charge.
 */
let scope: string | null = null;
let requested = false;
const listeners = new Set<() => void>();

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  if (!requested) {
    requested = true;
    void (async () => {
      try {
        scope = await getWorkspaceScope();
      } catch {
        // Leave the door open for a later mount to retry (e.g. the bridge was
        // still starting); the app just shows an unlocked picker in the meantime.
        requested = false;
        scope = null;
      }
      for (const notify of [...listeners]) notify();
    })();
  }
  return () => listeners.delete(listener);
}

function getSnapshot(): string | null {
  return scope;
}

export function useWorkspaceScope(): string | null {
  return useSyncExternalStore(subscribe, getSnapshot);
}