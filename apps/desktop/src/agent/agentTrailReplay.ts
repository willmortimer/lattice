import {
  applyActiveOverlays,
  type OverlayClearFn,
} from "./agentOverlayEffects";
import type {
  ActiveOverlay,
  AgentFollowMode,
  TrailReplaySnapshot,
  TrailStep,
} from "./agentStore";

export type { TrailReplaySnapshot };

export function canReplayTrailStep(step: TrailStep): boolean {
  return Boolean(step.anchors && step.anchors.length > 0);
}

export function overlayFromTrailStep(step: TrailStep): ActiveOverlay | null {
  if (!canReplayTrailStep(step)) {
    return null;
  }

  return {
    overlayId: step.overlayId ?? `replay-${step.stepId}`,
    runId: step.runId,
    anchors: step.anchors!,
    purpose: step.purpose ?? "attention",
    ...(step.commentary !== undefined ? { commentary: step.commentary } : {}),
  };
}

export function replayTrailStep(
  step: TrailStep,
  followMode: AgentFollowMode,
): OverlayClearFn[] {
  const overlay = overlayFromTrailStep(step);
  if (!overlay) {
    return [];
  }

  return applyActiveOverlays({ [overlay.overlayId]: overlay }, followMode);
}
