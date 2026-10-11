import { useGitBranch } from "./useGitBranch";

interface Props {
  /** The session's working directory; the branch is derived from it. */
  directory: string;
}

/**
 * Inline "working in <branch>" indicator for a session's directory. Renders
 * nothing when the branch is unknown (not a git repository) or empty. A detached
 * HEAD (git's literal `HEAD`) is shown as `detached`.
 */
export function BranchChip({ directory }: Props) {
  const branch = useGitBranch(directory);
  if (!branch) return null;

  const label = branch === "HEAD" ? "detached" : branch;
  return (
    <span
      className="agent-card__branch"
      title="Git branch of this agent's working directory"
    >
      working in <strong>{label}</strong>
    </span>
  );
}
