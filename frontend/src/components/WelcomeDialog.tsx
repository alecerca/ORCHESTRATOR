import { useEffect } from "react";

interface Props {
  open: boolean;
  /** Live connection state from the store. */
  connected: boolean;
  /** Effective server base URL, shown in the status line. */
  baseUrl: string;
  /** Persist the default local config (or just dismiss when already connected). */
  onPrimary: () => void;
  /** Open the Join workspace dialog for a shared server. */
  onJoin: () => void;
  /** Dismiss without connecting; the caller marks first run as seen. */
  onClose: () => void;
}

const STEPS: ReadonlyArray<{ title: string; body: string }> = [
  {
    title: "Connect to opencode serve",
    body: "Orquestador drives a local OpenCode server. It starts pointing at http://127.0.0.1:4096.",
  },
  {
    title: "Instruct the orchestrator",
    body: "Type a goal in the composer at the bottom. The orchestrator spawns the sub-agents it needs.",
  },
  {
    title: "Watch, steer, inspect",
    body: "Each agent streams live in its own pane. Approve actions, send follow-ups, and open files or diffs on the right.",
  },
];

/**
 * First-run Welcome. Shown once, before anything has been configured: it frames
 * what Orquestador is, shows the live connection state, and offers the two
 * entry paths — connect to a local server (the default persona) or join a shared
 * workspace (the cowork persona). The cowork-only "Join a workspace" form is now
 * secondary instead of the forced first screen.
 */
export function WelcomeDialog({ open, connected, baseUrl, onPrimary, onJoin, onClose }: Props) {
  useEffect(() => {
    if (!open) return;
    function onKey(event: KeyboardEvent): void {
      if (event.key === "Escape") onClose();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open) return null;

  return (
    <div
      className="modal-backdrop"
      onClick={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div
        className="modal welcome"
        role="dialog"
        aria-modal="true"
        aria-labelledby="welcome-title"
      >
        <div className="welcome__brand">
          <span className="welcome__logo" aria-hidden>
            ◆
          </span>
          <span className="welcome__wordmark">Orquestador</span>
        </div>
        <h2 className="welcome__title" id="welcome-title">
          Welcome to Orquestador
        </h2>
        <p className="welcome__lead">
          Your control tower for OpenCode sub-agents — orchestrate, watch, and
          steer them from one window.
        </p>

        <p className={`welcome__status${connected ? " is-up" : " is-down"}`}>
          <span className={`dot ${connected ? "dot--up" : "dot--down"}`} aria-hidden />
          {connected ? (
            <>
              Connected to <code>{baseUrl}</code>
            </>
          ) : (
            <>
              Not connected yet — start <code>opencode serve</code>, then connect.
            </>
          )}
        </p>

        <ol className="welcome__steps">
          {STEPS.map((step) => (
            <li key={step.title} className="welcome__step">
              <span className="welcome__step-title">{step.title}</span>
              <span className="welcome__step-body">{step.body}</span>
            </li>
          ))}
        </ol>

        <div className="welcome__actions">
          <button type="button" className="btn btn--primary" onClick={onPrimary}>
            {connected ? "Get started" : "Connect to local server"}
          </button>
          <button type="button" className="btn" onClick={onJoin}>
            Join a shared workspace
          </button>
          <button type="button" className="btn btn--ghost" onClick={onClose}>
            Skip
          </button>
        </div>
      </div>
    </div>
  );
}
