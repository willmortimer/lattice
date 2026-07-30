import { describe, expect, it } from "vitest";

import {
  isPackDownloadDisabled,
  packDownloadButtonLabel,
  packStatusLabel,
} from "./packStatusLabels";
import type { PackStatus } from "../lib/packs";

const ALL: PackStatus[] = ["missing", "downloading", "ready", "failed", "unavailable"];

describe("packStatusLabel", () => {
  it("labels every pack status", () => {
    expect(packStatusLabel("missing")).toBe("Not downloaded");
    expect(packStatusLabel("downloading")).toBe("Downloading…");
    expect(packStatusLabel("ready")).toBe("Ready");
    expect(packStatusLabel("failed")).toBe("Failed");
    expect(packStatusLabel("unavailable")).toBe("Unavailable");
    expect(ALL).toHaveLength(5);
  });
});

describe("packDownloadButtonLabel", () => {
  it("prefers busy and lifecycle state", () => {
    expect(packDownloadButtonLabel("missing", false)).toBe("Download");
    expect(packDownloadButtonLabel("missing", true)).toBe("Downloading…");
    expect(packDownloadButtonLabel("downloading", false)).toBe("Downloading…");
    expect(packDownloadButtonLabel("ready", false)).toBe("Downloaded");
    expect(packDownloadButtonLabel("failed", false)).toBe("Retry download");
    expect(packDownloadButtonLabel("unavailable", false)).toBe("Unavailable");
  });
});

describe("isPackDownloadDisabled", () => {
  it("blocks ready, downloading, unavailable, and busy", () => {
    expect(isPackDownloadDisabled("missing", false)).toBe(false);
    expect(isPackDownloadDisabled("failed", false)).toBe(false);
    expect(isPackDownloadDisabled("ready", false)).toBe(true);
    expect(isPackDownloadDisabled("downloading", false)).toBe(true);
    expect(isPackDownloadDisabled("unavailable", false)).toBe(true);
    expect(isPackDownloadDisabled("missing", true)).toBe(true);
  });
});
