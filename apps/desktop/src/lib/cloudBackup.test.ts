import { describe, expect, it, vi } from "vitest";

vi.mock("../demo", () => ({
  inBrowser: false,
}));

import {
  cloudBackupErrorMessage,
  isCloudBackupResource,
} from "./cloudBackup";

describe("cloudBackupErrorMessage", () => {
  it("maps unsigned cloud errors to Settings guidance", () => {
    expect(
      cloudBackupErrorMessage(
        new Error(
          "not signed in to cloud; sign in via desktop Settings → Cloud account",
        ),
      ),
    ).toBe("Sign in under Settings → Cloud account to back up resources.");
  });

  it("preserves other error text", () => {
    expect(cloudBackupErrorMessage(new Error("upload failed"))).toBe("upload failed");
    expect(cloudBackupErrorMessage("network timeout")).toBe("network timeout");
  });
});

describe("isCloudBackupResource", () => {
  it("excludes folders", () => {
    expect(isCloudBackupResource("folder")).toBe(false);
    expect(isCloudBackupResource("page")).toBe(true);
    expect(isCloudBackupResource("file")).toBe(true);
  });
});
