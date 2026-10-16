import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";
import { getProviders } from "../commands";
import { getModel, getSnapshot, setModel, subscribe } from "../model";
import { useAppState } from "../store";
import type { Provider, ProviderList, ProviderModel } from "../types";

interface Props {
  className?: string;
  /** Effective orchestrator root; its `model` is the server default. */
  rootSessionId: string | null;
}

/**
 * Maximum model rows rendered at once. The catalogue can hold thousands of
 * models; with no query we show only the usable providers and cap the rows, and
 * the search box is how the rest is reached.
 */
const RENDER_CAP = 50;

/** Case-insensitive substring match across the given strings (empty = all). */
function matches(query: string, ...haystacks: string[]): boolean {
  if (query === "") return true;
  return haystacks.some((value) => value.toLowerCase().includes(query));
}

/** Look a model's human name up in the catalogue; `null` when unknown. */
function resolveModelName(
  list: ProviderList | null,
  providerID: string,
  modelID: string,
): string | null {
  const provider = list?.all.find((p) => p.id === providerID);
  const model = provider?.models[modelID];
  const name = model?.name?.trim();
  return name ? name : null;
}

/** Why a provider's models cannot be chosen right now. */
function unavailableReason(provider: Provider): string {
  return provider.hasAuth
    ? `${provider.name} is not connected — authentication is required before it can serve a request.`
    : `${provider.name} is not available: it provides no authentication method.`;
}

export function ModelPicker({ className = "", rootSessionId }: Props) {
  const [open, setOpen] = useState(false);
  const [providers, setProviders] = useState<ProviderList | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const rootRef = useRef<HTMLDivElement | null>(null);
  const searchRef = useRef<HTMLInputElement | null>(null);

  const current = useSyncExternalStore(subscribe, getSnapshot, getSnapshot);

  // The root session's model is the server default when no local choice is set.
  const state = useAppState();
  const rootSession = rootSessionId
    ? state.sessions.find((s) => s.session.id === rootSessionId)
    : undefined;
  const sessionModel = rootSession?.session.model ?? null;

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const list = await getProviders();
      setProviders(list);
      // If the stored choice is no longer offered, or its provider cannot serve
      // a request, fall back to the server default rather than sending a model
      // the server rejects with `ProviderModelNotFoundError`.
      const choice = getModel();
      if (choice !== null) {
        const provider = list.all.find((p) => p.id === choice.providerID);
        const offered =
          provider !== undefined &&
          Object.prototype.hasOwnProperty.call(provider.models, choice.modelID);
        if (!offered || (provider !== undefined && !provider.usable)) {
          setModel(null);
        }
      }
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  }, []);

  // Load once so the trigger can show the real name before the first open.
  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    if (!open) return;
    setQuery("");
    searchRef.current?.focus();
    function onKeyDown(event: KeyboardEvent): void {
      if (event.key !== "Escape") return;
      setOpen(false);
    }
    function onPointerDown(event: PointerEvent): void {
      const target = event.target as Node | null;
      if (rootRef.current && target && rootRef.current.contains(target)) return;
      setOpen(false);
    }
    document.addEventListener("keydown", onKeyDown);
    document.addEventListener("pointerdown", onPointerDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
      document.removeEventListener("pointerdown", onPointerDown);
    };
  }, [open]);

  function toggle(): void {
    const next = !open;
    setOpen(next);
    if (next) void refresh();
  }

  function choose(choice: { providerID: string; modelID: string } | null): void {
    setModel(choice);
    setOpen(false);
  }

  const q = query.trim().toLowerCase();

  const groups = useMemo(() => {
    const list = providers;
    if (!list) return { visible: [], hidden: 0, total: 0 };

    const filtered: { provider: Provider; models: ProviderModel[] }[] = [];
    for (const provider of list.all) {
      // With an empty query, only providers that can actually serve a request.
      if (q === "" && !provider.usable) continue;
      const providerMatches = matches(q, provider.name, provider.id);
      const models = Object.values(provider.models).filter(
        (m) => matches(q, m.id, m.name) || providerMatches,
      );
      if (models.length === 0) continue;
      filtered.push({ provider, models });
    }

    const visible: { provider: Provider; models: ProviderModel[] }[] = [];
    let remaining = RENDER_CAP;
    let hidden = 0;
    for (const group of filtered) {
      if (remaining <= 0) {
        hidden += group.models.length;
        continue;
      }
      const take = group.models.slice(0, remaining);
      hidden += group.models.length - take.length;
      visible.push({ provider: group.provider, models: take });
      remaining -= take.length;
    }
    const total = filtered.reduce((n, group) => n + group.models.length, 0);
    return { visible, hidden, total };
  }, [providers, q]);

  const effectiveProvider =
    current?.providerID ?? sessionModel?.providerID ?? null;
  const effectiveModelId = current?.modelID ?? sessionModel?.id ?? null;
  const resolvedName =
    effectiveProvider && effectiveModelId
      ? resolveModelName(providers, effectiveProvider, effectiveModelId)
      : null;
  const triggerLabel =
    effectiveProvider && effectiveModelId
      ? resolvedName ?? `${effectiveProvider}/${effectiveModelId}`
      : "Default model";
  const triggerTitle =
    effectiveProvider && effectiveModelId
      ? `${effectiveProvider}:${effectiveModelId}`
      : "Use the server default";

  return (
    <div className={`model-picker ${className}`} ref={rootRef}>
      <button
        type="button"
        className={`topbar__btn${open ? " is-active" : ""}`}
        aria-haspopup="menu"
        aria-expanded={open}
        title={triggerTitle}
        onClick={toggle}
      >
        Model: {triggerLabel}
      </button>
      {open && (
        <div
          className="skills-popover model-picker__popover"
          role="menu"
          aria-label="Models"
        >
          <div className="model-picker__search">
            <svg
              className="model-picker__search-icon"
              viewBox="0 0 16 16"
              aria-hidden
              focusable="false"
            >
              <circle
                cx="6.5"
                cy="6.5"
                r="4.5"
                fill="none"
                stroke="currentColor"
                strokeWidth="1.5"
              />
              <line
                x1="10"
                y1="10"
                x2="14"
                y2="14"
                stroke="currentColor"
                strokeWidth="1.5"
                strokeLinecap="round"
              />
            </svg>
            <input
              ref={searchRef}
              className="model-picker__search-input"
              type="search"
              value={query}
              placeholder="Search models…"
              aria-label="Search providers and models"
              onChange={(event) => setQuery(event.target.value)}
            />
          </div>
          {loading && <p className="skills-popover__empty">Loading models…</p>}
          {error && !loading && <p className="skills-popover__error">{error}</p>}
          {!loading && !error && providers && (
            <div className="model-picker__content">
              <ul className="model-picker__list">
                <li>
                  <button
                    type="button"
                    role="menuitemradio"
                    aria-checked={current === null}
                    className={`model-picker__item${current === null ? " is-selected" : ""}`}
                    onClick={() => choose(null)}
                  >
                    <span className="model-picker__check" aria-hidden>
                      {current === null ? "✓" : ""}
                    </span>
                    <span className="model-picker__label">Use the server default</span>
                  </button>
                </li>
                {groups.visible.map(({ provider, models }) => {
                  const defaultModel = providers.default[provider.id];
                  const unavailable = !provider.usable;
                  return (
                    <li key={provider.id} className="model-picker__provider">
                      <div className="model-picker__provider-name">
                        <span className="model-picker__provider-label">
                          {provider.name}
                        </span>
                        {unavailable && (
                          <span
                            className="model-picker__tag model-picker__tag--unavailable"
                            title={unavailableReason(provider)}
                          >
                            unavailable
                          </span>
                        )}
                      </div>
                      <ul className="model-picker__provider-models">
                        {models.map((m) => {
                          const isSelected =
                            current !== null &&
                            current.providerID === provider.id &&
                            current.modelID === m.id;
                          const isDefault = defaultModel === m.id;
                          return (
                            <li key={m.id}>
                              <button
                                type="button"
                                role="menuitemradio"
                                aria-checked={isSelected}
                                aria-disabled={unavailable}
                                disabled={unavailable}
                                title={
                                  unavailable
                                    ? unavailableReason(provider)
                                    : `${provider.id}:${m.id}`
                                }
                                className={`model-picker__item${isSelected ? " is-selected" : ""}${unavailable ? " is-unusable" : ""}`}
                                onClick={() =>
                                  choose({ providerID: provider.id, modelID: m.id })
                                }
                              >
                                <span className="model-picker__check" aria-hidden>
                                  {isSelected ? "✓" : ""}
                                </span>
                                <span className="model-picker__label">
                                  {m.name || m.id}
                                  {isDefault && (
                                    <span className="model-picker__default"> (default)</span>
                                  )}
                                </span>
                                {m.free && (
                                  <span className="model-picker__tag model-picker__tag--free">
                                    free
                                  </span>
                                )}
                              </button>
                            </li>
                          );
                        })}
                      </ul>
                    </li>
                  );
                })}
              </ul>
              {groups.total === 0 && (
                <p className="skills-popover__empty">
                  {q !== ""
                    ? `No matches for “${query.trim()}”.`
                    : "No usable models available."}
                </p>
              )}
              {groups.hidden > 0 && (
                <p className="model-picker__more">
                  Showing {RENDER_CAP} of {groups.total}. Refine the search to reach
                  the rest.
                </p>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
