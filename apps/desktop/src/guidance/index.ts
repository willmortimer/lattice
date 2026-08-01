export type { GuidanceAnchor } from "./types";
export {
  GUIDANCE_ANCHOR_ATTR,
  createDomGuidanceAnchor,
  elementRect,
  queryGuidanceAnchorElement,
} from "./domAnchor";
export {
  clearGuidanceAnchors,
  getGuidanceAnchor,
  listGuidanceAnchors,
  registerGuidanceAnchor,
} from "./registry";
export {
  createAgentHighlightDomAnchor,
  createAgentOverlayGuidanceAnchor,
} from "./agentBridge";
export {
  createDefaultGuidanceAnchors,
  DEFAULT_GUIDANCE_ANCHOR_IDS,
  seedGuidanceAnchors,
  type DefaultGuidanceAnchorId,
} from "./seedAnchors";
export {
  getAnchorRectForAriaLabel,
  queryGuidanceAnchorElementForAriaLabel,
  resolveGuidanceAnchorFromAriaLabel,
  resolveGuidanceAnchorIdFromAriaLabel,
  revealGuidanceAnchorForAriaLabel,
} from "./demoBridge";
export { GuidanceTourController, startGuidanceTour, startSampleGuidanceTour } from "./tour/controller";
export { GuidanceTourHost } from "./tour/GuidanceTourHost";
export { guidanceTourMachine, resolveStepAnchor } from "./tour/machine";
export { sampleShellTour } from "./tour/sampleTour";
export type { TourDefinition, TourPlacement, TourSkipRules, TourStep } from "./tour/types";
