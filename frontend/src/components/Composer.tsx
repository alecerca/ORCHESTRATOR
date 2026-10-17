import { useState } from "react";
import { sendMessageAsync } from "../commands";
import { getSelectedNames } from "../skills";
import { getModel } from "../model";
import { useAppState } from "../store";
import { describeError } from "../utils";

interface Props {
  /** The orchestrator (root) session that receives the instruction. */
  sessionId: string | null;
}

/** Terminal-style instruction box for the orchestrator. */
export function Composer({ sessionId }: Props) {
  const agents = useAppState().agents;
  const [text, setText] = useState("");
  const [agent, setAgent] = useState(""); // "" = server default agent
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const canSend = sessionId !== null && !sending && text.trim().length > 0;

  async function send(): Promise<void> {
    const value = text.trim();
    if (!value || !sessionId || sending) return;
    setSending(true);
    setError(null);
    try {
      await sendMessageAsync(
        sessionId,
        value,
        agent || undefined,
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
    <div className="composer">
      <span className="composer__prompt" aria-hidden>
        ›
      </span>
      <textarea
        className="composer__input"
        rows={2}
        aria-label="Instruction to the orchestrator"
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
      {agents.length > 0 && (
        <select
          className="composer__agent"
          aria-label="Run as agent"
          title="Agent for this message (model is chosen in the top bar)"
          value={agent}
          disabled={!sessionId}
          onChange={(event) => setAgent(event.target.value)}
        >
          <option value="">default agent</option>
          {agents.map((entry) => (
            <option key={entry.name} value={entry.name}>
              {entry.name}
            </option>
          ))}
        </select>
      )}
      <button
        type="button"
        className="btn btn--primary"
        disabled={!canSend}
        onClick={() => void send()}
      >
        {sending ? "Sending…" : "Send"}
      </button>
      {error && (
        <p className="composer__error" role="alert">
          {error}
        </p>
      )}
    </div>
  );
}
