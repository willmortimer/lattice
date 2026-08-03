import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../demo", () => ({
  inBrowser: false,
}));

const cloudBlobOpen = vi.fn();
const cloudBlobMaterialize = vi.fn();
const getCloudSessionStatus = vi.fn();

vi.mock("./cloud", () => ({
  cloudBlobOpen: (...args: unknown[]) => cloudBlobOpen(...args),
  cloudBlobMaterialize: (...args: unknown[]) => cloudBlobMaterialize(...args),
  getCloudSessionStatus: (...args: unknown[]) => getCloudSessionStatus(...args),
}));

const invoke = vi.fn();

vi.mock("./ipc", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));

import {
  CLOUD_BLOB_CONFLICT_MESSAGE,
  cloudBackupErrorMessage,
  isCloudBackupResource,
  isCloudBlobConflictError,
  reopenResourceFromCloud,
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

  it("maps HTTP 409 blob binding conflicts to actionable copy", () => {
    expect(
      cloudBackupErrorMessage(
        new Error(
          "cloud blob error: cloud API error (409): resource already bound to a different content hash",
        ),
      ),
    ).toBe(CLOUD_BLOB_CONFLICT_MESSAGE);
  });

  it("maps in-memory duplicate-put conflicts", () => {
    expect(
      cloudBackupErrorMessage(
        new Error("blob already exists for resource: 00000000-0000-0000-0000-000000000001"),
      ),
    ).toBe(CLOUD_BLOB_CONFLICT_MESSAGE);
  });

  it("preserves other error text", () => {
    expect(cloudBackupErrorMessage(new Error("upload failed"))).toBe("upload failed");
    expect(cloudBackupErrorMessage("network timeout")).toBe("network timeout");
  });
});

describe("isCloudBlobConflictError", () => {
  it("detects 409 and duplicate-put shapes", () => {
    expect(isCloudBlobConflictError("cloud API error (409): conflict")).toBe(true);
    expect(isCloudBlobConflictError("blob already exists for resource: abc")).toBe(true);
    expect(isCloudBlobConflictError("HTTP 409")).toBe(true);
    expect(isCloudBlobConflictError("cloud API error (503): unavailable")).toBe(false);
  });
});

describe("isCloudBackupResource", () => {
  it("excludes folders", () => {
    expect(isCloudBackupResource("folder")).toBe(false);
    expect(isCloudBackupResource("page")).toBe(true);
    expect(isCloudBackupResource("file")).toBe(true);
  });
});

describe("reopenResourceFromCloud", () => {
  beforeEach(() => {
    cloudBlobOpen.mockReset();
    invoke.mockReset();
  });

  it("hydrates UTF-8 cloud bytes through apply_page_update", async () => {
    const text = "# from cloud\n";
    cloudBlobOpen.mockResolvedValue([...new TextEncoder().encode(text)]);
    invoke.mockImplementation(async (command: string) => {
      if (command === "read_page") {
        return { content: "# stale local\n", revision: "sha256:abc" };
      }
      if (command === "apply_page_update") {
        return "sha256:def";
      }
      throw new Error(`unexpected invoke ${command}`);
    });

    const result = await reopenResourceFromCloud("/ws", "Notes.md");
    expect(result).toEqual({
      ok: true,
      byteLength: text.length,
      hydrated: true,
      content: text,
      revision: "sha256:def",
    });
    expect(invoke).toHaveBeenCalledWith("apply_page_update", {
      root: "/ws",
      relPath: "Notes.md",
      content: text,
      baseRevision: "sha256:abc",
    });
  });

  it("returns best-effort when cloud bytes are not UTF-8", async () => {
    cloudBlobOpen.mockResolvedValue([0xff, 0xfe, 0xfd]);
    const result = await reopenResourceFromCloud("/ws", "photo.png");
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.byteLength).toBe(3);
    expect(result.hydrated).toBe(false);
    if (result.hydrated) return;
    expect(result.reason).toMatch(/UTF-8/i);
    expect(invoke).not.toHaveBeenCalled();
  });

  it("surfaces mapped 409 conflicts from cloudBlobOpen", async () => {
    cloudBlobOpen.mockRejectedValue(
      new Error("cloud blob error: cloud API error (409): already bound"),
    );
    const result = await reopenResourceFromCloud("/ws", "Notes.md");
    expect(result).toEqual({ ok: false, message: CLOUD_BLOB_CONFLICT_MESSAGE });
  });
});
