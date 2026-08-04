import { useEffect, useMemo, useRef } from "react";

import type { AgentRunStatus } from "../lib/agentRunEvents";
import {
  eventsAfterSequence,
  isKernelfsLifecycleEventType,
  projectAgentRunEvents,
  type AgentRunProjection,
} from "../query/agentRunProjection";
import { useAgentRunEventsQuery } from "../query/useAgentRunEventsQuery";
import { useAgentRunActiveQuery } from "../query/useAgentRunStatusQuery";
import { useAgentSessionStore } from "./agentStore";

export type AgentRunWorkbenchModel = {
  runId: string | null;
  status: AgentRunStatus | null;
  projection: AgentRunProjection | null;
};

/**
 * Resolve the durable run for the active thread, project events for the
 * workbench, and one-shot hydrate chat spatial trail/evidence with a sequence
 * cursor so live `recordAgentEvent` does not double-apply the same rows.
 */
export function useAgentRunWorkbenchModel(
  workspaceRoot: string | null,
  threadId: string | null,
): AgentRunWorkbenchModel {
  const recordAgentEvent = useAgentSessionStore((state) => state.recordAgentEvent);
  const trailSteps = useAgentSessionStore((state) => state.trailSteps);

  const { data: activeStatus } = useAgentRunActiveQuery(workspaceRoot, threadId);
  const runId = activeStatus?.run?.runId ?? null;
  const status = activeStatus?.run?.status ?? null;

  const { data: eventsResult } = useAgentRunEventsQuery(workspaceRoot, runId);

  const projection = useMemo(() => {
    if (!eventsResult) {
      return null;
    }
    return projectAgentRunEvents(eventsResult.events, {
      runId: eventsResult.runId,
      run: eventsResult.run,
    });
  }, [eventsResult]);

  const hydrateCursorRef = useRef<{ runId: string; afterSequence: number } | null>(null);

  useEffect(() => {
    if (!runId || !eventsResult) {
      return;
    }

    if (hydrateCursorRef.current?.runId !== runId) {
      const hasLiveTrail = trailSteps.some((step) => step.runId === runId);
      // Live stream already populated this run — advance cursor to avoid replay.
      const initialSequence = hasLiveTrail
        ? Math.max(
            eventsResult.run.lastSequence,
            ...eventsResult.events.map((event) => event.eventSequence),
            0,
          )
        : 0;
      hydrateCursorRef.current = { runId, afterSequence: initialSequence };
      if (hasLiveTrail) {
        return;
      }
    }

    const cursor = hydrateCursorRef.current;
    if (!cursor || cursor.runId !== runId) {
      return;
    }

    const pending = eventsAfterSequence(eventsResult.events, cursor.afterSequence);
    if (pending.length === 0) {
      return;
    }

    let afterSequence = cursor.afterSequence;
    for (const event of pending) {
      if (!isKernelfsLifecycleEventType(event.eventType)) {
        recordAgentEvent(event.payload);
      }
      afterSequence = Math.max(afterSequence, event.eventSequence);
    }
    hydrateCursorRef.current = { runId, afterSequence };
  }, [runId, eventsResult, recordAgentEvent, trailSteps]);

  return { runId, status, projection };
}

export function formatAgentRunStatusLabel(status: AgentRunStatus | null): string | null {
  if (!status) {
    return null;
  }
  switch (status) {
    case "running":
      return "Running";
    case "completed":
      return "Completed";
    case "failed":
      return "Failed";
    case "cancelled":
      return "Cancelled";
    default: {
      const _exhaustive: never = status;
      return _exhaustive;
    }
  }
}
