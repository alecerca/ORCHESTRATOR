import { useState } from "react";
import { respondPermission } from "../commands";
import { usePendingPermissions } from "../store";
import type { PermissionResponse } from "../types";
import { describeError, safeJson } from "../utils";

/**
 * Permission gate. Driven by `snapshot.permissions` (the Rust core's registry),
 * so it is robust to approval events arriving before the webview was listening.
 */
export function ApprovalDialog() {
  const pending = usePendingPermissions();
  const [remember, setRemember] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const current = pending[0];
  if (!current) return null;

  async function respond(response: PermissionResponse): Promise<void> {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      await respondPermission(current.sessionID, current.id, response, remember ? true : undefined);
      setRemember(false);
    } catch (cause) {
      setError(describeError(cause));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="modal-backdrop">
      <div className="modal approval">
        <div className="approval__head">
          <span className="chip chip--waiting_approval">approval</span>
          <span className="approval__type">{current.type}</span>
          {pending.length > 1 && (
            <span className="approval__queue">+{pending.length - 1} queued</span>
          )}
        </div>
        <h2 className="approval__title">{current.title}</h2>
        {current.metadata !== undefined && (
          <pre className="approval__meta">{safeJson(current.metadata)}</pre>
        )}
        <label className="approval__remember">
          <input
            type="checkbox"
            checked={remember}
            onChange={(event) => setRemember(event.target.checked)}
          />
          Remember this decision for the rest of the session
        </label>
        {error && <p className="approval__error">{error}</p>}
        <div className="approval__actions">
          <button
            type="button"
            className="btn btn--primary"
            disabled={busy}
            onClick={() => void respond("once")}
          >
            Once
          </button>
          <button
            type="button"
            className="btn"
            disabled={busy}
            onClick={() => void respond("always")}
          >
            Always
          </button>
          <button
            type="button"
            className="btn btn--danger"
            disabled={busy}
            onClick={() => void respond("reject")}
          >
            Reject
          </button>
        </div>
      </div>
    </div>
  );
}