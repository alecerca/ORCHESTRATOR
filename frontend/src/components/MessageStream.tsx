import { useEffect, useRef, useState } from "react";
import { getMessages } from "../commands";
import { store, useSessionStream, type StreamMessage } from "../store";
import type { Part, ToolState } from "../types";
import { describeError } from "../utils";

interface Props {
  sessionId: string;
}

const MAX_PRE = 4000;

/**
 * Outputs longer than this start folded: the transcript stays scannable and the
 * full text is still one click away, rather than a screenful of log per call.
 */
const INLINE_OUTPUT_LIMIT = 700;

function lineCount(text: string): number {
  let lines = 1;
  for (let index = 0; index < text.length; index += 1) {
    if (text.charCodeAt(index) === 10) lines += 1;
  }
  return lines;
}

function truncate(text: string, max: number): string {
  return text.length > max ? `${text.slice(0, max)}\n… (${text.length - max} more chars)` : text;
}

/** Live transcript for one session; seeds history then folds live events. */
export function MessageStream({ sessionId }: Props) {
  const stream = useSessionStream(sessionId);
  const [error, setError] = useState<string | null>(null);
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const pinnedRef = useRef(true);

  useEffect(() => {
    let cancelled = false;
    setError(null);
    void (async () => {
      try {
        const history = await getMessages(sessionId, 50);
        if (!cancelled) store.hydrateStream(sessionId, history);
      } catch (cause) {
        if (!cancelled) setError(describeError(cause));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [sessionId]);

  useEffect(() => {
    const el = scrollRef.current;
    if (el && pinnedRef.current) {
      el.scrollTop = el.scrollHeight;
    }
  }, [stream]);

  function onScroll(): void {
    const el = scrollRef.current;
    if (!el) return;
    pinnedRef.current = el.scrollHeight - el.scrollTop - el.clientHeight < 40;
  }

  return (
    <div
      className="stream"
      ref={scrollRef}
      onScroll={onScroll}
      role="log"
      aria-live="polite"
      aria-relevant="additions text"
      aria-label="Agent transcript"
    >
      {error && (
        <p className="stream__error" role="alert">
          History failed: {error}
        </p>
      )}
      {stream.length === 0 && !error && (
        <p className="stream__empty">No messages yet.</p>
      )}
      {stream.map((message) => (
        <MessageItem key={message.info.id} message={message} />
      ))}
    </div>
  );
}

function MessageItem({ message }: { message: StreamMessage }) {
  const { info, parts } = message;
  const isUser = info.role === "user";
  const errorText = info.error?.data?.message ?? info.error?.name;

  return (
    <div className={`msg msg--${isUser ? "user" : "assistant"}`}>
      <div className="msg__role">
        {info.agent ? `${info.agent}` : isUser ? "you" : "agent"}
      </div>
      <div className="msg__body">
        {parts.map((part) => (
          <PartView key={part.id} part={part} />
        ))}
        {errorText && <pre className="part__pre part__pre--error">{errorText}</pre>}
      </div>
    </div>
  );
}

function PartView({ part }: { part: Part }) {
  switch (part.type) {
    case "text":
      return <div className="part part--text">{part.text ?? ""}</div>;
    case "reasoning":
      return (
        <details className="part part--reasoning">
          <summary>reasoning</summary>
          <pre className="part__pre part__pre--muted">{truncate(part.text ?? "", MAX_PRE)}</pre>
        </details>
      );
    case "tool":
      return <ToolPart part={part} />;
    case "subtask":
      return (
        <div className="part part--subtask">
          ↳ {part.description ?? part.prompt ?? "subtask"}
          {part.agent ? <span className="part__agent"> @{part.agent}</span> : null}
        </div>
      );
    case "agent":
      return <div className="part part--agent">⚑ spawn{part.name ? ` ${part.name}` : ""}</div>;
    case "step-start":
      return <div className="part part--divider">step</div>;
    case "step-finish":
      return <div className="part part--divider part--divider-end">done</div>;
    case "file":
      return (
        <div className="part part--file">
          📎 {part.filename ?? "file"}
          {part.mime ? ` (${String(part.mime)})` : ""}
        </div>
      );
    case "retry":
      return <div className="part part--retry">⟳ retry attempt {String(part.attempt ?? "?")}</div>;
    case "snapshot":
      return <div className="part part--file">⧗ snapshot {String(part.snapshot ?? "")}</div>;
    case "patch":
      return <div className="part part--file">✎ patch {String(part.hash ?? "")}</div>;
    case "compaction":
      return <div className="part part--divider">context compacted</div>;
    default:
      return null;
  }
}

function ToolPart({ part }: { part: Part }) {
  const state = part.state as ToolState | undefined;
  const status = state?.status ?? "pending";
  const title = "title" in (state ?? {}) ? (state as { title?: string }).title : undefined;
  const output =
    state && state.status === "completed" ? state.output : undefined;
  const error = state && state.status === "error" ? state.error : undefined;
  const outputLines = output ? lineCount(output) : 0;
  const folded = output !== undefined && output.length > INLINE_OUTPUT_LIMIT;

  return (
    <div className={`part part--tool part--tool-${status}`}>
      <div className="part__tool-head">
        <span className="part__tool-name">{part.tool ?? "tool"}</span>
        <span className={`chip chip--tool-${status}`}>{status}</span>
      </div>
      {title && <div className="part__tool-title">{title}</div>}
      {output && output.length > 0 && folded && (
        <details className="part__tool-details">
          <summary>
            output · {outputLines} {outputLines === 1 ? "line" : "lines"}
          </summary>
          <pre className="part__pre">{truncate(output, MAX_PRE)}</pre>
        </details>
      )}
      {output && output.length > 0 && !folded && <pre className="part__pre">{output}</pre>}
      {error && <pre className="part__pre part__pre--error">{truncate(error, MAX_PRE)}</pre>}
    </div>
  );
}
