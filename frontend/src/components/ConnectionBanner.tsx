import { useAppState } from "../store";
import { useServerConfig } from "../serverConfig";

interface Props {
  /**
   * True once the first authoritative snapshot has landed. Gates rendering so
   * the banner never flashes during the initial load, when the store still
   * holds the empty (disconnected) default.
   */
  ready: boolean;
}

/**
 * Full-width warning shown while the app cannot reach `opencode serve`.
 *
 * Driven by the existing `connected` snapshot flag — the core event hub flips
 * it on every reconnect attempt — so it appears a few seconds after the server
 * dies and clears itself automatically on reconnect. It is deliberately not
 * dismissible: being offline is a state, not a user choice.
 */
export function ConnectionBanner({ ready }: Props) {
  const state = useAppState();
  const { baseUrl } = useServerConfig();
  if (!ready || state.connected) return null;

  return (
    <div className="connection-banner" role="alert">
      <span className="connection-banner__icon" aria-hidden>
        ⚠
      </span>
      <span className="connection-banner__text">
        Can’t reach <code>opencode serve</code> at <code>{baseUrl}</code> — retrying…
        Start it with <code>opencode serve</code>, or change the address in Settings.
      </span>
    </div>
  );
}
