import { useEffect, useState } from "react";

import { GuidanceTourHost } from "./GuidanceTourHost";
import { sampleShellTour } from "./sampleTour";
import { subscribeShellTourStart } from "./shellTourBridge";
import type { ShellTourOutcome } from "./shellTourPersistence";
import type { TourDefinition } from "./types";

let startTourHandler: ((tour: TourDefinition) => void) | null = null;

export function startGuidanceTour(tour: TourDefinition): void {
  startTourHandler?.(tour);
}

/** Start the built-in workspace shell quick-start tour. */
export function startSampleShellTour(): void {
  startGuidanceTour(sampleShellTour);
}

/** @deprecated Use {@link startSampleShellTour} */
export function startSampleGuidanceTour(): void {
  startSampleShellTour();
}

type GuidanceTourControllerProps = {
  onShellTourFinished?: (outcome: ShellTourOutcome) => void;
};

export function GuidanceTourController({ onShellTourFinished }: GuidanceTourControllerProps) {
  const [activeTour, setActiveTour] = useState<TourDefinition | null>(null);

  useEffect(() => {
    startTourHandler = setActiveTour;
    return () => {
      startTourHandler = null;
    };
  }, []);

  useEffect(() => subscribeShellTourStart(() => startSampleShellTour()), []);

  return (
    <GuidanceTourHost
      tour={activeTour}
      onFinished={(outcome) => {
        setActiveTour(null);
        if (outcome === "completed" || outcome === "skipped") {
          onShellTourFinished?.(outcome);
        }
      }}
    />
  );
}
