import { useEffect, useState } from "react";
import { describeError } from "../utils";

interface Props {
  open: boolean;
  /** Provider id the key is stored for (`PUT /auth/:providerID`). */
  providerId: string;
  /** Provider display name, shown to the user. */
  providerName: string;
  /**
   * Store the key. Resolves on success (the dialog closes); rejects with the
   * server's reason when the key is refused.
   */
  onSubmit: (key: string) => Promise<void>;
  onClose: () => void;
}

/**
 * Small dialog to enter a provider's API key.
 *
 * SECURITY: the key is write-only and lives only here while the dialog is
 * open. It is never persisted (no `localStorage`), never logged, and never
 * echoed back — after submission it is only ever held by the calling command.
 * The transient state is cleared whenever the dialog closes.
 */
export function ApiKeyDialog({
  open,
  providerId,
  providerName,
  onSubmit,
  onClose,
}: Props) {
  const [apiKey, setApiKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Never keep a secret (or a stale error) around once the dialog closes.
  useEffect(() => {
    if (open) return;
    setApiKey("");
    setBusy(false);
    setError(null);
  }, [open]);

  // Close on Escape while open.
  useEffect(() => {
    if (!open) return;
    function onKey(event: KeyboardEvent) {
      if (event.key === "Escape") onClose();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open) return null;

  async function handleSubmit() {
    // The key is sent verbatim; only a fully empty value is rejected.
    if (apiKey.length === 0) {
      setError("Enter an API key.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await onSubmit(apiKey);
    } catch (cause) {
      setError(describeError(cause));
      setBusy(false);
      return;
    }
    // Drop the secret before the parent unmounts the dialog.
    setApiKey("");
    setBusy(false);
    onClose();
  }

  return (
    <div
      className="modal-backdrop"
      onClick={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div
        className="modal"
        role="dialog"
        aria-modal="true"
        aria-label={`Add API key for ${providerName}`}
      >
        <h2 className="approval__title">Add API key</h2>
        <p className="api-key-dialog__lead">
          Enter an API key for <strong>{providerName}</strong> (
          <code>{providerId}</code>). The key is stored by the OpenCode server
          and is never shown again.
        </p>
        <div className="settings-fields">
          <label className="settings-field">
            <span>API key</span>
            <input
              className="settings-input"
              type="password"
              value={apiKey}
              onChange={(event) => setApiKey(event.target.value)}
              placeholder="sk-…"
              autoComplete="off"
              autoFocus
              onKeyDown={(event) => {
                if (event.key === "Enter") void handleSubmit();
              }}
            />
          </label>
        </div>
        {error && (
          <p className="approval__error" role="alert">
            {error}
          </p>
        )}
        <div className="approval__actions">
          <button type="button" className="btn btn--ghost" onClick={onClose}>
            Cancel
          </button>
          <button
            type="button"
            className="btn btn--primary"
            disabled={busy}
            onClick={() => void handleSubmit()}
          >
            {busy ? "Connecting…" : "Connect"}
          </button>
        </div>
      </div>
    </div>
  );
}
