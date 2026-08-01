import { useEffect, useState } from "react";

import { GuidanceTourHost } from "./GuidanceTourHost";
import { sampleShellTour } from "./sampleTour";
import type { TourDefinition } from "./types";

let startTourHandler: ((tour: TourDefinition) => void) | null = null;

export function startGuidanceTour(tour: TourDefinition): void {
  startTourHandler?.(tour);
}

export function startSampleGuidanceTour(): void {
  startGuidanceTour(sampleShellTour);
}

export function GuidanceTourController() {
  const [activeTour, setActiveTour] = useState<TourDefinition | null>(null);

  useEffect(() => {
    startTourHandler = setActiveTour;
    return () => {
      startTourHandler = null;
    };
  }, []);

  return <GuidanceTourHost tour={activeTour} onFinished={() => setActiveTour(null)} />;
}
