import { useMachine } from "@xstate/react";
import { Button, PopoverPortal, PopoverPopup, PopoverRoot } from "@lattice/ui";
import { useEffect, useRef } from "react";

import { FloatingPopoverPositioner } from "../../overlay/FloatingPopoverPositioner";
import { tourSideToPlacement } from "../../overlay/floatingPosition";

import "./guidanceTour.css";
import { GuidanceSpotlight } from "./spotlight";
import { currentTourStep, guidanceTourMachine, isTourActiveState } from "./machine";
import type { TourDefinition } from "./types";
import type { ShellTourOutcome } from "./shellTourPersistence";

type GuidanceTourHostProps = {
  tour: TourDefinition | null;
  onFinished?: (outcome: ShellTourOutcome) => void;
};

export function GuidanceTourHost({ tour, onFinished }: GuidanceTourHostProps) {
  const [snapshot, send] = useMachine(guidanceTourMachine);
  const active = isTourActiveState(snapshot.value);
  const step = currentTourStep(snapshot.context);
  const showing = snapshot.matches("stepShowing");
  const finishedRef = useRef(false);

  useEffect(() => {
    if (!tour) return;
    finishedRef.current = false;
    send({ type: "START", tour });
  }, [send, tour]);

  useEffect(() => {
    if (!onFinished || snapshot.status !== "done" || finishedRef.current) return;
    finishedRef.current = true;
    const outcome: ShellTourOutcome = snapshot.value === "skipped" ? "skipped" : "completed";
    onFinished(outcome);
  }, [onFinished, snapshot.status, snapshot.value]);

  if (!active || !step) return null;

  return (
    <>
      <GuidanceSpotlight rect={snapshot.context.anchorRect} />
      <PopoverRoot open={showing}>
        <PopoverPortal>
          <FloatingPopoverPositioner
            rect={snapshot.context.anchorRect}
            placement={tourSideToPlacement(step.placement ?? "bottom")}
            sideOffset={12}
            enabled={showing}
            style={{ zIndex: 1201 }}
          >
            <PopoverPopup className="guidance-tour-popover" role="dialog" aria-label={step.title}>
              <div className="guidance-tour-popover__eyebrow">Tour</div>
              <h2 className="guidance-tour-popover__title">{step.title}</h2>
              {step.body ? <p className="guidance-tour-popover__body">{step.body}</p> : null}
              <div className="guidance-tour-popover__actions">
                <Button variant="ghost" size="sm" onClick={() => send({ type: "SKIP" })}>
                  Skip tour
                </Button>
                <Button variant="ghost" size="sm" onClick={() => send({ type: "SKIP_STEP" })}>
                  Skip step
                </Button>
                <Button variant="primary" size="sm" onClick={() => send({ type: "NEXT" })}>
                  Next
                </Button>
              </div>
            </PopoverPopup>
          </FloatingPopoverPositioner>
        </PopoverPortal>
      </PopoverRoot>
    </>
  );
}
