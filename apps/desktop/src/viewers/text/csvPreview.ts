/** Bounded RFC4180-ish CSV/TSV preview parser for the text viewer. */

export const CSV_PREVIEW_MAX_BYTES = 256 * 1024;
export const CSV_PREVIEW_MAX_ROWS = 200;
export const CSV_PREVIEW_MAX_COLS = 40;

export type CsvDelimiter = "," | "\t" | ";";

export interface CsvPreviewLimits {
  maxRows: number;
  maxCols: number;
}

export const DEFAULT_CSV_PREVIEW_LIMITS: CsvPreviewLimits = {
  maxRows: CSV_PREVIEW_MAX_ROWS,
  maxCols: CSV_PREVIEW_MAX_COLS,
};

export type CsvPreviewResult =
  | {
      ok: true;
      delimiter: CsvDelimiter;
      headers: string[];
      rows: string[][];
      totalRows: number;
      totalCols: number;
      truncatedRows: boolean;
      truncatedCols: boolean;
      note: string | null;
    }
  | {
      ok: false;
      message: string;
    };

export function isCsvPreviewPath(path: string): boolean {
  const lower = path.toLowerCase();
  return lower.endsWith(".csv") || lower.endsWith(".tsv");
}

/** Caller should only pass full-file content when `!truncated` and size ≤ 256 KiB. */
export function canShowCsvPreview(input: {
  path: string;
  truncated: boolean;
  totalSize: number;
}): boolean {
  return isCsvPreviewPath(input.path) && !input.truncated && input.totalSize <= CSV_PREVIEW_MAX_BYTES;
}

export function detectDelimiter(path: string, content: string): CsvDelimiter {
  const lower = path.toLowerCase();
  if (lower.endsWith(".tsv")) return "\t";
  if (lower.endsWith(".csv")) return ",";

  const sample = content.slice(0, 4096);
  let commas = 0;
  let tabs = 0;
  let semis = 0;
  let inQuotes = false;
  for (let i = 0; i < sample.length; i += 1) {
    const ch = sample[i];
    if (ch === '"') {
      if (inQuotes && sample[i + 1] === '"') {
        i += 1;
        continue;
      }
      inQuotes = !inQuotes;
      continue;
    }
    if (inQuotes) continue;
    if (ch === "\n" || ch === "\r") break;
    if (ch === ",") commas += 1;
    else if (ch === "\t") tabs += 1;
    else if (ch === ";") semis += 1;
  }
  if (tabs > commas && tabs >= semis) return "\t";
  if (semis > commas && semis > tabs) return ";";
  return ",";
}

function parseRecords(source: string, delimiter: CsvDelimiter): string[][] | { error: string } {
  const records: string[][] = [];
  let row: string[] = [];
  let field = "";
  let inQuotes = false;
  let i = 0;

  const pushField = () => {
    row.push(field);
    field = "";
  };

  const pushRow = () => {
    // Ignore a trailing empty line after the final record (common in text files).
    if (row.length === 1 && row[0] === "" && records.length > 0) {
      row = [];
      return;
    }
    records.push(row);
    row = [];
  };

  while (i < source.length) {
    const ch = source[i];
    if (inQuotes) {
      if (ch === '"') {
        if (source[i + 1] === '"') {
          field += '"';
          i += 2;
          continue;
        }
        inQuotes = false;
        i += 1;
        continue;
      }
      field += ch;
      i += 1;
      continue;
    }

    if (ch === '"') {
      // Quotes may only open at the start of a field (RFC4180).
      if (field.length > 0) {
        return { error: `Unexpected quote at character ${i + 1}` };
      }
      inQuotes = true;
      i += 1;
      continue;
    }

    if (ch === delimiter) {
      pushField();
      i += 1;
      continue;
    }

    if (ch === "\r" || ch === "\n") {
      pushField();
      pushRow();
      if (ch === "\r" && source[i + 1] === "\n") i += 2;
      else i += 1;
      continue;
    }

    field += ch;
    i += 1;
  }

  if (inQuotes) {
    return { error: "Unterminated quoted field" };
  }

  // Flush final field/row when the file does not end with a newline.
  if (field.length > 0 || row.length > 0 || records.length === 0) {
    pushField();
    pushRow();
  }

  return records;
}

function truncationNote(truncatedRows: boolean, truncatedCols: boolean, totalRows: number, totalCols: number, limits: CsvPreviewLimits): string | null {
  if (!truncatedRows && !truncatedCols) return null;
  const parts: string[] = [];
  if (truncatedRows) parts.push(`showing first ${limits.maxRows} of ${totalRows} rows`);
  if (truncatedCols) parts.push(`showing first ${limits.maxCols} of ${totalCols} columns`);
  return `Preview truncated: ${parts.join("; ")}.`;
}

export function parseCsvPreview(
  content: string,
  options: {
    path?: string;
    delimiter?: CsvDelimiter;
    limits?: Partial<CsvPreviewLimits>;
  } = {},
): CsvPreviewResult {
  const limits: CsvPreviewLimits = { ...DEFAULT_CSV_PREVIEW_LIMITS, ...options.limits };
  const path = options.path ?? "";
  const delimiter = options.delimiter ?? detectDelimiter(path, content);

  if (content.length === 0) {
    return {
      ok: true,
      delimiter,
      headers: [],
      rows: [],
      totalRows: 0,
      totalCols: 0,
      truncatedRows: false,
      truncatedCols: false,
      note: null,
    };
  }

  const parsed = parseRecords(content, delimiter);
  if ("error" in parsed) {
    return { ok: false, message: parsed.error };
  }

  const records = parsed;
  const totalRows = records.length;
  let totalCols = 0;
  for (const record of records) {
    if (record.length > totalCols) totalCols = record.length;
  }

  const truncatedRows = totalRows > limits.maxRows;
  const truncatedCols = totalCols > limits.maxCols;
  const displayRecords = truncatedRows ? records.slice(0, limits.maxRows) : records;
  const clipped = displayRecords.map((record) =>
    truncatedCols ? record.slice(0, limits.maxCols) : record,
  );

  // Pad short rows so the table stays rectangular within the display window.
  const displayCols = Math.min(totalCols, limits.maxCols);
  for (const record of clipped) {
    while (record.length < displayCols) record.push("");
  }

  const headers = clipped[0] ?? [];
  const rows = clipped.slice(1);

  return {
    ok: true,
    delimiter,
    headers,
    rows,
    totalRows,
    totalCols,
    truncatedRows,
    truncatedCols,
    note: truncationNote(truncatedRows, truncatedCols, totalRows, totalCols, limits),
  };
}
