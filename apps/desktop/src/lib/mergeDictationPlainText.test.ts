import { describe, expect, it } from "vitest";

import { mergeDictationPlainText } from "./mergeDictationPlainText";

describe("mergeDictationPlainText", () => {
  it("returns content unchanged for empty finals", () => {
    expect(mergeDictationPlainText("hello", "   ", 5)).toBe("hello");
  });

  it("appends transcript at the cursor", () => {
    expect(mergeDictationPlainText("alpha beta", "gamma", 6)).toBe("alpha gamma beta");
  });

  it("maps voice structure markers to plain newlines", () => {
    expect(mergeDictationPlainText("", "one new line two new paragraph three", 0)).toBe(
      "one\ntwo\n\nthree",
    );
  });

  it("inserts at end without an extra space", () => {
    expect(mergeDictationPlainText("typed ", "spoken", 6)).toBe("typed spoken");
  });
});
