import { describe, expect, it } from "vitest";
import {
  displayDataToMime,
  mimeBundleToDisplayData,
  sanitizeNotebookHtml,
  sanitizeNotebookSvg,
} from "./notebookMime";
import { splitNbformatLines } from "./mergeNotebookOutputs";

describe("notebookMime", () => {
  it("parses rich MIME bundles into display data", () => {
    expect(
      mimeBundleToDisplayData({
        "text/plain": "42",
        "text/html": "<table><tr><td>a</td></tr></table>",
        "text/markdown": "# Title",
        "image/png": "aGVsbG8=",
        "image/svg+xml": "<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>",
        "application/vnd.vegalite.v5+json": {
          mark: "bar",
          data: { values: [{ a: 1 }] },
        },
      }),
    ).toEqual({
      textPlain: "42",
      html: "<table><tr><td>a</td></tr></table>",
      markdown: "# Title",
      imageDataUrl: "data:image/png;base64,aGVsbG8=",
      svg: "<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>",
      vegaLite: {
        mark: "bar",
        data: { values: [{ a: 1 }] },
      },
    });
  });

  it("round-trips display data back to nbformat MIME keys", () => {
    const data = {
      textPlain: "42",
      markdown: "# Title",
      html: "<p>hi</p>",
      svg: "<svg></svg>",
      imageDataUrl: "data:image/png;base64,abc",
      vegaLite: { mark: "point" },
    };
    expect(displayDataToMime(data, splitNbformatLines)).toEqual({
      "text/plain": ["42"],
      "text/markdown": ["# Title"],
      "text/html": ["<p>hi</p>"],
      "image/svg+xml": ["<svg></svg>"],
      "image/png": "abc",
      "application/vnd.vegalite.v5+json": { mark: "point" },
    });
  });

  it("strips unsafe HTML and SVG", () => {
    expect(sanitizeNotebookHtml('<p>ok</p><script>alert(1)</script>')).toBe("<p>ok</p>");
    expect(sanitizeNotebookSvg('<svg><script>alert(1)</script><circle /></svg>')).not.toContain(
      "script",
    );
  });
});
