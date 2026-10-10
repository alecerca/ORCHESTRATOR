import { useEffect, useSyncExternalStore } from "react";
import { ensureDraft, getDraft, setContent, subscribe } from "../skillDrafts";
import { MonacoEditor } from "./MonacoEditor";

interface Props {
  /** The tab this editor backs; draft state is keyed by it. */
  tabId: string;
  /** The skill being edited (its on-disk filename stem). */
  name: string;
  theme: "dark" | "light";
}

/**
 * One open skill document in a group: reads through the draft store (which
 * loads via `readSkill` and scaffolds missing files) and every keystroke
 * writes back into the draft. Nothing reaches the disk until the user saves —
 * see `SkillSavePrompt` in App.tsx for that path.
 */
export function SkillEditor({ tabId, name, theme }: Props) {
  useEffect(() => {
    ensureDraft(tabId, name);
    // The draft is keyed by tab id; `name` is fixed at tab creation.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tabId, name]);

  const draft = useSyncExternalStore(
    subscribe,
    () => getDraft(tabId),
    () => getDraft(tabId),
  );

  if (!draft || draft.loading) {
    return (
      <div className="skill-editor">
        <p className="skill-editor__muted">Loading skill…</p>
      </div>
    );
  }

  return (
    <div className="skill-editor">
      <MonacoEditor
        path={`${name}.md`}
        value={draft.content}
        theme={theme}
        onChange={(value) => setContent(tabId, value)}
      />
    </div>
  );
}