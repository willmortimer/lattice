export type TourPlacement = "top" | "bottom" | "left" | "right";

export type TourStep = {
  id: string;
  anchor: string;
  fallbackAnchor?: string;
  title: string;
  body?: string;
  placement?: TourPlacement;
  /** When true, unavailable anchors advance to the next step instead of ending the tour. */
  skipWhenUnavailable?: boolean;
};

export type TourSkipRules = {
  /** Skip the whole tour when the first step anchor cannot be resolved. */
  skipEntireTourWhenUnavailable?: boolean;
};

export type TourDefinition = {
  version: number;
  id: string;
  title: string;
  steps: TourStep[];
  skipRules?: TourSkipRules;
};
