import { Button } from "@lattice/ui";

import type { TransactionProposalSummary } from "./lib/proposals";

export interface ProposalInboxPanelProps {
  proposals: readonly TransactionProposalSummary[];
  busy?: boolean;
  onRefresh: () => void | Promise<void>;
  onOpen: (proposalId: string) => void | Promise<void>;
  onCreateDemo?: () => void | Promise<void>;
}

export function ProposalInboxPanel({
  proposals,
  busy = false,
  onRefresh,
  onOpen,
  onCreateDemo,
}: ProposalInboxPanelProps) {
  return (
    <section className="proposal-inbox" aria-label="Pending proposals">
      <header className="proposal-inbox-head">
        <strong>Proposals</strong>
        <span className="proposal-inbox-count">{proposals.length}</span>
        <Button variant="ghost" size="sm" disabled={busy} onClick={() => void onRefresh()}>
          Refresh
        </Button>
      </header>
      {proposals.length === 0 ? (
        <p className="proposal-inbox-empty">No pending proposals.</p>
      ) : (
        <ul className="proposal-inbox-list">
          {proposals.map((item) => (
            <li key={item.id}>
              <button
                type="button"
                className="proposal-inbox-item"
                disabled={busy}
                onClick={() => void onOpen(item.id)}
              >
                <strong>{item.summary}</strong>
                <small>
                  {item.commandCount} command{item.commandCount === 1 ? "" : "s"} ·{" "}
                  {item.source.type}
                </small>
              </button>
            </li>
          ))}
        </ul>
      )}
      {onCreateDemo && (
        <Button
          variant="ghost"
          size="sm"
          className="proposal-inbox-demo"
          disabled={busy}
          onClick={() => void onCreateDemo()}
        >
          Create demo proposal
        </Button>
      )}
    </section>
  );
}
