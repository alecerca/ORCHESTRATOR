import { useState } from "react";
import { sendMessageAsync } from "../commands";
import { describeError } from "../utils";

interface Props {
  /** The orchestrator (root) session that receives the instruction. */
  sessionId: string | null;
}

/** Terminal-style instruction box for the orchestrator. */
export function Composer({ sessionId }: Props) {
  const [text, setText] = useState("");
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const canSend = sessionId !== null && !sending && text.trim().length > 0;

  async function send(): Promise<void> {
    const value = text.trim();
    if (!value || !sessionId || sending) return;
    setSending(true);
    setError(null);
    try {
      await sendMessageAsync(sessionId, value);
      setText("");
    } catch (cause) {
      setError(describeError(cause));
    } finally {
      setSending(false);
    }
  }

  return (
    <div className="composer">
      <span className="composer__prompt" aria-hidden>
        ›
      </span>
      <textarea
        className="composer__input"
        rows={2}
        value={text}
        disabled={!sessionId}
        placeholder={
          sessionId
            ? "Instruct the orchestrator… (Enter to send, Shift+Enter for a new line)"
            : "Waiting for an orchestrator session…"
        }
        onChange={(event) => setText(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter" && !event.shiftKey) {
            event.preventDefault();
            void send();
          }
        }}
      />
      <button
        type="button"
        className="btn btn--primary"
        disabled={!canSend}
        onClick={() => void send()}
      >
        {sending ? "Sending…" : "Send"}
      </button>
      {error && <p className="composer__error">{error}</p>}
    </div>
  );
}
