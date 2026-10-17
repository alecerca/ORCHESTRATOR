import { useCallback, useEffect, useState, type FormEvent } from "react";
import {
  deleteProviderApiKey,
  getMcpStatus,
  getOpencodeConfig,
  getProviderAuth,
  getProviders,
  listCommands,
  oauthAuthorize,
  oauthCallback,
  patchOpencodeConfig,
  setProviderApiKey,
} from "../commands";
import { useAppState } from "../store";
import type {
  ConfigPatch,
  McpStatus,
  OpencodeConfig,
  Provider,
  ProviderAuthAuthorization,
  ProviderAuthMethod,
  ProviderList,
  SlashCommand,
} from "../types";
import { describeError } from "../utils";
import { ApiKeyDialog } from "./ApiKeyDialog";

type Tab = "providers" | "agents" | "mcp" | "commands" | "config";

interface Props {
  open: boolean;
  onClose: () => void;
}

const TABS: { id: Tab; label: string }[] = [
  { id: "providers", label: "Providers" },
  { id: "agents", label: "Agents" },
  { id: "mcp", label: "MCP" },
  { id: "commands", label: "Commands" },
  { id: "config", label: "Config" },
];

/** Editable view of one config scalar; blank values mirror an absent field. */
interface ConfigDraft {
  shell: string;
  logLevel: string;
  share: string;
  model: string;
  small_model: string;
  default_agent: string;
  snapshot: boolean;
  autoshare: boolean;
}

function draftFrom(config: OpencodeConfig): ConfigDraft {
  return {
    shell: config.shell ?? "",
    logLevel: config.logLevel ?? "",
    share: config.share ?? "",
    model: config.model ?? "",
    small_model: config.small_model ?? "",
    default_agent: config.default_agent ?? "",
    snapshot: config.snapshot ?? false,
    autoshare: config.autoshare ?? false,
  };
}

/** Only the scalars the user actually touched ride in the patch. */
function toPatch(draft: ConfigDraft, base: OpencodeConfig): ConfigPatch {
  const patch: ConfigPatch = {};
  if (draft.shell !== (base.shell ?? "")) patch.shell = draft.shell;
  if (draft.logLevel !== (base.logLevel ?? "")) patch.logLevel = draft.logLevel;
  if (draft.share !== (base.share ?? "")) patch.share = draft.share;
  if (draft.model !== (base.model ?? "")) patch.model = draft.model;
  if (draft.small_model !== (base.small_model ?? "")) patch.small_model = draft.small_model;
  if (draft.default_agent !== (base.default_agent ?? ""))
    patch.default_agent = draft.default_agent;
  if (draft.snapshot !== (base.snapshot ?? false)) patch.snapshot = draft.snapshot;
  if (draft.autoshare !== (base.autoshare ?? false)) patch.autoshare = draft.autoshare;
  return patch;
}

function mcpChip(status: McpStatus["status"]): string {
  switch (status) {
    case "connected":
      return "chip chip--done";
    case "failed":
      return "chip chip--error";
    case "needs_auth":
    case "needs_client_registration":
      return "chip chip--waiting_approval";
    default:
      return "chip chip--pending";
  }
}

/**
 * The rest of the OpenCode Ctrl+P surface (T2/T4–T9): providers with auth
 * state and models, agents, MCP servers, slash commands, and the scalar
 * config allowlist.
 *
 * SECURITY: credential values never enter this component — keys are typed
 * straight into `ApiKeyDialog` (write-only), OAuth codes are pasted inputs,
 * and every DTO here omits the credential-bearing wire sections.
 */
export function OpenCodeDialog({ open, onClose }: Props) {
  const agents = useAppState().agents;
  const [tab, setTab] = useState<Tab>("providers");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);

  const [providers, setProviders] = useState<ProviderList | null>(null);
  const [auth, setAuth] = useState<Record<string, ProviderAuthMethod[]> | null>(null);
  const [mcp, setMcp] = useState<Record<string, McpStatus> | null>(null);
  const [commands, setCommands] = useState<SlashCommand[] | null>(null);
  const [config, setConfig] = useState<OpencodeConfig | null>(null);
  const [draft, setDraft] = useState<ConfigDraft | null>(null);

  const [query, setQuery] = useState("");
  const [expanded, setExpanded] = useState<string | null>(null);
  const [keyProvider, setKeyProvider] = useState<Provider | null>(null);
  const [oauth, setOauth] = useState<{
    providerId: string;
    providerName: string;
    method: number;
    auth: ProviderAuthAuthorization;
  } | null>(null);
  const [oauthCode, setOauthCode] = useState("");
  /** Id of the row action currently in flight (`null` = none). */
  const [action, setAction] = useState<string | null>(null);

  /** Re-read providers + auth methods; returns the fresh list for checks. */
  const refreshProviders = useCallback(async (): Promise<ProviderList | null> => {
    try {
      const [list, methods] = await Promise.all([getProviders(), getProviderAuth()]);
      setProviders(list);
      setAuth(methods);
      setError(null);
      return list;
    } catch (cause) {
      setError(describeError(cause));
      return null;
    }
  }, []);

  const load = useCallback(async () => {
    setBusy(true);
    setError(null);
    setNote(null);
    try {
      const [list, methods, mcpStatuses, cmds, cfg] = await Promise.all([
        getProviders(),
        getProviderAuth(),
        getMcpStatus(),
        listCommands(),
        getOpencodeConfig(),
      ]);
      setProviders(list);
      setAuth(methods);
      setMcp(mcpStatuses);
      setCommands(cmds);
      setConfig(cfg);
      setDraft(draftFrom(cfg));
    } catch (cause) {
      setError(describeError(cause));
    } finally {
      setBusy(false);
    }
  }, []);

  useEffect(() => {
    if (open) void load();
  }, [open, load]);

  // Escape: the ApiKeyDialog handles its own first (it also listens on the
  // window, so the outer dialog must stand down while it is open), then the
  // inline OAuth panel, then the dialog itself.
  useEffect(() => {
    if (!open) return;
    function onKey(event: KeyboardEvent): void {
      if (event.key !== "Escape") return;
      if (keyProvider) return;
      if (oauth) {
        setOauth(null);
        setOauthCode("");
        return;
      }
      onClose();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, keyProvider, oauth, onClose]);

  if (!open) return null;

  const connectedSet = new Set(providers?.connected ?? []);
  const isConnected = (id: string): boolean => connectedSet.has(id);

  const q = query.trim().toLowerCase();
  const visibleProviders = (providers?.all ?? []).filter(
    (p) => q === "" || p.id.toLowerCase().includes(q) || p.name.toLowerCase().includes(q),
  );
  const visibleCommands = (commands ?? []).filter(
    (c) =>
      q === "" ||
      c.name.toLowerCase().includes(q) ||
      (c.description ?? "").toLowerCase().includes(q),
  );

  /** OAuth methods of a provider, keeping the original array index. */
  function oauthMethods(providerId: string): { method: number; label: string }[] {
    return (auth?.[providerId] ?? [])
      .map((m, method) => ({ method, label: m.label, type: m.type }))
      .filter((entry) => entry.type === "oauth")
      .map(({ method, label }) => ({ method, label }));
  }

  async function removeKey(p: Provider): Promise<void> {
    if (!window.confirm(`Remove the stored API key for ${p.name}?`)) return;
    setAction(p.id);
    setError(null);
    try {
      await deleteProviderApiKey(p.id);
      await refreshProviders();
      setNote(`${p.name}: credential removed.`);
    } catch (cause) {
      setError(describeError(cause));
    } finally {
      setAction(null);
    }
  }

  async function submitApiKey(key: string): Promise<void> {
    if (!keyProvider) return;
    await setProviderApiKey(keyProvider.id, key);
    await refreshProviders();
    setNote(`${keyProvider.name}: key stored.`);
  }

  async function startOauth(p: Provider, method: number): Promise<void> {
    setAction(`oauth-${p.id}-${method}`);
    setError(null);
    try {
      const authz = await oauthAuthorize(p.id, method);
      setOauth({ providerId: p.id, providerName: p.name, method, auth: authz });
      setOauthCode("");
    } catch (cause) {
      setError(describeError(cause));
    } finally {
      setAction(null);
    }
  }

  async function finishOauth(): Promise<void> {
    if (!oauth) return;
    setAction("oauth-finish");
    setError(null);
    try {
      await oauthCallback(oauth.providerId, oauth.method, oauthCode.trim());
      await refreshProviders();
      setNote(`${oauth.providerName}: connected.`);
      setOauth(null);
      setOauthCode("");
    } catch (cause) {
      setError(describeError(cause));
    } finally {
      setAction(null);
    }
  }

  /** `method: "auto"` flow: the user authorized in the browser; re-poll. */
  async function recheckOauth(): Promise<void> {
    if (!oauth) return;
    setAction("oauth-finish");
    const list = await refreshProviders();
    if (list && list.connected.includes(oauth.providerId)) {
      setNote(`${oauth.providerName}: connected.`);
      setOauth(null);
      setOauthCode("");
    } else {
      setError("Not connected yet — finish the authorization in the browser, then check again.");
    }
    setAction(null);
  }

  async function saveConfig(event: FormEvent): Promise<void> {
    event.preventDefault();
    if (!config || !draft) return;
    const patch = toPatch(draft, config);
    if (Object.keys(patch).length === 0) {
      setNote("Nothing to save — no changes.");
      return;
    }
    setBusy(true);
    setError(null);
    setNote(null);
    try {
      const next = await patchOpencodeConfig(patch);
      setConfig(next);
      setDraft(draftFrom(next));
      setNote("Saved.");
    } catch (cause) {
      setError(describeError(cause));
    } finally {
      setBusy(false);
    }
  }

  function textField(
    key: Exclude<keyof ConfigDraft, "snapshot" | "autoshare" | "logLevel">,
    label: string,
    placeholder?: string,
  ): JSX.Element | null {
    const value = draft;
    if (!value) return null;
    return (
      <label className="settings-field">
        <span>{label}</span>
        <input
          className="settings-input"
          value={value[key]}
          placeholder={placeholder}
          onChange={(event) => setDraft({ ...value, [key]: event.target.value })}
        />
      </label>
    );
  }

  return (
    <div
      className="modal-backdrop"
      onClick={(event) => {
        if (event.target === event.currentTarget && !keyProvider) onClose();
      }}
    >
      <div className="modal oc-dialog" role="dialog" aria-modal="true" aria-label="OpenCode surface">
        <div className="oc-dialog__head">
          <h2 className="approval__title">OpenCode</h2>
          <button
            type="button"
            className="btn btn--ghost btn--xs"
            aria-label="Close"
            disabled={!!keyProvider}
            onClick={onClose}
          >
            ✕
          </button>
        </div>

        <div className="oc-tabs" role="tablist" aria-label="OpenCode surfaces">
          {TABS.map((t) => (
            <button
              key={t.id}
              type="button"
              role="tab"
              aria-selected={tab === t.id}
              className={`oc-tab${tab === t.id ? " is-active" : ""}`}
              onClick={() => setTab(t.id)}
            >
              {t.label}
            </button>
          ))}
        </div>

        {error && (
          <p className="approval__error" role="alert">
            {error}
          </p>
        )}
        {note && <p className="oc-note">{note}</p>}

        <div className="oc-body">
          {tab === "providers" && (
            <>
              {oauth && (
                <div className="oc-oauth" role="group" aria-label={`Connect ${oauth.providerName}`}>
                  <p className="oc-oauth__title">
                    {oauth.auth.method === "code"
                      ? "Paste the authorization code"
                      : `Connect ${oauth.providerName}`}
                  </p>
                  <p className="oc-oauth__steps">{oauth.auth.instructions}</p>
                  <a
                    className="oc-oauth__link"
                    href={oauth.auth.url}
                    target="_blank"
                    rel="noreferrer"
                  >
                    {oauth.auth.url}
                  </a>
                  {oauth.auth.method === "code" && (
                    <label className="settings-field">
                      <span>Authorization code</span>
                      <input
                        className="settings-input"
                        value={oauthCode}
                        placeholder="code…"
                        autoComplete="off"
                        autoFocus
                        onChange={(event) => setOauthCode(event.target.value)}
                      />
                    </label>
                  )}
                  <div className="approval__actions">
                    <button
                      type="button"
                      className="btn btn--ghost"
                      disabled={action === "oauth-finish"}
                      onClick={() => {
                        setOauth(null);
                        setOauthCode("");
                      }}
                    >
                      Cancel
                    </button>
                    {oauth.auth.method === "code" ? (
                      <button
                        type="button"
                        className="btn btn--primary"
                        disabled={!oauthCode.trim() || action === "oauth-finish"}
                        onClick={() => void finishOauth()}
                      >
                        {action === "oauth-finish" ? "Finishing…" : "Finish"}
                      </button>
                    ) : (
                      <button
                        type="button"
                        className="btn btn--primary"
                        disabled={action === "oauth-finish"}
                        onClick={() => void recheckOauth()}
                      >
                        {action === "oauth-finish" ? "Checking…" : "I've authorized — check"}
                      </button>
                    )}
                  </div>
                </div>
              )}

              <div className="oc-toolbar">
                <input
                  className="settings-input"
                  type="search"
                  placeholder="Filter providers…"
                  aria-label="Filter providers"
                  value={query}
                  onChange={(event) => setQuery(event.target.value)}
                />
              </div>

              {!providers ? (
                <p className="oc-empty">{busy ? "Loading…" : "Providers unavailable."}</p>
              ) : (
                <ul className="oc-list">
                  {visibleProviders.map((p) => {
                    const connected = isConnected(p.id);
                    const models = Object.values(p.models);
                    const open = expanded === p.id;
                    return (
                      <li key={p.id} className="oc-row">
                        <div className="oc-row__main">
                          <button
                            type="button"
                            className="oc-row__head"
                            aria-expanded={open}
                            onClick={() => setExpanded(open ? null : p.id)}
                          >
                            <span
                              className={
                                connected
                                  ? "chip chip--done"
                                  : p.hasAuth
                                    ? "chip chip--pending"
                                    : "chip chip--error"
                              }
                            >
                              {connected ? "authenticated" : p.hasAuth ? "needs auth" : "no auth"}
                            </span>
                            <span className="oc-row__name">{p.name}</span>
                            <code className="oc-row__id">{p.id}</code>
                            <span className="oc-row__count">{models.length} models</span>
                            <span className="oc-row__chevron" aria-hidden>
                              {open ? "▾" : "▸"}
                            </span>
                          </button>
                          <div className="oc-row__actions">
                            {p.offersApiKey &&
                              (connected ? (
                                <button
                                  type="button"
                                  className="btn btn--ghost btn--xs"
                                  disabled={action !== null}
                                  onClick={() => void removeKey(p)}
                                >
                                  Remove key
                                </button>
                              ) : (
                                <button
                                  type="button"
                                  className="btn btn--ghost btn--xs"
                                  disabled={action !== null}
                                  onClick={() => setKeyProvider(p)}
                                >
                                  Add key
                                </button>
                              ))}
                            {!connected &&
                              oauthMethods(p.id).map(({ method, label }) => (
                                <button
                                  key={method}
                                  type="button"
                                  className="btn btn--ghost btn--xs"
                                  disabled={action !== null}
                                  title={`OAuth: ${label}`}
                                  onClick={() => void startOauth(p, method)}
                                >
                                  {label}
                                </button>
                              ))}
                          </div>
                        </div>
                        {open && (
                          <ul className="oc-models">
                            {models.length === 0 && <li className="oc-empty">No models.</li>}
                            {models.map((model) => (
                              <li key={model.id}>
                                <span className="oc-models__name">{model.name}</span>
                                {model.free && <span className="oc-free">free</span>}
                                {model.status === "deprecated" && (
                                  <span className="chip chip--pending">deprecated</span>
                                )}
                              </li>
                            ))}
                          </ul>
                        )}
                      </li>
                    );
                  })}
                  {visibleProviders.length === 0 && (
                    <li className="oc-empty">No providers match “{query.trim()}”.</li>
                  )}
                </ul>
              )}
            </>
          )}

          {tab === "agents" && (
            <ul className="oc-list">
              {agents.length === 0 && (
                <li className="oc-empty">No agents reported by the server.</li>
              )}
              {agents.map((agent) => (
                <li key={agent.name} className="oc-row oc-row--static">
                  <span className="chip chip--pending">{agent.type}</span>
                  <span className="oc-row__name">{agent.name}</span>
                  {agent.model && (
                    <code className="oc-row__id">
                      {agent.model.providerID}/{agent.model.modelID}
                    </code>
                  )}
                  {agent.description && (
                    <span className="oc-row__hint">{agent.description}</span>
                  )}
                </li>
              ))}
            </ul>
          )}

          {tab === "mcp" && (
            <ul className="oc-list">
              {!mcp && <li className="oc-empty">{busy ? "Loading…" : "Unavailable."}</li>}
              {mcp && Object.keys(mcp).length === 0 && (
                <li className="oc-empty">No MCP servers configured.</li>
              )}
              {mcp &&
                Object.entries(mcp).map(([name, status]) => (
                  <li key={name} className="oc-row oc-row--static">
                    <span className={mcpChip(status.status)}>
                      {status.status.replace(/_/g, " ")}
                    </span>
                    <span className="oc-row__name">{name}</span>
                    {"error" in status && <span className="oc-row__hint">{status.error}</span>}
                  </li>
                ))}
            </ul>
          )}

          {tab === "commands" && (
            <>
              <div className="oc-toolbar">
                <input
                  className="settings-input"
                  type="search"
                  placeholder="Filter commands…"
                  aria-label="Filter commands"
                  value={query}
                  onChange={(event) => setQuery(event.target.value)}
                />
              </div>
              {!commands ? (
                <p className="oc-empty">{busy ? "Loading…" : "Unavailable."}</p>
              ) : (
                <ul className="oc-list">
                  {visibleCommands.map((cmd, index) => (
                    <li
                      key={`${cmd.source ?? ""}:${cmd.name}:${index}`}
                      className="oc-row oc-row--static"
                    >
                      <span className="oc-row__name">/{cmd.name}</span>
                      {cmd.agent && <span className="chip chip--pending">{cmd.agent}</span>}
                      {cmd.source && <span className="oc-row__hint">{cmd.source}</span>}
                      {cmd.description && <span className="oc-row__hint">{cmd.description}</span>}
                      <code className="oc-row__tpl">{cmd.template}</code>
                    </li>
                  ))}
                  {visibleCommands.length === 0 && (
                    <li className="oc-empty">No commands match “{query.trim()}”.</li>
                  )}
                </ul>
              )}
            </>
          )}

          {tab === "config" &&
            (config && draft ? (
              <form className="oc-config" onSubmit={(event) => void saveConfig(event)}>
                {textField("model", "Default model", "server default")}
                {textField("small_model", "Small model", "—")}
                {textField("default_agent", "Default agent", "—")}
                <label className="settings-field">
                  <span>Log level</span>
                  <select
                    className="settings-input"
                    value={draft.logLevel}
                    onChange={(event) => setDraft({ ...draft, logLevel: event.target.value })}
                  >
                    <option value="">server default</option>
                    <option value="DEBUG">DEBUG</option>
                    <option value="INFO">INFO</option>
                    <option value="WARN">WARN</option>
                    <option value="ERROR">ERROR</option>
                  </select>
                </label>
                {textField("shell", "Shell", "—")}
                {textField("share", "Share", "—")}
                <label className="settings-field">
                  <span>Snapshot</span>
                  <input
                    type="checkbox"
                    checked={draft.snapshot}
                    onChange={(event) => setDraft({ ...draft, snapshot: event.target.checked })}
                  />
                </label>
                <label className="settings-field">
                  <span>Autoshare</span>
                  <input
                    type="checkbox"
                    checked={draft.autoshare}
                    onChange={(event) => setDraft({ ...draft, autoshare: event.target.checked })}
                  />
                </label>
                <div className="oc-config__ro">
                  <span>Instructions (read-only)</span>
                  <pre>{(config.instructions ?? []).join("\n") || "—"}</pre>
                </div>
                <div className="approval__actions">
                  <button type="submit" className="btn btn--primary" disabled={busy}>
                    {busy ? "Saving…" : "Save"}
                  </button>
                </div>
              </form>
            ) : (
              <p className="oc-empty">{busy ? "Loading…" : "Config unavailable."}</p>
            ))}
        </div>

        {keyProvider && (
          <ApiKeyDialog
            open
            providerId={keyProvider.id}
            providerName={keyProvider.name}
            onSubmit={submitApiKey}
            onClose={() => setKeyProvider(null)}
          />
        )}
      </div>
    </div>
  );
}
