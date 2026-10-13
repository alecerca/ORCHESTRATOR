import {
  useCallback,
  useEffect,
  useRef,
  type DragEvent,
  type KeyboardEvent,
  type MouseEvent,
  useState,
} from "react";
import { tabSessionId, type GroupNode, type Tab } from "../tabs";
import type { SessionSnapshot } from "../types";
import { baseName, shortId } from "../utils";
import { DiffIcon, FileIcon, SkillIcon, TerminalIcon } from "./icons";

/** Custom MIME type carrying the dragged tab payload (see PaneGrid's drop targets). */
export const TAB_DRAG_MIME = "application/x-orquestador-tab";

interface Props {
  group: GroupNode;
  sessions: SessionSnapshot[];
  onActivateTab: (groupId: string, tabId: string) => void;
  onCloseTab: (groupId: string, tabId: string) => void;
  onSplit?: (groupId: string, direction: "right" | "down", tabId?: string) => void;
}

/**
 * Tab title: a file tab shows its basename, a skill tab its skill name, a diff
 * tab its path's basename (like a file tab), and a session tab its title,
 * falling back to its short id.
 */
function tabTitle(tab: Tab, sessions: SessionSnapshot[]): string {
  if (tab.kind === "file") return baseName(tab.path);
  if (tab.kind === "skill") return tab.name;
  if (tab.kind === "diff") return baseName(tab.path);
  const sessionId = tabSessionId(tab);
  if (!sessionId) return tab.id;
  const snapshot = sessions.find((entry) => entry.session.id === sessionId);
  return snapshot?.session.title || shortId(sessionId);
}

/**
 * One group's tab strip (VS Code style): a real `role="tablist"` with one
 * `role="tab"` per document. The active tab is illuminated; arrow keys move
 * between tabs with a roving tabindex. Tabs are draggable between groups.
 */
export function TabStrip({ group, sessions, onActivateTab, onCloseTab, onSplit }: Props) {
  const refs = useRef<(HTMLDivElement | null)[]>([]);
  const menuRef = useRef<HTMLDivElement | null>(null);
  const firstItemRef = useRef<HTMLButtonElement | null>(null);
  const [contextMenu, setContextMenu] = useState<{
    x: number;
    y: number;
    tabId: string;
  } | null>(null);
  const scrollerRef = useRef<HTMLDivElement | null>(null);
  const [canScrollLeft, setCanScrollLeft] = useState(false);
  const [canScrollRight, setCanScrollRight] = useState(false);

  // Show the scroll arrows only while the strip overflows in that direction.
  // The native horizontal scrollbar is hidden because the webview's overlay
  // scrollbar expands on hover and swallows the strip.
  const updateScrollState = useCallback(() => {
    const el = scrollerRef.current;
    if (!el) return;
    setCanScrollLeft(el.scrollLeft > 1);
    setCanScrollRight(el.scrollLeft < el.scrollWidth - el.clientWidth - 1);
  }, []);

  useEffect(() => {
    const el = scrollerRef.current;
    if (!el) return;
    updateScrollState();
    const observer = new ResizeObserver(updateScrollState);
    observer.observe(el);
    el.addEventListener("scroll", updateScrollState, { passive: true });
    window.addEventListener("resize", updateScrollState);
    return () => {
      observer.disconnect();
      el.removeEventListener("scroll", updateScrollState);
      window.removeEventListener("resize", updateScrollState);
    };
  }, [updateScrollState]);

  function scrollTabs(direction: -1 | 1): void {
    scrollerRef.current?.scrollBy({ left: 260 * direction, behavior: "smooth" });
  }

  // While the menu is open: focus its first item, close on Escape, and close on
  // any pointerdown outside it. `onMouseLeave` (below) is kept too.
  useEffect(() => {
    if (!contextMenu) return;
    firstItemRef.current?.focus();
    function onKeyDown(event: globalThis.KeyboardEvent): void {
      if (event.key === "Escape") setContextMenu(null);
    }
    function onPointerDown(event: globalThis.PointerEvent): void {
      const target = event.target as globalThis.Node | null;
      if (menuRef.current && target && menuRef.current.contains(target)) return;
      setContextMenu(null);
    }
    document.addEventListener("keydown", onKeyDown);
    document.addEventListener("pointerdown", onPointerDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
      document.removeEventListener("pointerdown", onPointerDown);
    };
  }, [contextMenu]);

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

  function handleContextMenu(event: MouseEvent, tabId: string): void {
    event.preventDefault();
    setContextMenu({ x: event.clientX, y: event.clientY, tabId });
  }

  function handleDragStart(event: DragEvent<HTMLDivElement>, tabId: string): void {
    const payload = JSON.stringify({ groupId: group.id, tabId });
    event.dataTransfer.setData(TAB_DRAG_MIME, payload);
    event.dataTransfer.effectAllowed = "move";
  }

  function closeMenu(): void {
    setContextMenu(null);
  }

  return (
    <>
      <div className="tabstrip-wrap">
        {canScrollLeft && (
          <button
            type="button"
            className="tabstrip__scroll tabstrip__scroll--left"
            aria-label="Scroll tabs left"
            title="Scroll tabs left"
            onClick={() => scrollTabs(-1)}
          >
            ‹
          </button>
        )}
        <div
          className="tabstrip"
          ref={scrollerRef}
          role="tablist"
          aria-label="Open tabs"
          title="Open tabs"
        >
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
              draggable
              onClick={() => onActivateTab(group.id, tab.id)}
              onKeyDown={(event) => handleKey(event, index)}
              onDragStart={(event) => handleDragStart(event, tab.id)}
              onContextMenu={(event) => handleContextMenu(event, tab.id)}
            >
              <span className="tab__icon" aria-hidden>
                {tab.kind === "file" ? (
                  <FileIcon name={tab.path} size={14} />
                ) : tab.kind === "skill" ? (
                  <SkillIcon />
                ) : tab.kind === "diff" ? (
                  <DiffIcon />
                ) : (
                  <TerminalIcon />
                )}
              </span>
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
        {canScrollRight && (
          <button
            type="button"
            className="tabstrip__scroll tabstrip__scroll--right"
            aria-label="Scroll tabs right"
            title="Scroll tabs right"
            onClick={() => scrollTabs(1)}
          >
            ›
          </button>
        )}
      </div>
      {contextMenu && (
        <div
          ref={menuRef}
          className="tab-context-menu"
          role="menu"
          style={{ left: contextMenu.x, top: contextMenu.y }}
          onMouseLeave={closeMenu}
        >
          <button
            ref={firstItemRef}
            role="menuitem"
            type="button"
            onClick={() => {
              onSplit?.(group.id, "right", contextMenu.tabId);
              closeMenu();
            }}
          >
            Split right
          </button>
          <button
            role="menuitem"
            type="button"
            onClick={() => {
              onSplit?.(group.id, "down", contextMenu.tabId);
              closeMenu();
            }}
          >
            Split down
          </button>
          <button
            role="menuitem"
            type="button"
            onClick={() => {
              onCloseTab(group.id, contextMenu.tabId);
              closeMenu();
            }}
          >
            Close tab
          </button>
        </div>
      )}
    </>
  );
}
