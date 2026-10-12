import { useEffect, useState } from "react";
import { getConfig, setConfig } from "../serverConfig";
import { getServerConfig, setServerConfig, testConnection } from "../commands";
import { describeError } from "../utils";

interface Props {
  open: boolean;
  onClose: () => void;
  /** Retarget the app to the Members dialog (App owns both, one at a time). */
  onManageMembers?: () => void;
  /** Retarget the app to the Join workspace dialog (App owns both). */
  onJoinWorkspace?: () => void;
}

/**
 * Server connection settings. Rendered at the app root (like the approval and
 * skill-save modals) so the fixed backdrop can cover the whole window.
 */
export function SettingsDialog({ open, onClose, onManageMembers, onJoinWorkspace }: Props) {
  const [baseUrl, setBaseUrl] = useState("");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [memberName, setMemberName] = useState("");
  const [proxyConfigPath, setProxyConfigPath] = useState("");
  const [testing, setTesting] = useState(false);
  const [saving, setSaving] = useState(false);
  const [result, setResult] = useState<string | null>(null);
  const [isError, setIsError] = useState(false);

  // Seed the fields from the persisted backend config each time we open.
  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    const fallback = getConfig();
    setBaseUrl(fallback.baseUrl);
    setUsername(fallback.username ?? "");
    setPassword(fallback.password ?? "");
    setMemberName(fallback.memberName ?? "");
    setProxyConfigPath(fallback.proxyConfigPath ?? "");
    setResult(null);
    setIsError(false);
    getServerConfig()
      .then((cfg) => {
        if (cancelled) return;
        setBaseUrl(cfg.baseUrl);
        setUsername(cfg.username ?? "");
        setPassword(cfg.password ?? "");
        setMemberName(cfg.memberName ?? "");
        setProxyConfigPath(cfg.proxyConfigPath ?? "");
      })
      .catch(() => {
        /* keep the local fallback already shown */
      });
    return () => {
      cancelled = true;
    };
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

  async function handleTest() {
    setTesting(true);
    setResult(null);
    setIsError(false);
    try {
      const health = await testConnection(baseUrl, username, password);
      setResult(`Connection OK — ${health.version}`);
      setIsError(false);
    } catch (e) {
      setResult(`Connection failed: ${describeError(e)}`);
      setIsError(true);
    } finally {
      setTesting(false);
    }
  }

  async function handleSave() {
    setSaving(true);
    setResult(null);
    setIsError(false);
    try {
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
      setResult(`Could not save: ${describeError(e)}`);
      setIsError(true);
    } finally {
      setSaving(false);
    }
  }

  const busy = testing || saving;

  return (
    <div
      className="modal-backdrop"
      onClick={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div className="modal" role="dialog" aria-modal="true" aria-label="Server settings">
        <h2 className="approval__title">Server Settings</h2>
        <div className="settings-fields">
          <label className="settings-field">
            <span>Base URL</span>
            <input
              className="settings-input"
              value={baseUrl}
              onChange={(e) => setBaseUrl(e.target.value)}
              placeholder="http://127.0.0.1:4096"
              autoFocus
            />
          </label>
          <label className="settings-field">
            <span>Username (optional)</span>
            <input
              className="settings-input"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              placeholder="opencode"
            />
          </label>
          <label className="settings-field">
            <span>Display name (shown on your sessions)</span>
            <input
              className="settings-input"
              value={memberName}
              onChange={(e) => setMemberName(e.target.value)}
              placeholder="your username"
            />
          </label>
          <label className="settings-field">
            <span>Password (optional)</span>
            <input
              className="settings-input"
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
            />
          </label>
          <label className="settings-field">
            <span>Proxy config path (optional — for hosts)</span>
            <input
              className="settings-input"
              value={proxyConfigPath}
              onChange={(e) => setProxyConfigPath(e.target.value)}
              placeholder="./proxy.json"
            />
          </label>
        </div>
        <div className="settings-manage">
          <button
            type="button"
            className="btn"
            disabled={busy}
            onClick={() => onJoinWorkspace?.()}
            title="Connect to a teammate's workspace with a username and password"
          >
            Join a workspace…
          </button>
          <span className="settings-manage__hint">
            Use the username and password from the host.
          </span>
        </div>
        <div className="settings-test">
          <button type="button" className="btn" disabled={busy} onClick={() => void handleTest()}>
            {testing ? "Testing..." : "Test connection"}
          </button>
          {result && (
            <span className={`settings-test__result${isError ? " settings-test__result--error" : ""}`}>
              {result}
            </span>
          )}
        </div>
        <div className="settings-manage">
          <button
            type="button"
            className="btn"
            disabled={busy}
            onClick={() => onManageMembers?.()}
            title="Edit the host's proxy.json whitelist"
          >
            Manage members…
          </button>
          <span className="settings-manage__hint">
            Edit the host's <code>proxy.json</code> whitelist.
          </span>
        </div>
        <div className="approval__actions">
          <button type="button" className="btn btn--ghost" disabled={busy} onClick={onClose}>
            Cancel
          </button>
          <button type="button" className="btn btn--primary" disabled={busy} onClick={() => void handleSave()}>
            {saving ? "Saving..." : "Save"}
          </button>
        </div>
      </div>
    </div>
  );
}
