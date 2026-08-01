import { Button } from "@lattice/ui";

export function SettingsSectionToolbar({
  hasDraft,
  busy,
  onApply,
  onResetSection,
}: {
  hasDraft: boolean;
  busy: boolean;
  onApply: () => void;
  onResetSection: () => void;
}) {
  return (
    <div className="settings-section-toolbar" role="group" aria-label="Section actions">
      {hasDraft ? (
        <span className="settings-draft-indicator" role="status">
          Unsaved changes
        </span>
      ) : null}
      <Button size="sm" disabled={!hasDraft || busy} onClick={onApply}>
        {busy ? "Applying…" : "Apply"}
      </Button>
      <Button size="sm" variant="secondary" disabled={busy} onClick={onResetSection}>
        Reset section
      </Button>
    </div>
  );
}
