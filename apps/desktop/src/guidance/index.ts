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
