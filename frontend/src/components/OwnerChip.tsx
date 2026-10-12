interface Props {
  /** Session tree owner, or `null` when it cannot be determined. */
  owner: string | null;
}

/**
 * Inline "by <owner>" indicator for a session, mirroring the branch chip
 * pattern. Renders nothing when the owner is unknown (no `[name]` prefix in the
 * root title, e.g. sessions created by another tool).
 */
export function OwnerChip({ owner }: Props) {
  if (!owner) return null;

  return (
    <span className="agent-card__owner" title="Session owner">
      by <strong>{owner}</strong>
    </span>
  );
}