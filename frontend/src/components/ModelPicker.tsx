import { useCallback, useEffect, useRef, useState, useSyncExternalStore } from "react";
import { getProviders } from "../commands";
import { getModel, getSnapshot, setModel, subscribe } from "../model";
import type { ProviderList } from "../types";

interface Props {
  className?: string;
}

export function ModelPicker({ className = "" }: Props) {
  const [open, setOpen] = useState(false);
  const [providers, setProviders] = useState<ProviderList | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const rootRef = useRef<HTMLDivElement | null>(null);

  const current = useSyncExternalStore(subscribe, getSnapshot, getSnapshot);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const list = await getProviders();
      setProviders(list);
      // If stored choice no longer offered, treat as default
      const choice = getModel();
      if (choice !== null) {
        const offered = list.all.some(
          (p) =>
            p.id === choice.providerID &&
            Object.prototype.hasOwnProperty.call(p.models, choice.modelID),
        );
        if (!offered) {
          setModel(null);
        }
      }
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (!open) return;
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

  const triggerLabel =
    current === null ? "Default model" : current.modelID;

  return (
    <div className={`model-picker ${className}`} ref={rootRef}>
      <button
        type="button"
        className={`topbar__btn${open ? " is-active" : ""}`}
        aria-haspopup="menu"
        aria-expanded={open}
        title={current === null ? "Use server default" : `${current.providerID}:${current.modelID}`}
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
          {loading && (
            <p className="skills-popover__empty">Loading models…</p>
          )}
          {error && !loading && (
            <p className="skills-popover__error">{error}</p>
          )}
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
                {providers.all.map((provider) => {
                  const models = Object.values(provider.models);
                  if (models.length === 0) return null;
                  const defaultModel = providers.default[provider.id];
                  return (
                    <li key={provider.id} className="model-picker__provider">
                      <div className="model-picker__provider-name">{provider.name}</div>
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
                                className={`model-picker__item${isSelected ? " is-selected" : ""}`}
                                onClick={() => choose({ providerID: provider.id, modelID: m.id })}
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
                              </button>
                            </li>
                          );
                        })}
                      </ul>
                    </li>
                  );
                })}
              </ul>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
