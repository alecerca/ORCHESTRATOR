import { useEffect, useState } from "react";
import { getConfig, setConfig } from "../serverConfig";
import { getServerConfig, setServerConfig, testConnection } from "../commands";
import { describeError } from "../utils";

interface Props {
  open: boolean;
  onClose: () => void;
}

export function SettingsDialog({ open, onClose }: Props) {
  const [baseUrl, setBaseUrl] = useState("");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [testing, setTesting] = useState(false);
  const [saving, setSaving] = useState(false);
  const [result, setResult] = useState<string | null>(null);

  // Seed the fields from the persisted backend config each time we open.
  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    const fallback = getConfig();
    setBaseUrl(fallback.baseUrl);
    setUsername(fallback.username ?? "");
    setPassword(fallback.password ?? "");
    setResult(null);
    getServerConfig()
      .then((cfg) => {
        if (cancelled) return;
        setBaseUrl(cfg.baseUrl);
        setUsername(cfg.username ?? "");
        setPassword(cfg.password ?? "");
      })
      .catch(() => {
        /* keep the local fallback already shown */
      });
    return () => {
      cancelled = true;
    };
  }, [open]);

  if (!open) return null;

  async function handleTest() {
    setTesting(true);
    setResult(null);
    try {
      const health = await testConnection(baseUrl, username, password);
      setResult(`Connection OK — ${health.version}`);
    } catch (e) {
      setResult(`Connection failed: ${describeError(e)}`);
    } finally {
      setTesting(false);
    }
  }

  async function handleSave() {
    setSaving(true);
    setResult(null);
    try {
      const cfg = await setServerConfig(baseUrl, username, password);
      // Mirror the persisted config locally so the UI is consistent offline.
      setConfig({
        baseUrl: cfg.baseUrl,
        username: cfg.username,
        password: cfg.password ?? undefined,
      });
      onClose();
    } catch (e) {
      setResult(`Could not save: ${describeError(e)}`);
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="modal" role="dialog" aria-modal="true">
      <div className="dialog" style={{ minWidth: 420 }}>
        <div className="dialog__header">
          <h2>Server Settings</h2>
        </div>
        <div className="dialog__body" style={{ display: "flex", flexDirection: "column", gap: 12 }}>
          <label style={{ display: "flex", flexDirection: "column", gap: 4 }}>
            <span>Base URL</span>
            <input
              className="input"
              value={baseUrl}
              onChange={(e) => setBaseUrl(e.target.value)}
              placeholder="http://127.0.0.1:4096"
            />
          </label>
          <label style={{ display: "flex", flexDirection: "column", gap: 4 }}>
            <span>Username (optional)</span>
            <input
              className="input"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              placeholder="opencode"
            />
          </label>
          <label style={{ display: "flex", flexDirection: "column", gap: 4 }}>
            <span>Password (optional)</span>
            <input
              className="input"
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
            />
          </label>
          <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
            <button className="btn" disabled={testing || saving} onClick={() => void handleTest()}>
              {testing ? "Testing..." : "Test connection"}
            </button>
            {result && <span>{result}</span>}
          </div>
        </div>
        <div className="dialog__actions">
          <button className="btn btn--ghost" onClick={onClose}>
            Cancel
          </button>
          <button className="btn btn--primary" disabled={saving} onClick={() => void handleSave()}>
            {saving ? "Saving..." : "Save"}
          </button>
        </div>
      </div>
    </div>
  );
}
