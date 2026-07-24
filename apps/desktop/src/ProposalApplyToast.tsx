import { Button } from "@lattice/ui";

export interface ProposalApplyToastProps {
  transactionId: string;
  openPaths: readonly string[];
  onOpenPath: (path: string) => void | Promise<void>;
  onDismiss: () => void;
}

export function ProposalApplyToast({
  transactionId,
  openPaths,
  onOpenPath,
  onDismiss,
}: ProposalApplyToastProps) {
  return (
    <div className="proposal-apply-toast" role="status" aria-live="polite">
      <div className="proposal-apply-toast-copy">
        <strong>Proposal applied</strong>
        <span className="proposal-apply-toast-transaction">Transaction {transactionId}</span>
      </div>
      <div className="proposal-apply-toast-actions">
        {openPaths.map((path) => (
          <Button key={path} size="sm" onClick={() => void onOpenPath(path)}>
            Open {path}
          </Button>
        ))}
        <Button variant="ghost" size="sm" onClick={onDismiss}>
          Dismiss
        </Button>
      </div>
    </div>
  );
}
