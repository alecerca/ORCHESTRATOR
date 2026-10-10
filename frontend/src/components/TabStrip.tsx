import { useRef, type KeyboardEvent } from "react";
import { tabSessionId, type Tab, type TabGroup } from "../tabs";
import type { SessionSnapshot } from "../types";
import { shortId } from "../utils";

interface Props {
  group: TabGroup;
  sessions: SessionSnapshot[];
  onActivateTab: (groupId: string, tabId: string) => void;
  onCloseTab: (groupId: string, tabId: string) => void;
}

/** Tab title: the session title, falling back to its short id. */
function tabTitle(tab: Tab, sessions: SessionSnapshot[]): string {
  const sessionId = tabSessionId(tab);
  if (!sessionId) return tab.id;
  const snapshot = sessions.find((entry) => entry.session.id === sessionId);
  return snapshot?.session.title || shortId(sessionId);
}

/**
 * One group's tab strip (VS Code style): a real `role="tablist"` with one
 * `role="tab"` per document. The active tab is illuminated; arrow keys move
 * between tabs with a roving tabindex.
 */
export function TabStrip({ group, sessions, onActivateTab, onCloseTab }: Props) {
  const refs = useRef<(HTMLDivElement | null)[]>([]);

  function handleKey(event: KeyboardEvent<HTMLDivElement>, index: number): void {
    const last = group.tabs.length - 1;
    let next = -1;
    if (event.key === "ArrowRight") next = index === last ? 0 : index + 1;
    else if (event.key === "ArrowLeft") next = index === 0 ? last : index - 1;
    else if (event.key === "Home") next = 0;
    else if (event.key === "End") next = last;
    if (next < 0) return;

    event.preventDefault();
    onActivateTab(group.id, group.tabs[next].id);
    refs.current[next]?.focus();
  }

  return (
    <div className="tabstrip" role="tablist" aria-label="Open tabs" title="Open tabs">
      {group.tabs.map((tab, index) => {
        const active = tab.id === group.activeTabId;
        const title = tabTitle(tab, sessions);
        return (
          <div
            key={tab.id}
            ref={(element) => {
              refs.current[index] = element;
            }}
            role="tab"
            aria-selected={active}
            tabIndex={active ? 0 : -1}
            className={`tab${active ? " is-active" : ""}`}
            title={title}
            onClick={() => onActivateTab(group.id, tab.id)}
            onKeyDown={(event) => handleKey(event, index)}
          >
            <span className="tab__label">{title}</span>
            <button
              type="button"
              className="tab__close"
              aria-label={`Close ${title}`}
              title={`Close ${title}`}
              onClick={(event) => {
                event.stopPropagation();
                onCloseTab(group.id, tab.id);
              }}
            >
              ✕
            </button>
          </div>
        );
      })}
    </div>
  );
}
