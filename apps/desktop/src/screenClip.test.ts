import { describe, expect, it } from "vitest";

import {
  CAPTURE_CANCELLED_EVENT,
  CAPTURE_ERROR_EVENT,
  CAPTURE_INGESTED_EVENT,
} from "./screenClip";

describe("screenClip events", () => {
  it("exports stable capture event names for the desktop shell", () => {
    expect(CAPTURE_INGESTED_EVENT).toBe("capture-ingested");
    expect(CAPTURE_CANCELLED_EVENT).toBe("capture-cancelled");
    expect(CAPTURE_ERROR_EVENT).toBe("capture-error");
  });
});
