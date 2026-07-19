import { describe, expect, it } from "vitest";
import {
  canShowCsvPreview,
  detectDelimiter,
  parseCsvPreview,
  CSV_PREVIEW_MAX_BYTES,
  CSV_PREVIEW_MAX_COLS,
  CSV_PREVIEW_MAX_ROWS,
} from "./csvPreview";

describe("csvPreview", () => {
  it("detects delimiter from extension", () => {
    expect(detectDelimiter("Data/sample.csv", "a\tb")).toBe(",");
    expect(detectDelimiter("Data/sample.tsv", "a,b")).toBe("\t");
  });

  it("sniffs delimiter from content when extension is ambiguous", () => {
    expect(detectDelimiter("data.txt", "a\tb\tc")).toBe("\t");
    expect(detectDelimiter("data.txt", "a;b;c")).toBe(";");
    expect(detectDelimiter("data.txt", "a,b,c")).toBe(",");
  });

  it("parses quoted commas", () => {
    const result = parseCsvPreview('name,note\n"Ada, Lovelace",Analyst\n', { path: "x.csv" });
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.headers).toEqual(["name", "note"]);
    expect(result.rows).toEqual([["Ada, Lovelace", "Analyst"]]);
  });

  it("parses newlines inside quoted fields", () => {
    const result = parseCsvPreview('name,bio\n"Ada","Line1\nLine2"\n', { path: "x.csv" });
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.rows).toEqual([["Ada", "Line1\nLine2"]]);
  });

  it("parses escaped quotes", () => {
    const result = parseCsvPreview('name\n"She said ""hello"""\n', { path: "x.csv" });
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.rows).toEqual([['She said "hello"']]);
  });

  it("parses TSV with tabs", () => {
    const result = parseCsvPreview("a\tb\n1\t2\n", { path: "x.tsv" });
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.delimiter).toBe("\t");
    expect(result.headers).toEqual(["a", "b"]);
    expect(result.rows).toEqual([["1", "2"]]);
  });

  it("truncates rows and columns with a note", () => {
    const header = Array.from({ length: CSV_PREVIEW_MAX_COLS + 5 }, (_, i) => `c${i}`).join(",");
    const body = Array.from({ length: CSV_PREVIEW_MAX_ROWS + 10 }, (_, row) =>
      Array.from({ length: CSV_PREVIEW_MAX_COLS + 5 }, (_, col) => `${row}:${col}`).join(","),
    ).join("\n");
    const result = parseCsvPreview(`${header}\n${body}`, { path: "big.csv" });
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.headers).toHaveLength(CSV_PREVIEW_MAX_COLS);
    expect(result.rows).toHaveLength(CSV_PREVIEW_MAX_ROWS - 1);
    expect(result.truncatedRows).toBe(true);
    expect(result.truncatedCols).toBe(true);
    expect(result.note).toMatch(/Preview truncated/);
    expect(result.note).toMatch(/rows/);
    expect(result.note).toMatch(/columns/);
  });

  it("returns a diagnostic for unterminated quotes", () => {
    const result = parseCsvPreview('a,b\n"open', { path: "x.csv" });
    expect(result.ok).toBe(false);
    if (result.ok) return;
    expect(result.message).toMatch(/Unterminated/);
  });

  it("gates preview on full load and size", () => {
    expect(canShowCsvPreview({ path: "Data/sample.csv", truncated: false, totalSize: 100 })).toBe(true);
    expect(canShowCsvPreview({ path: "Data/sample.tsv", truncated: false, totalSize: CSV_PREVIEW_MAX_BYTES })).toBe(true);
    expect(canShowCsvPreview({ path: "Data/sample.csv", truncated: true, totalSize: 100 })).toBe(false);
    expect(canShowCsvPreview({ path: "Data/sample.csv", truncated: false, totalSize: CSV_PREVIEW_MAX_BYTES + 1 })).toBe(false);
    expect(canShowCsvPreview({ path: "Data/sample.json", truncated: false, totalSize: 100 })).toBe(false);
  });
});
