import { useEffect, useRef, useState } from "react";

import { useAgentSessionStore } from "./agentStore";
import { clearOverlayHighlights, type OverlayClearFn } from "./agentOverlayEffects";
import { canReplayTrailStep, replayTrailStep } from "./agentTrailReplay";

function trailStepKey(step: { runId: string; stepId: string }): string {
  return `${step.runId}:${step.stepId}`;
}

function formatTrailKind(kind: string): string {
  return kind.replace(/_/g, " ");
}

export function AgentTrail() {
  const trailSteps = useAgentSessionStore((state) => state.trailSteps);
  const followMode = useAgentSessionStore((state) => state.followMode);
  const clearsRef = useRef<OverlayClearFn[]>([]);
  const [activeReplayKey, setActiveReplayKey] = useState<string | null>(null);

  useEffect(() => {
    return () => {
      clearOverlayHighlights(clearsRef.current);
      clearsRef.current = [];
    };
  }, []);

  if (trailSteps.length === 0) {
    return null;
  }

  const handleReplay = (step: (typeof trailSteps)[number]) => {
    if (!canReplayTrailStep(step)) {
      return;
    }

    clearOverlayHighlights(clearsRef.current);
    clearsRef.current = replayTrailStep(step, followMode);
    setActiveReplayKey(trailStepKey(step));
  };

  return (
    <nav className="agent-trail" aria-label="Agent trail">
      <div className="agent-trail-header">
        <span className="agent-trail-eyebrow">Trail</span>
      </div>
      <ol className="agent-trail-list">
        {trailSteps.map((step) => {
          const replayable = canReplayTrailStep(step);
          const key = trailStepKey(step);
          const isActive = activeReplayKey === key;

          return (
            <li key={key} className="agent-trail-item">
              {replayable ? (
                <button
                  type="button"
                  className={`agent-trail-step${isActive ? " agent-trail-step-active" : ""}`}
                  onClick={() => handleReplay(step)}
                  aria-pressed={isActive}
                >
                  <TrailStepContent step={step} />
                </button>
              ) : (
                <div className="agent-trail-step agent-trail-step-static" aria-disabled="true">
                  <TrailStepContent step={step} />
                </div>
              )}
            </li>
          );
        })}
      </ol>
    </nav>
  );
}

function TrailStepContent({
  step,
}: {
  step: {
    kind: string;
    label: string;
    status: "in_progress" | "completed";
    summary?: string;
  };
}) {
  const detail = step.summary ?? step.label;

  return (
    <>
      <span className="agent-trail-step-kind">{formatTrailKind(step.kind)}</span>
      <span className="agent-trail-step-label">{detail}</span>
      <span
        className={`agent-trail-step-status agent-trail-step-status-${step.status.replace("_", "-")}`}
        aria-label={step.status === "in_progress" ? "In progress" : "Completed"}
      />
    </>
  );
}
