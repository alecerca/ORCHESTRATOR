import { activeTabOf, tabSessionId, type Layout, type TabGroup } from "../tabs";
import type { SessionSnapshot } from "../types";
import { AgentPane } from "./AgentPane";
import { GroupInfoBar } from "./GroupInfoBar";
import { TabStrip } from "./TabStrip";

interface Props {
  layout: Layout;
  sessions: SessionSnapshot[];
  onActivateTab: (groupId: string, tabId: string) => void;
  onActivateGroup: (groupId: string) => void;
  onCloseTab: (groupId: string, tabId: string) => void;
  onAbort: (sessionId: string) => void;
}

/** The centre workspace: every split group rendered side by side. */
export function PaneGrid({
  layout,
  sessions,
  onActivateTab,
  onActivateGroup,
  onCloseTab,
  onAbort,
}: Props) {
  if (layout.groups.length === 0) {
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
      <div className="pane-groups">
        {layout.groups.map((group) => (
          <GroupView
            key={group.id}
            group={group}
            sessions={sessions}
            onActivateTab={onActivateTab}
            onActivateGroup={onActivateGroup}
            onCloseTab={onCloseTab}
            onAbort={onAbort}
          />
        ))}
      </div>
    </div>
  );
}

interface GroupProps {
  group: TabGroup;
  sessions: SessionSnapshot[];
  onActivateTab: (groupId: string, tabId: string) => void;
  onActivateGroup: (groupId: string) => void;
  onCloseTab: (groupId: string, tabId: string) => void;
  onAbort: (sessionId: string) => void;
}

/**
 * One split group: its tab strip, the thin info bar for the active tab, and the
 * active session's transcript. Clicking anywhere in the group makes it active
 * (where the next opened tab lands).
 */
function GroupView({
  group,
  sessions,
  onActivateTab,
  onActivateGroup,
  onCloseTab,
  onAbort,
}: GroupProps) {
  const activeTab = activeTabOf(group);
  const sessionId = activeTab ? tabSessionId(activeTab) : null;
  const snapshot = sessionId
    ? sessions.find((entry) => entry.session.id === sessionId)
    : undefined;

  return (
    <section className="pane-group" onClick={() => onActivateGroup(group.id)}>
      <TabStrip
        group={group}
        sessions={sessions}
        onActivateTab={onActivateTab}
        onCloseTab={onCloseTab}
      />
      {sessionId && (
        <>
          <GroupInfoBar sessionId={sessionId} snapshot={snapshot} onAbort={onAbort} />
          <AgentPane key={sessionId} sessionId={sessionId} />
        </>
      )}
    </section>
  );
}
