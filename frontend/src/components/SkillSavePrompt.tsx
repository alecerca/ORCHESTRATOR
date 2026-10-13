import { useEffect } from "react";

interface Props {
  /** The skill whose tab is closing (its on-disk filename stem). */
  name: string;
  /** True while a save is in flight (buttons disabled). */
  busy: boolean;
  /** Save failure surfaced in-line; the tab stays open on error. */
  error: string | null;
  onSave: () => void;
  onDiscard: () => void;
  onCancel: () => void;
}

/**
 * Save-on-close gate for dirty skill tabs (skills only — every other tab keeps
 * today's close behaviour). Styled like the approval modal: same backdrop,
 * surface and action row.
 */
export function SkillSavePrompt({ name, busy, error, onSave, onDiscard, onCancel }: Props) {
  // Escape cancels the close (the tab stays open), matching the other modals.
  useEffect(() => {
    function onKey(event: KeyboardEvent): void {
      if (event.key === "Escape") onCancel();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onCancel]);

  return (
    <div className="modal-backdrop">
      <div
        className="modal approval"
        role="dialog"
        aria-modal="true"
        aria-labelledby="skill-save-title"
      >
        <div className="approval__head">
          <span className="chip chip--pending">skill</span>
          <span className="approval__type">unsaved changes</span>
        </div>
        <h2 className="approval__title" id="skill-save-title">Save skill?</h2>
        <p className="skill-prompt__body">
          <span className="skill-prompt__name">{name}</span> has unsaved changes.
        </p>
        {error && (
          <p className="approval__error" role="alert">
            {error}
          </p>
        )}
        <div className="approval__actions">
          <button type="button" className="btn btn--danger" disabled={busy} onClick={onDiscard}>
            Discard
          </button>
          <button type="button" className="btn" disabled={busy} autoFocus onClick={onCancel}>
            Cancel
          </button>
          <button type="button" className="btn btn--primary" disabled={busy} onClick={onSave}>
            Save
          </button>
        </div>
      </div>
    </div>
  );
}
