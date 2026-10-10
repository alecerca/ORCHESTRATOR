import { useState } from "react";
import { abortSession } from "../commands";
import type { SessionSnapshot } from "../types";
import { describeError, shortId } from "../utils";
import { FollowUpInput } from "./FollowUpInput";
import { MessageStream } from "./MessageStream";
import { StatusChip } from "./StatusChip";

interface Props {
  sessionId: string;
  snapshot?: SessionSnapshot;
  onClose: (sessionId: string) => void;
}

/** One open agent transcript: header + live stream + follow-up input. */
export function AgentPane({ sessionId, snapshot, onClose }: Props) {
  const [error, setError] = useState<string | null>(null);
  const status = snapshot?.status ?? "idle";
  const canAbort = status === "running" || status === "waiting_approval";

  async function abort(): Promise<void> {
    setError(null);
    try {
      await abortSession(sessionId);
    } catch (cause) {
      setError(describeError(cause));
    }
  }

  return (
    <section className="pane">
      <header className="pane__head">
        <span className="pane__title" title={sessionId}>
          {snapshot?.session.title || shortId(sessionId)}
        </span>
        <StatusChip status={status} />
        <div className="pane__actions">
          {canAbort && (
            <button
              type="button"
              className="btn btn--danger btn--xs"
              onClick={() => void abort()}
              title="Abort this agent"
            >
              Abort
            </button>
          )}
          <button
            type="button"
            className="btn btn--ghost btn--xs"
            onClick={() => onClose(sessionId)}
            title="Close pane"
          >
            ✕
          </button>
        </div>
      </header>
      {error && <p className="pane__error">{error}</p>}
      <MessageStream sessionId={sessionId} />
      <FollowUpInput sessionId={sessionId} />
    </section>
  );
}