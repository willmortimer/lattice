import { assign, enqueueActions, fromPromise, setup } from "xstate";

import { getGuidanceAnchor } from "../registry";
import type { GuidanceAnchor } from "../types";

import type { TourDefinition, TourStep } from "./types";

export type TourMachineContext = {
  tour: TourDefinition | null;
  stepIndex: number;
  anchor: GuidanceAnchor | null;
  anchorRect: DOMRect | null;
  skipReason: string | null;
};

export type TourMachineEvent =
  | { type: "START"; tour: TourDefinition }
  | { type: "NEXT" }
  | { type: "SKIP" }
  | { type: "SKIP_STEP" }
  | { type: "DISMISS" }
  | { type: "STEP_RESOLVED" }
  | { type: "STEP_RETRY" }
  | { type: "STEP_ABORT" }
  | { type: "STEP_COMPLETE" };

function currentStep(context: TourMachineContext): TourStep | null {
  if (!context.tour) return null;
  return context.tour.steps[context.stepIndex] ?? null;
}

export function resolveStepAnchor(step: TourStep): GuidanceAnchor | null {
  const primary = getGuidanceAnchor(step.anchor);
  if (primary?.isAvailable()) return primary;
  if (step.fallbackAnchor) {
    const fallback = getGuidanceAnchor(step.fallbackAnchor);
    if (fallback?.isAvailable()) return fallback;
  }
  return primary ?? (step.fallbackAnchor ? getGuidanceAnchor(step.fallbackAnchor) : null) ?? null;
}

function advanceStepIndex(context: TourMachineContext): number {
  return context.stepIndex + 1;
}

function hasMoreSteps(context: TourMachineContext, nextIndex: number): boolean {
  return Boolean(context.tour && nextIndex < context.tour.steps.length);
}

type ResolveOutcome = "reveal" | "skip-step" | "skip-tour" | "complete";

function resolveStepEntry(context: TourMachineContext): {
  anchor: GuidanceAnchor | null;
  skipReason: string | null;
  nextStepIndex: number;
  outcome: ResolveOutcome;
} {
  const step = currentStep(context);
  if (!step) {
    return {
      anchor: null,
      skipReason: null,
      nextStepIndex: context.stepIndex,
      outcome: "complete",
    };
  }

  const anchor = resolveStepAnchor(step);
  if (anchor?.isAvailable()) {
    return {
      anchor,
      skipReason: null,
      nextStepIndex: context.stepIndex,
      outcome: "reveal",
    };
  }

  const reason = step.skipWhenUnavailable ? "anchor-unavailable" : "anchor-missing";
  if (
    context.stepIndex === 0 &&
    context.tour?.skipRules?.skipEntireTourWhenUnavailable &&
    reason.length > 0
  ) {
    return {
      anchor,
      skipReason: reason,
      nextStepIndex: context.stepIndex,
      outcome: "skip-tour",
    };
  }

  if (step.skipWhenUnavailable) {
    return {
      anchor,
      skipReason: reason,
      nextStepIndex: advanceStepIndex(context),
      outcome: "skip-step",
    };
  }

  return {
    anchor,
    skipReason: reason,
    nextStepIndex: context.stepIndex,
    outcome: "skip-tour",
  };
}

export const guidanceTourMachine = setup({
  types: {
    context: {} as TourMachineContext,
    events: {} as TourMachineEvent,
  },
  actors: {
    revealAnchor: fromPromise(async ({ input }: { input: { anchor: GuidanceAnchor } }) => {
      await input.anchor.reveal();
    }),
  },
  guards: {
    hasMoreSteps: ({ context }) => hasMoreSteps(context, advanceStepIndex(context)),
  },
}).createMachine({
  id: "guidanceTour",
  initial: "idle",
  context: {
    tour: null,
    stepIndex: 0,
    anchor: null,
    anchorRect: null,
    skipReason: null,
  },
  states: {
    idle: {
      on: {
        START: {
          target: "stepResolving",
          actions: assign({
            tour: ({ event }) => event.tour,
            stepIndex: 0,
            anchor: null,
            anchorRect: null,
            skipReason: null,
          }),
        },
      },
    },
    stepResolving: {
      entry: enqueueActions(({ context, enqueue }) => {
        const resolved = resolveStepEntry(context);
        enqueue.assign({
          anchor: resolved.anchor,
          anchorRect: null,
          skipReason: resolved.skipReason,
          stepIndex: resolved.nextStepIndex,
        });
        switch (resolved.outcome) {
          case "complete":
            enqueue.raise({ type: "STEP_COMPLETE" });
            break;
          case "skip-tour":
            enqueue.raise({ type: "STEP_ABORT" });
            break;
          case "skip-step":
            enqueue.raise({ type: "STEP_RETRY" });
            break;
          case "reveal":
            enqueue.raise({ type: "STEP_RESOLVED" });
            break;
          default: {
            const _exhaustive: never = resolved.outcome;
            return _exhaustive;
          }
        }
      }),
      on: {
        STEP_COMPLETE: "complete",
        STEP_ABORT: "skipped",
        STEP_RETRY: "stepResolving",
        STEP_RESOLVED: "stepRevealing",
        DISMISS: "idle",
      },
    },
    stepRevealing: {
      invoke: {
        src: "revealAnchor",
        input: ({ context }) => ({ anchor: context.anchor! }),
        onDone: "stepPositioning",
        onError: "stepPositioning",
      },
      on: {
        DISMISS: "idle",
      },
    },
    stepPositioning: {
      entry: assign(({ context }) => ({
        anchorRect: context.anchor?.getRect() ?? null,
      })),
      always: [
        {
          guard: ({ context }) => {
            const step = currentStep(context);
            if (context.anchorRect) return false;
            return Boolean(step?.skipWhenUnavailable);
          },
          target: "stepResolving",
          actions: assign(({ context }) => ({
            stepIndex: advanceStepIndex(context),
            anchor: null,
            anchorRect: null,
            skipReason: "anchor-unpositionable",
          })),
        },
        {
          guard: ({ context }) => !context.anchorRect,
          target: "skipped",
        },
        {
          target: "stepShowing",
        },
      ],
      on: {
        DISMISS: "idle",
      },
    },
    stepShowing: {
      on: {
        NEXT: [
          {
            guard: "hasMoreSteps",
            target: "stepResolving",
            actions: assign(({ context }) => ({
              stepIndex: advanceStepIndex(context),
              anchor: null,
              anchorRect: null,
              skipReason: null,
            })),
          },
          { target: "complete" },
        ],
        SKIP_STEP: [
          {
            guard: "hasMoreSteps",
            target: "stepResolving",
            actions: assign(({ context }) => ({
              stepIndex: advanceStepIndex(context),
              anchor: null,
              anchorRect: null,
              skipReason: null,
            })),
          },
          { target: "complete" },
        ],
        SKIP: "skipped",
        DISMISS: "idle",
      },
    },
    complete: {
      type: "final",
    },
    skipped: {
      type: "final",
    },
  },
});

export function isTourActiveState(stateValue: unknown): boolean {
  if (typeof stateValue === "string") {
    return stateValue.startsWith("step");
  }
  return false;
}

export function currentTourStep(context: TourMachineContext): TourStep | null {
  return currentStep(context);
}
