import { useEffect, useState } from "react";
import { getConfig, setConfig } from "../serverConfig";
import { getServerConfig, setServerConfig } from "../commands";
import { describeError } from "../utils";

interface Props {
  open: boolean;
  onClose: () => void;
}

/** Connection default shown before anything has been persisted. */
const DEFAULT_BASE_URL = "http://127.0.0.1:4096";

/**
 * First-run / "Join a workspace" screen: enter the host's server address, the
 * username + password the owner created, and an optional display name. Saves
 * through the same path as Settings so the joined username drives member
 * identity via the existing `memberName || username || "opencode"` fallback.
 */
export function JoinWorkspaceDialog({ open, onClose }: Props) {
  const [baseUrl, setBaseUrl] = useState(DEFAULT_BASE_URL);
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [memberName, setMemberName] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Reset the form each time we open; seed the address from the local mirror so
  // a reopen shows the current server (default when nothing is persisted).
  useEffect(() => {
    if (!open) return;
    setBaseUrl(getConfig().baseUrl || DEFAULT_BASE_URL);
    setUsername("");
    setPassword("");
    setMemberName("");
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

  async function handleJoin() {
    setSaving(true);
    setError(null);
    try {
      // `set_server_config` overwrites `proxy_config_path`, and this dialog does
      // not own it: read the persisted value so joining never clobbers the
      // host's Members configuration.
      const persisted = await getServerConfig().catch(() => getConfig());
      const proxyConfigPath = persisted.proxyConfigPath ?? undefined;
      const cfg = await setServerConfig(
        baseUrl,
        username,
        password,
        memberName,
        proxyConfigPath,
      );
      // Mirror the persisted config locally so the UI is consistent offline.
      setConfig({
        baseUrl: cfg.baseUrl,
        username: cfg.username,
        password: cfg.password ?? undefined,
        memberName: cfg.memberName,
        proxyConfigPath: cfg.proxyConfigPath,
      });
      onClose();
    } catch (e) {
      setError(describeError(e));
    } finally {
      setSaving(false);
    }
  }

  const canJoin = baseUrl.trim().length > 0 && username.trim().length > 0;

  return (
    <div
      className="modal-backdrop"
      onClick={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div className="modal" role="dialog" aria-modal="true" aria-label="Join a workspace">
        <h2 className="approval__title">Join a workspace</h2>
        <p className="join-hint">
          Your username becomes your identity on shared sessions, so teammates
          can tell your work apart.
        </p>
        <div className="settings-fields">
          <label className="settings-field">
            <span>Server address</span>
            <input
              className="settings-input"
              value={baseUrl}
              onChange={(e) => setBaseUrl(e.target.value)}
              placeholder={DEFAULT_BASE_URL}
              autoFocus
            />
          </label>
          <label className="settings-field">
            <span>Username</span>
            <input
              className="settings-input"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              placeholder="your username"
              autoComplete="username"
            />
          </label>
          <label className="settings-field">
            <span>Password</span>
            <input
              className="settings-input"
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              autoComplete="current-password"
            />
          </label>
          <label className="settings-field">
            <span>Display name (optional — shown on your sessions)</span>
            <input
              className="settings-input"
              value={memberName}
              onChange={(e) => setMemberName(e.target.value)}
              placeholder="your username"
            />
          </label>
        </div>
        {error && <p className="approval__error">{error}</p>}
        <div className="approval__actions">
          <button type="button" className="btn btn--ghost" disabled={saving} onClick={onClose}>
            Cancel
          </button>
          <button
            type="button"
            className="btn btn--primary"
            disabled={saving || !canJoin}
            onClick={() => void handleJoin()}
          >
            {saving ? "Saving…" : "Join"}
          </button>
        </div>
      </div>
    </div>
  );
}
