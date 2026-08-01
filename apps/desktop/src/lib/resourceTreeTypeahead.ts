import type { FlatRow } from "./resourceTree";

export const TREE_TYPEAHEAD_RESET_MS = 1000;

export function isTreeTypeaheadKey(key: string): boolean {
  return key.length === 1 && /[\p{L}\p{N}]/u.test(key);
}

export function appendTreeTypeaheadPrefix(prefix: string, key: string): string {
  return prefix + key;
}

export function rowMatchesTypeaheadPrefix(rowName: string, prefix: string): boolean {
  if (!prefix) return false;
  return rowName.toLocaleLowerCase().startsWith(prefix.toLocaleLowerCase());
}

function isTypeaheadTargetRow(row: FlatRow): boolean {
  return row.type === "file" || row.type === "folder";
}

/**
 * Find the next visible row whose name matches `prefix`, searching forward from
 * `startIndex` and wrapping to the top — standard tree type-ahead behavior.
 */
export function findNextTypeaheadRowIndex(
  rows: readonly FlatRow[],
  prefix: string,
  startIndex: number,
): number | null {
  if (!prefix || rows.length === 0) return null;

  const from = startIndex + 1;
  for (let offset = 0; offset < rows.length; offset += 1) {
    const index = (from + offset) % rows.length;
    const row = rows[index];
    if (!isTypeaheadTargetRow(row)) continue;
    if (rowMatchesTypeaheadPrefix(row.name, prefix)) return index;
  }
  return null;
}
