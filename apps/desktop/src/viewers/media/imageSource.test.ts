import { describe, expect, it, vi } from "vitest";
import type { ResourceInspection } from "../../lib/resourceRuntime";
import { loadImageAsset } from "./imageSource";
import { IMAGE_READ_CHUNK_SIZE } from "./imageSource";

const inspection = (size: number) => ({ size } as ResourceInspection);

describe("bounded image source", () => {
  it("reads encoded bytes in bounded ranges and revokes its object URL", async () => {
    const bytes = new Uint8Array(IMAGE_READ_CHUNK_SIZE + 10);
    bytes.set([0x89, 0x50, 0x4e, 0x47], 0);
    const reads: Array<{ offset: number; length: number }> = [];
    const revoke = vi.fn();
    const asset = await loadImageAsset(
      { root: "/workspace", path: "image.png" },
      new AbortController().signal,
      {
        inspect: vi.fn(async () => inspection(bytes.length)),
        readRange: vi.fn(async ({ offset, length }) => {
          reads.push({ offset, length });
          return bytes.slice(offset, offset + length);
        }),
        objectUrls: { createObjectURL: () => "blob:test-image", revokeObjectURL: revoke },
      },
    );
    expect(reads).toEqual([
      { offset: 0, length: IMAGE_READ_CHUNK_SIZE },
      { offset: IMAGE_READ_CHUNK_SIZE, length: 10 },
    ]);
    expect(asset.lease.url).toBe("blob:test-image");
    asset.lease.revoke();
    asset.lease.revoke();
    expect(revoke).toHaveBeenCalledTimes(1);
  });

  it("does not issue a read for an oversized image", async () => {
    const read = vi.fn();
    await expect(loadImageAsset(
      { root: "/workspace", path: "huge.png" },
      new AbortController().signal,
      { inspect: vi.fn(async () => inspection(64 * 1024 * 1024 + 1)), readRange: read },
    )).rejects.toThrow(/too large/);
    expect(read).not.toHaveBeenCalled();
  });
});
