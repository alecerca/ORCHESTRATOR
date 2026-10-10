import type { SessionSnapshot } from "../types";
import { AgentPane } from "./AgentPane";

interface Props {
  panes: string[];
  sessions: SessionSnapshot[];
  onClosePane: (sessionId: string) => void;
}

/** The centre transcript area — one pane per open session id. */
export function PaneGrid({ panes, sessions, onClosePane }: Props) {
  if (panes.length === 0) {
    return (
      <div className="pane-grid pane-grid--empty">
        <p>No panes open.</p>
        <p className="pane-grid__hint">
          Open an agent from the panel on the left to watch its live stream.
        </p>
      </div>
    );
  }

  return (
    <div className="pane-grid">
      {panes.map((sessionId) => (
        <AgentPane
          key={sessionId}
          sessionId={sessionId}
          snapshot={sessions.find((s) => s.session.id === sessionId)}
          onClose={onClosePane}
        />
      ))}
    </div>
  );
}