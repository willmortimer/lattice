/**
 * One user-facing conflict presentation for save conflicts (stale revision)
 * and external-edit conflicts alike (ADR 0028: present all conflicts as
 * incompatible resource revisions, through one envelope, with a small set
 * of universal actions).
 */

export interface ConflictAction {
  label: string;
  onClick: () => void;
  /** Defaults to "secondary". Exactly one action per envelope should be primary. */
  variant?: "primary" | "secondary";
}

interface ConflictEnvelopeProps {
  message: string;
  actions: ConflictAction[];
}

export function ConflictEnvelope({ message, actions }: ConflictEnvelopeProps) {
  return (
    <div className="conflict-banner">
      <span className="conflict-banner-copy">{message}</span>
      <div className="conflict-banner-actions">
        {actions.map((action) => (
          <button
            key={action.label}
            className={action.variant === "primary" ? "primary-button" : "secondary-button"}
            onClick={action.onClick}
          >
            {action.label}
          </button>
        ))}
      </div>
    </div>
  );
}
