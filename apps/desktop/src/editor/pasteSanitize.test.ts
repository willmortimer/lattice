import { describe, expect, it } from "vitest";

import { classifyClipboard, sanitizePastedHtml } from "./pasteSanitize";

function fakeClipboard(parts: Record<string, string>, files = 0): DataTransfer {
  return {
    files: { length: files } as FileList,
    getData: (type: string) => parts[type] ?? "",
  } as DataTransfer;
}

describe("sanitizePastedHtml", () => {
  it("strips scripts, handlers, styles, and remote media", () => {
    const dirty =
      '<p onclick="alert(1)" style="color:red">Hello <script>evil()</script>' +
      '<img src="https://evil.example/x.png" onerror="alert(2)">' +
      '<a href="https://example.com">ok</a></p>';
    const clean = sanitizePastedHtml(dirty);
    expect(clean).toContain("<p>");
    expect(clean).toContain("Hello");
    expect(clean).toContain('<a href="https://example.com">ok</a>');
    expect(clean).not.toContain("script");
    expect(clean).not.toContain("onclick");
    expect(clean).not.toContain("style=");
    expect(clean).not.toContain("<img");
    expect(clean).not.toContain("onerror");
  });
});

describe("classifyClipboard", () => {
  it("prefers files, then markdown, then html, then plain", () => {
    expect(classifyClipboard(fakeClipboard({}, 2))).toBe("files");
    expect(classifyClipboard(fakeClipboard({ "text/markdown": "# Hi", "text/html": "<p>x</p>", "text/plain": "x" }))).toBe(
      "markdown",
    );
    expect(classifyClipboard(fakeClipboard({ "text/html": "<p>x</p>", "text/plain": "x" }))).toBe("html");
    expect(classifyClipboard(fakeClipboard({ "text/plain": "hello" }))).toBe("plain");
    expect(classifyClipboard(fakeClipboard({}))).toBe("none");
  });
});
