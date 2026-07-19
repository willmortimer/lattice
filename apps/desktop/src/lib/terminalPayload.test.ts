import { describe, expect, it } from "vitest";

import { decodeTerminalOutput } from "./terminalPayload";

describe("decodeTerminalOutput", () => {
  it("converts event byte arrays to Uint8Array", () => {
    const bytes = decodeTerminalOutput([72, 101, 108, 108, 111]);
    expect(bytes).toBeInstanceOf(Uint8Array);
    expect(Array.from(bytes)).toEqual([72, 101, 108, 108, 111]);
    expect(new TextDecoder().decode(bytes)).toBe("Hello");
  });

  it("returns an empty array for empty payloads", () => {
    const bytes = decodeTerminalOutput([]);
    expect(bytes.length).toBe(0);
  });
});
