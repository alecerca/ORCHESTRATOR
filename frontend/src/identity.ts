// Identity + session-title attribution helpers (Phase 1 of the cowork feature).
//
// The effective member rule (`memberName` -> `username` -> "opencode") is the
// same on both sides of the app: the Rust core uses its own chain for the root
// session, the webview uses this module for the sessions it creates.
import type { ServerConfig, SessionSnapshot } from "./types";

/** Final fallback when neither `memberName` nor `username` is configured. */
export const DEFAULT_MEMBER = "opencode";

/**
 * Effective member (display) name of this app instance.
 * `memberName` -> `username` -> `"opencode"`; always yields a value.
 */
export function effectiveMember(cfg: ServerConfig): string {
  return cfg.memberName?.trim() || cfg.username?.trim() || DEFAULT_MEMBER;
}

/**
 * Session title with the creator's identity: `[<member>] <base>`.
 * A blank member leaves the base title untouched.
 */
export function attributedTitle(member: string, base: string): string {
  const trimmed = member.trim();
  return trimmed ? `[${trimmed}] ${base}` : base;
}

/** Tolerant owner prefix: `^\[([^\]]+)\]\s+` (e.g. `[alice] New session`). */
const OWNER_RE = /^\[([^\]]+)\]\s+/;

/**
 * Owner captured from a session title's `[name]` prefix, or `null` when the
 * title carries none (e.g. sessions created by another tool).
 */
export function parseOwner(title: string | undefined | null): string | null {
  if (!title) return null;
  const owner = OWNER_RE.exec(title)?.[1]?.trim();
  return owner ? owner : null;
}

/**
 * Resolve the owner of a session's tree: walk `parentID` up to the root
 * ancestor (cycle- and missing-parent-safe), then read the `[name]` prefix of
 * the root title.
 */
export function ownerOf(sessionId: string, sessions: SessionSnapshot[]): string | null {
  const byId = new Map<string, SessionSnapshot>();
  for (const snapshot of sessions) byId.set(snapshot.session.id, snapshot);

  const seen = new Set<string>();
  let current = byId.get(sessionId);
  while (current && current.session.parentID) {
    if (seen.has(current.session.id)) return null; // cycle: no clear owner
    seen.add(current.session.id);
    current = byId.get(current.session.parentID);
  }
  return current ? parseOwner(current.session.title) : null;
}