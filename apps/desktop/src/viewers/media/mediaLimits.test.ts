import { describe, expect, it } from "vitest";
import { assertDecodedPixelLimit, assertEncodedLimit, MAX_IMAGE_DECODED_PIXELS, MAX_IMAGE_ENCODED_BYTES, MediaLimitError } from "./mediaLimits";

describe("media safety limits", () => {
  it("rejects images over the encoded limit before reading bytes", () => {
    expect(() => assertEncodedLimit(MAX_IMAGE_ENCODED_BYTES + 1, MAX_IMAGE_ENCODED_BYTES, "Image")).toThrow(MediaLimitError);
  });

  it("rejects images over the decoded pixel limit", () => {
    expect(() => assertDecodedPixelLimit(10_001, 10_000)).toThrow(/pixels/);
    expect(() => assertDecodedPixelLimit(MAX_IMAGE_DECODED_PIXELS, 1)).not.toThrow();
  });
});
