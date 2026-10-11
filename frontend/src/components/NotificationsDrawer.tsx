import type { Effect } from "../types";

interface Props {
  effects: Effect[];
  onDismiss: (index: number) => void;
  onDismissAll: () => void;
}

function describe(effect: Effect): string {
  switch (effect.effect) {
    case "finished":
      return `✓ ${effect.title} finished`;
    case "failed":
      return `✗ ${effect.title}: ${effect.message}`;
    case "approval":
      return `⏳ approval needed: ${effect.title}`;
    case "usage_limit":
      return `⚠ usage limit reached — ${effect.message}`;
  }
}

/** In-app notification toasts fed by `core://message` effects. */
export function NotificationsDrawer({ effects, onDismiss, onDismissAll }: Props) {
  if (effects.length === 0) return null;

  return (
    <div className="notifications">
      <div className="notifications__head">
        <span>Notifications</span>
        <button type="button" className="btn btn--ghost btn--xs" onClick={onDismissAll}>
          Clear
        </button>
      </div>
      {effects.map((effect, index) => (
        <div
          key={`${effect.effect}-${effect.session_id}-${index}`}
          className={`notification notification--${effect.effect}`}
        >
          <span className="notification__text">{describe(effect)}</span>
          <button
            type="button"
            className="notification__close"
            onClick={() => onDismiss(index)}
            title="Dismiss"
          >
            ✕
          </button>
        </div>
      ))}
    </div>
  );
}