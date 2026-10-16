import { useState } from "react";
import { sendMessageAsync } from "../commands";
import { getSelectedNames } from "../skills";
import { getModel } from "../model";
import { describeError } from "../utils";

interface Props {
  sessionId: string;
}

/** In-pane follow-up input for a single sub-agent. */
export function FollowUpInput({ sessionId }: Props) {
  const [text, setText] = useState("");
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const canSend = !sending && text.trim().length > 0;

  async function send(): Promise<void> {
    const value = text.trim();
    if (!value || sending) return;
    setSending(true);
    setError(null);
    try {
      await sendMessageAsync(
        sessionId,
        value,
        undefined,
        getSelectedNames(),
        getModel(),
      );
      setText("");
    } catch (cause) {
      setError(describeError(cause));
    } finally {
      setSending(false);
    }
  }

  return (
    <div className="followup">
      <input
        type="text"
        className="followup__input"
        aria-label="Follow-up message to this agent"
        placeholder="Send a follow-up to this agent… (Enter)"
        value={text}
        onChange={(event) => setText(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            event.preventDefault();
            void send();
          }
        }}
      />
      <button
        type="button"
        className="btn btn--xs"
        disabled={!canSend}
        onClick={() => void send()}
      >
        {sending ? "…" : "Send"}
      </button>
      {error && (
        <p className="followup__error" role="alert">
          {error}
        </p>
      )}
    </div>
  );
}