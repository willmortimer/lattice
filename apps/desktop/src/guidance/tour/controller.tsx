import { useEffect, useState } from "react";

import { GuidanceTourHost } from "./GuidanceTourHost";
import { sampleShellTour } from "./sampleTour";
import { subscribeShellTourStart } from "./shellTourBridge";
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

export function GuidanceTourController() {
  const [activeTour, setActiveTour] = useState<TourDefinition | null>(null);

  useEffect(() => {
    startTourHandler = setActiveTour;
    return () => {
      startTourHandler = null;
    };
  }, []);

  useEffect(() => subscribeShellTourStart(() => startSampleShellTour()), []);

  return <GuidanceTourHost tour={activeTour} onFinished={() => setActiveTour(null)} />;
}
