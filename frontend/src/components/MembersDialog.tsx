import { useEffect, useState } from "react";
import { addProxyMember, getProxyMembers, removeProxyMember } from "../commands";
import { getConfig, useServerConfig } from "../serverConfig";
import type { ProxyMember } from "../types";
import { describeError } from "../utils";

interface Props {
  open: boolean;
  onClose: () => void;
}

/** Roles the proxy enforces; the shell rejects anything else. */
type Role = "editor" | "viewer";

/** Details shown once after a successful add — never recoverable later. */
interface Invite {
  name: string;
  password: string;
  address: string;
}

/**
 * Host console for the whitelist `proxy.json`: lists/adds/removes members and
 * shows an invite (address + username + password once). The path is the one
 * persisted in Settings; the app never runs the proxy — it only edits its file.
 */
export function MembersDialog({ open, onClose }: Props) {
  const configuredPath = useServerConfig().proxyConfigPath?.trim() ?? "";

  const [members, setMembers] = useState<ProxyMember[]>([]);
  const [loading, setLoading] = useState(false);
  const [listError, setListError] = useState<string | null>(null);

  const [name, setName] = useState("");
  const [role, setRole] = useState<Role>("editor");
  const [password, setPassword] = useState("");
  const [adding, setAdding] = useState(false);
  const [addError, setAddError] = useState<string | null>(null);
  const [removing, setRemoving] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [invite, setInvite] = useState<Invite | null>(null);

  // Reset transient form/invite state each time the dialog opens.
  useEffect(() => {
    if (!open) return;
    setName("");
    setRole("editor");
    setPassword("");
    setAddError(null);
    setRemoving(null);
    setActionError(null);
    setInvite(null);
  }, [open]);

  // Load the member list whenever the dialog opens or the path changes.
  useEffect(() => {
    if (!open) return;
    if (!configuredPath) {
      setMembers([]);
      setLoading(false);
      setListError(null);
      return;
    }
    let cancelled = false;
    setLoading(true);
    setListError(null);
    getProxyMembers(configuredPath)
      .then((list) => {
        if (!cancelled) setMembers(list);
      })
      .catch((cause) => {
        if (!cancelled) setListError(describeError(cause));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [open, configuredPath]);

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

  async function handleAdd() {
    if (!configuredPath) return;
    const trimmed = name.trim();
    if (!trimmed) {
      setAddError("Enter a username.");
      return;
    }
    if (!password) {
      setAddError("Enter a password.");
      return;
    }
    setAdding(true);
    setAddError(null);
    // Capture the invite before clearing the form: the password is shown once.
    const pending: Invite = {
      name: trimmed,
      password,
      address: getConfig().baseUrl,
    };
    try {
      await addProxyMember(configuredPath, trimmed, role, password);
    } catch (cause) {
      setAddError(describeError(cause));
      setAdding(false);
      return;
    }
    // Success: reset the form, reveal the one-time invite, then reload the list.
    setName("");
    setPassword("");
    setInvite(pending);
    setActionError(null);
    setAdding(false);
    try {
      setMembers(await getProxyMembers(configuredPath));
      setListError(null);
    } catch (cause) {
      setListError(describeError(cause));
    }
  }

  async function handleRemove(member: ProxyMember) {
    if (!configuredPath) return;
    if (!window.confirm(`Remove "${member.name}" from the whitelist?`)) return;
    setRemoving(member.name);
    setActionError(null);
    try {
      await removeProxyMember(configuredPath, member.name);
      setMembers(await getProxyMembers(configuredPath));
      setListError(null);
    } catch (cause) {
      setActionError(describeError(cause));
    } finally {
      setRemoving(null);
    }
  }

  async function copy(value: string) {
    try {
      await navigator.clipboard.writeText(value);
    } catch {
      // Clipboard may be unavailable; the readonly input still allows manual copy.
    }
  }

  const busy = adding || removing !== null;

  return (
    <div
      className="modal-backdrop"
      onClick={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div className="modal" role="dialog" aria-modal="true" aria-label="Whitelist members">
        <h2 className="approval__title">Members</h2>
        <p className="members-path" title={configuredPath || undefined}>
          {configuredPath || "No proxy config path configured."}
        </p>

        {!configuredPath ? (
          <p className="members-hint">
            Open <strong>Server Settings</strong> and set{" "}
            <em>Proxy config path</em> to the host's <code>proxy.json</code>, then
            reopen this dialog to manage members.
          </p>
        ) : (
          <>
            {actionError && <p className="approval__error">{actionError}</p>}
            {loading ? (
              <p className="members-status">Loading members…</p>
            ) : listError ? (
              <p className="approval__error">{listError}</p>
            ) : members.length === 0 ? (
              <p className="members-status">No members yet. Add the first one below.</p>
            ) : (
              <ul className="members-list">
                {members.map((member) => (
                  <li key={member.name} className="members-row">
                    <span className="members-row__name">{member.name}</span>
                    <span className={`members-role members-role--${member.role}`}>
                      {member.role}
                    </span>
                    <button
                      type="button"
                      className="btn btn--danger btn--xs"
                      disabled={busy}
                      onClick={() => void handleRemove(member)}
                    >
                      {removing === member.name ? "Removing…" : "Remove"}
                    </button>
                  </li>
                ))}
              </ul>
            )}

            <div className="members-add">
              <label className="settings-field">
                <span>Username</span>
                <input
                  className="settings-input"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder="alice"
                  autoComplete="off"
                />
              </label>
              <label className="settings-field">
                <span>Role</span>
                <select
                  className="settings-input"
                  value={role}
                  onChange={(e) => setRole(e.target.value as Role)}
                >
                  <option value="editor">editor</option>
                  <option value="viewer">viewer</option>
                </select>
              </label>
              <label className="settings-field">
                <span>Password</span>
                <input
                  className="settings-input"
                  type="password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  autoComplete="new-password"
                />
              </label>
              <div className="members-add__action">
                <button
                  type="button"
                  className="btn btn--primary"
                  disabled={busy}
                  onClick={() => void handleAdd()}
                >
                  {adding ? "Adding…" : "Add member"}
                </button>
              </div>
              {addError && <p className="approval__error">{addError}</p>}
            </div>

            {invite && (
              <div className="members-invite" role="group" aria-label="Invite">
                <h3 className="members-invite__title">Invite</h3>
                <p className="members-invite__note">
                  Share these with your teammate. The password is shown{" "}
                  <strong>only once</strong> — it cannot be retrieved again.
                </p>
                <div className="settings-field">
                  <span>Server address</span>
                  <div className="members-copy">
                    <input className="settings-input" readOnly value={invite.address} />
                    <button
                      type="button"
                      className="btn btn--xs"
                      onClick={() => void copy(invite.address)}
                    >
                      Copy
                    </button>
                  </div>
                </div>
                <div className="settings-field">
                  <span>Username</span>
                  <div className="members-copy">
                    <input className="settings-input" readOnly value={invite.name} />
                    <button
                      type="button"
                      className="btn btn--xs"
                      onClick={() => void copy(invite.name)}
                    >
                      Copy
                    </button>
                  </div>
                </div>
                <div className="settings-field">
                  <span>Password (shown once)</span>
                  <div className="members-copy">
                    <input className="settings-input" readOnly value={invite.password} />
                    <button
                      type="button"
                      className="btn btn--xs"
                      onClick={() => void copy(invite.password)}
                    >
                      Copy
                    </button>
                  </div>
                </div>
                <div className="approval__actions">
                  <button type="button" className="btn" onClick={() => setInvite(null)}>
                    Done
                  </button>
                </div>
              </div>
            )}
          </>
        )}

        <div className="approval__actions">
          <button type="button" className="btn btn--ghost" onClick={onClose}>
            Close
          </button>
        </div>
      </div>
    </div>
  );
}
