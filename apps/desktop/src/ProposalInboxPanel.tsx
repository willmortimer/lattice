import { Button } from "@lattice/ui";
import { useId, useMemo, useState } from "react";

import type { ProposalSourceType, ProposalStatus, TransactionProposalSummary } from "./lib/proposals";
import { filterProposalSummaries, proposalStatusLabel } from "./lib/proposals";

export type ProposalInboxStatusFilter = "all" | ProposalStatus;
export type ProposalInboxSourceFilter = "all" | ProposalSourceType;

export interface ProposalInboxPanelProps {
  proposals: readonly TransactionProposalSummary[];
  busy?: boolean;
  loading?: boolean;
  onRefresh: () => void | Promise<void>;
  onOpen: (proposalId: string) => void | Promise<void>;
  onCreateDemo?: () => void | Promise<void>;
}

const STATUS_FILTERS: readonly ProposalInboxStatusFilter[] = [
  "all",
  "pending",
  "accepted",
  "rejected",
];

const SOURCE_FILTERS: readonly ProposalInboxSourceFilter[] = [
  "all",
  "task",
  "workflow",
  "mcp",
  "external",
];

function sourceFilterLabel(filter: ProposalInboxSourceFilter): string {
  if (filter === "all") return "All sources";
  return filter;
}

export function ProposalInboxPanel({
  proposals,
  busy = false,
  loading = false,
  onRefresh,
  onOpen,
  onCreateDemo,
}: ProposalInboxPanelProps) {
  const [statusFilter, setStatusFilter] = useState<ProposalInboxStatusFilter>("pending");
  const [sourceFilter, setSourceFilter] = useState<ProposalInboxSourceFilter>("all");
  const [pathQuery, setPathQuery] = useState("");
  const pathSearchId = useId();

  const filtered = useMemo(
    () =>
      filterProposalSummaries(proposals, {
        status: statusFilter,
        source: sourceFilter,
        pathQuery,
      }),
    [pathQuery, proposals, sourceFilter, statusFilter],
  );

  const pendingCount = useMemo(
    () => proposals.filter((item) => item.status === "pending").length,
    [proposals],
  );

  return (
    <section className="proposal-inbox" aria-label="Proposal inbox">
      <header className="proposal-inbox-head">
        <strong>Proposals</strong>
        <span className="proposal-inbox-count" aria-live="polite">
          {pendingCount} pending
        </span>
        <Button variant="ghost" size="sm" disabled={busy || loading} onClick={() => void onRefresh()}>
          Refresh
        </Button>
      </header>

      <div className="proposal-inbox-filters" role="group" aria-label="Filter proposals">
        <div className="proposal-inbox-filter-row">
          <span className="proposal-inbox-filter-label">Status</span>
          <div className="proposal-inbox-filter-chips">
            {STATUS_FILTERS.map((filter) => (
              <button
                key={filter}
                type="button"
                className={`proposal-inbox-chip${statusFilter === filter ? " is-active" : ""}`}
                aria-pressed={statusFilter === filter}
                disabled={busy}
                onClick={() => setStatusFilter(filter)}
              >
                {filter === "all" ? "All" : proposalStatusLabel(filter)}
              </button>
            ))}
          </div>
        </div>
        <div className="proposal-inbox-filter-row">
          <label className="proposal-inbox-filter-label" htmlFor={pathSearchId}>
            Path
          </label>
          <input
            id={pathSearchId}
            className="proposal-inbox-path-search"
            type="search"
            placeholder="Affected path…"
            value={pathQuery}
            disabled={busy}
            onChange={(event) => setPathQuery(event.target.value)}
          />
        </div>
        <div className="proposal-inbox-filter-row">
          <label className="proposal-inbox-filter-label" htmlFor="proposal-inbox-source-filter">
            Source
          </label>
          <select
            id="proposal-inbox-source-filter"
            className="proposal-inbox-source-select"
            value={sourceFilter}
            disabled={busy}
            onChange={(event) => setSourceFilter(event.target.value as ProposalInboxSourceFilter)}
          >
            {SOURCE_FILTERS.map((filter) => (
              <option key={filter} value={filter}>
                {sourceFilterLabel(filter)}
              </option>
            ))}
          </select>
        </div>
      </div>

      {loading ? (
        <p className="proposal-inbox-empty" role="status">
          Loading proposals…
        </p>
      ) : filtered.length === 0 ? (
        <p className="proposal-inbox-empty" role="status">
          {proposals.length === 0
            ? "No proposals yet."
            : "No proposals match the current filters."}
        </p>
      ) : (
        <ul className="proposal-inbox-list" aria-live="polite">
          {filtered.map((item) => {
            const isPending = item.status === "pending";
            const statusLabel = proposalStatusLabel(item.status);
            const pathHint =
              item.affectedPaths.length > 0 ? item.affectedPaths.slice(0, 2).join(", ") : null;
            return (
              <li key={item.id}>
                <button
                  type="button"
                  className={`proposal-inbox-item${isPending ? "" : " is-archived"}`}
                  disabled={busy || !isPending}
                  aria-disabled={!isPending}
                  onClick={() => {
                    if (isPending) void onOpen(item.id);
                  }}
                >
                  <span className="proposal-inbox-item-head">
                    <strong>{item.summary}</strong>
                    <span className={`proposal-inbox-status is-${item.status}`}>{statusLabel}</span>
                  </span>
                  <small>
                    {item.commandCount} command{item.commandCount === 1 ? "" : "s"} · {item.source.type}
                    {pathHint ? ` · ${pathHint}` : ""}
                  </small>
                  {item.status === "accepted" && item.appliedTransactionId && (
                    <small className="proposal-inbox-transaction">
                      Transaction {item.appliedTransactionId}
                    </small>
                  )}
                </button>
              </li>
            );
          })}
        </ul>
      )}

      <p className="proposal-inbox-note">
        Accepted proposals are archived on disk (commands cleared). Superseded is not a separate
        status — workflow retries reuse the pending idempotency key.
      </p>

      {onCreateDemo && (
        <Button
          variant="ghost"
          size="sm"
          className="proposal-inbox-demo"
          disabled={busy || loading}
          onClick={() => void onCreateDemo()}
        >
          Create demo proposal
        </Button>
      )}
    </section>
  );
}
