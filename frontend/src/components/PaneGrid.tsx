import {
  Fragment,
  useEffect,
  useRef,
  useState,
  type DragEvent,
} from "react";
import {
  activeTabOf,
  groupsInTree,
  tabSessionId,
  type GroupNode,
  type Layout,
  type Node,
  type SplitNode,
} from "../tabs";
import type { SessionSnapshot } from "../types";
import { AgentPane } from "./AgentPane";
import { GroupInfoBar } from "./GroupInfoBar";
import { SplitDivider } from "./SplitDivider";
import { TAB_DRAG_MIME, TabStrip } from "./TabStrip";

/** Side of a group's content a dragged tab is hovering. */
type DropSide = "left" | "right" | "top" | "bottom";

interface Props {
  layout: Layout;
  sessions: SessionSnapshot[];
  onActivateTab: (groupId: string, tabId: string) => void;
  onActivateGroup: (groupId: string) => void;
  onCloseTab: (groupId: string, tabId: string) => void;
  onAbort: (sessionId: string) => void;
  onSplit?: (groupId: string, direction: "right" | "down", tabId?: string) => void;
  onMoveTab?: (
    sourceGroupId: string,
    tabId: string,
    targetGroupId: string,
    targetSide: "left" | "right" | "top" | "bottom",
  ) => void;
  onUpdateSplitSizes?: (splitId: string, sizes: number[]) => void;
}

/** The centre workspace: render the nested split tree. */
export function PaneGrid({
  layout,
  sessions,
  onActivateTab,
  onActivateGroup,
  onCloseTab,
  onAbort,
  onSplit,
  onMoveTab,
  onUpdateSplitSizes,
}: Props) {
  const groups = groupsInTree(layout.root);
  if (groups.length === 0 || layout.root === null) {
    return (
      <div className="pane-grid pane-grid--empty">
        <p>No panes open.</p>
        <p className="pane-grid__hint">
          Open an agent from the panel on the left to watch its live stream.
        </p>
      </div>
    );
  }

  return (
    <div className="pane-grid">
      <div className="pane-groups pane-groups--tree">
        <NodeView
          node={layout.root}
          sessions={sessions}
          onActivateTab={onActivateTab}
          onActivateGroup={onActivateGroup}
          onCloseTab={onCloseTab}
          onAbort={onAbort}
          onSplit={onSplit}
          onMoveTab={onMoveTab}
          onUpdateSplitSizes={onUpdateSplitSizes}
        />
      </div>
    </div>
  );
}

interface NodeProps {
  node: Node;
  sessions: SessionSnapshot[];
  onActivateTab: (groupId: string, tabId: string) => void;
  onActivateGroup: (groupId: string) => void;
  onCloseTab: (groupId: string, tabId: string) => void;
  onAbort: (sessionId: string) => void;
  onSplit?: (groupId: string, direction: "right" | "down", tabId?: string) => void;
  onMoveTab?: (
    sourceGroupId: string,
    tabId: string,
    targetGroupId: string,
    targetSide: "left" | "right" | "top" | "bottom",
  ) => void;
  onUpdateSplitSizes?: (splitId: string, sizes: number[]) => void;
}

function NodeView({ node, ...props }: NodeProps) {
  if (node.kind === "group") {
    return <GroupView group={node} {...props} />;
  }
  return <SplitView split={node} {...props} />;
}

interface SplitProps {
  split: SplitNode;
  sessions: SessionSnapshot[];
  onActivateTab: (groupId: string, tabId: string) => void;
  onActivateGroup: (groupId: string) => void;
  onCloseTab: (groupId: string, tabId: string) => void;
  onAbort: (sessionId: string) => void;
  onSplit?: (groupId: string, direction: "right" | "down", tabId?: string) => void;
  onMoveTab?: (
    sourceGroupId: string,
    tabId: string,
    targetGroupId: string,
    targetSide: "left" | "right" | "top" | "bottom",
  ) => void;
  onUpdateSplitSizes?: (splitId: string, sizes: number[]) => void;
}

function SplitView({ split, ...props }: SplitProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  return (
    <div ref={containerRef} className={`split split--${split.orientation}`}>
      {split.children.map((child, i) => (
        <Fragment key={child.id}>
          <div className="split__pane" style={{ flex: split.sizes[i] ?? 1 }}>
            <NodeView node={child} {...props} />
          </div>
          {i < split.children.length - 1 && (
            <SplitDivider
              orientation={split.orientation}
              index={i}
              sizes={split.sizes}
              containerRef={containerRef}
              onResize={(sizes) => props.onUpdateSplitSizes?.(split.id, sizes)}
            />
          )}
        </Fragment>
      ))}
    </div>
  );
}

interface GroupProps {
  group: GroupNode;
  sessions: SessionSnapshot[];
  onActivateTab: (groupId: string, tabId: string) => void;
  onActivateGroup: (groupId: string) => void;
  onCloseTab: (groupId: string, tabId: string) => void;
  onAbort: (sessionId: string) => void;
  onSplit?: (groupId: string, direction: "right" | "down", tabId?: string) => void;
  onMoveTab?: (
    sourceGroupId: string,
    tabId: string,
    targetGroupId: string,
    targetSide: "left" | "right" | "top" | "bottom",
  ) => void;
  onUpdateSplitSizes?: (splitId: string, sizes: number[]) => void;
}

/**
 * Nearest of the four half-zones to the pointer, comparing the distance to each
 * edge normalized by the group's own width and height.
 */
function dropZone(event: DragEvent<HTMLDivElement>): DropSide {
  const rect = event.currentTarget.getBoundingClientRect();
  if (rect.width === 0 || rect.height === 0) return "right";
  const x = (event.clientX - rect.left) / rect.width;
  const y = (event.clientY - rect.top) / rect.height;
  const distances: Array<[DropSide, number]> = [
    ["left", x],
    ["right", 1 - x],
    ["top", y],
    ["bottom", 1 - y],
  ];
  let side: DropSide = "right";
  let best = Infinity;
  for (const [candidate, distance] of distances) {
    if (distance < best) {
      best = distance;
      side = candidate;
    }
  }
  return side;
}

/**
 * One split group: its tab strip, the thin info bar for the active tab, and the
 * active session's transcript. Clicking anywhere in the group makes it active
 * (where the next opened tab lands). Its content area is a drop target for tabs
 * dragged from any group.
 */
function GroupView({
  group,
  sessions,
  onActivateTab,
  onActivateGroup,
  onCloseTab,
  onAbort,
  onSplit,
  onMoveTab,
}: GroupProps) {
  const activeTab = activeTabOf(group);
  const sessionId = activeTab ? tabSessionId(activeTab) : null;
  const snapshot = sessionId
    ? sessions.find((entry) => entry.session.id === sessionId)
    : undefined;
  const [dropSide, setDropSide] = useState<DropSide | null>(null);

  // A drag can end anywhere (Escape, release outside a group), so clear the
  // indicator from the document rather than only from this group's own events.
  useEffect(() => {
    if (!dropSide) return;
    const clear = () => setDropSide(null);
    window.addEventListener("dragend", clear);
    window.addEventListener("drop", clear);
    return () => {
      window.removeEventListener("dragend", clear);
      window.removeEventListener("drop", clear);
    };
  }, [dropSide]);

  function handleDragOver(event: DragEvent<HTMLDivElement>): void {
    if (!Array.from(event.dataTransfer.types).includes(TAB_DRAG_MIME)) return;
    event.preventDefault();
    event.dataTransfer.dropEffect = "move";
    const side = dropZone(event);
    setDropSide((current) => (current === side ? current : side));
  }

  function handleDragLeave(event: DragEvent<HTMLDivElement>): void {
    const related = event.relatedTarget as globalThis.Node | null;
    if (related && event.currentTarget.contains(related)) return;
    setDropSide(null);
  }

  function handleDrop(event: DragEvent<HTMLDivElement>): void {
    const raw = event.dataTransfer.getData(TAB_DRAG_MIME);
    setDropSide(null);
    if (!raw) return;
    event.preventDefault();
    let payload: { groupId?: unknown; tabId?: unknown };
    try {
      payload = JSON.parse(raw) as { groupId?: unknown; tabId?: unknown };
    } catch {
      return;
    }
    const sourceGroupId = typeof payload.groupId === "string" ? payload.groupId : null;
    const tabId = typeof payload.tabId === "string" ? payload.tabId : null;
    if (!sourceGroupId || !tabId) return;
    if (sourceGroupId === group.id) return; // dropping back onto its own group
    onMoveTab?.(sourceGroupId, tabId, group.id, dropZone(event));
  }

  return (
    <section className="pane-group" onClick={() => onActivateGroup(group.id)}>
      <TabStrip
        group={group}
        sessions={sessions}
        onActivateTab={onActivateTab}
        onCloseTab={onCloseTab}
        onSplit={onSplit}
      />
      <div
        className="pane-group__content"
        onDragOver={handleDragOver}
        onDragLeave={handleDragLeave}
        onDrop={handleDrop}
      >
        {sessionId && (
          <>
            <GroupInfoBar sessionId={sessionId} snapshot={snapshot} onAbort={onAbort} />
            <AgentPane key={sessionId} sessionId={sessionId} />
          </>
        )}
        {dropSide && (
          <div className={`drop-indicator drop-indicator--${dropSide}`} aria-hidden="true" />
        )}
      </div>
    </section>
  );
}
