import { describe, expect, it } from "vitest";

import { isCloudAiEntitled } from "./cloud";

describe("isCloudAiEntitled", () => {
  it("requires a signed-in session", () => {
    expect(isCloudAiEntitled({ signedIn: false })).toBe(false);
  });

  it("treats legacy signed-in sessions without entitlements as entitled", () => {
    expect(isCloudAiEntitled({ signedIn: true })).toBe(true);
  });

  it("allows allowlisted and paid access", () => {
    expect(
      isCloudAiEntitled({
        signedIn: true,
        entitlements: {
          ai_access: "allowlisted",
          ai_daily_request_budget: 200,
          ai_daily_requests_used: 0,
        },
      }),
    ).toBe(true);
    expect(
      isCloudAiEntitled({
        signedIn: true,
        entitlements: {
          ai_access: "paid",
          ai_daily_request_budget: 200,
          ai_daily_requests_used: 1,
        },
      }),
    ).toBe(true);
  });

  it("denies none access", () => {
    expect(
      isCloudAiEntitled({
        signedIn: true,
        entitlements: {
          ai_access: "none",
          ai_daily_request_budget: 200,
          ai_daily_requests_used: 0,
        },
      }),
    ).toBe(false);
  });
});
