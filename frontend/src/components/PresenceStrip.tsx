import { useEffect, useState } from "react";
import { getPresence } from "../commands";
import { effectiveMember } from "../identity";
import { useServerConfig } from "../serverConfig";
import type { PresenceEntry } from "../types";

/** How often presence is re-read. This poll is also the heartbeat: it is what
 * keeps this client visible to the others, so the interval must keep running
 * while the app is open. */
const POLL_MS = 15_000;

/** Compact "last active" hint for a member's tooltip, from its idle time. */
function idleLabel(ms: number): string {
  if (ms < 60_000) return "just now";
  const minutes = Math.floor(ms / 60_000);
  if (minutes < 60) return `${minutes}m ago`;
  return `${Math.floor(minutes / 60)}h ago`;
}

/**
 * Compact "who is online" strip for the top bar. Polls `get_presence` on mount
 * and every 15 s; there is no separate heartbeat, so this poll is what keeps
 * the client itself visible to the others.
 *
 * The strip is best-effort: it renders nothing while the list is empty
 * (presence off / no proxy) and a failed poll keeps the last good list, so a
 * hiccup never becomes a visible error.
 */
export function PresenceStrip() {
  const config = useServerConfig();
  const me = effectiveMember(config);
  const [entries, setEntries] = useState<PresenceEntry[]>([]);

  useEffect(() => {
    let cancelled = false;
    let inFlight = false;

    async function poll(): Promise<void> {
      // Skip overlapping polls so a slow request cannot pile up or resolve
      // out of order.
      if (inFlight) return;
      inFlight = true;
      try {
        const next = await getPresence();
        if (!cancelled) setEntries(next);
      } catch {
        // Presence is best-effort: keep the last good list and stay silent.
      } finally {
        inFlight = false;
      }
    }

    void poll();
    const id = window.setInterval(() => void poll(), POLL_MS);
    return () => {
      cancelled = true;
      window.clearInterval(id);
    };
  }, []);

  if (entries.length === 0) return null;

  return (
    <div className="presence" role="group" aria-label="Members online">
      {entries.map((entry) => {
        const you = entry.name === me;
        return (
          <span
            className="presence__member"
            key={entry.name}
            title={`Last active ${idleLabel(entry.idleMs)}`}
          >
            <span className="presence__dot" aria-hidden />
            <span className="presence__name">{entry.name}</span>
            {you && <span className="presence__you">(you)</span>}
            <span className="presence__role">{entry.role}</span>
          </span>
        );
      })}
    </div>
  );
}
