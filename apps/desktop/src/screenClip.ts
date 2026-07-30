export const CAPTURE_INGESTED_EVENT = "capture-ingested";
export const CAPTURE_CANCELLED_EVENT = "capture-cancelled";
export const CAPTURE_ERROR_EVENT = "capture-error";

export interface CaptureIngestedPayload {
  pagePath: string;
  assetPath: string;
  root: string;
}
