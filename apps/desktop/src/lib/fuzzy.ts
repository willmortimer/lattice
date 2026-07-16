/**
 * Minimal fuzzy matcher for the command palette and resource tree filter
 * (no dependency pulled in for something this small): `pattern`'s
 * characters must appear in `text`, in order, case-insensitively.
 * Consecutive matches and matches right after a word boundary score
 * higher than scattered ones, so "cp" ranks "Command Palette" above
 * "Corporate Plan".
 */
export interface FuzzyMatch {
  score: number;
  indices: number[];
}

const WORD_BOUNDARY_PATTERN = /[\s/._-]/;

/** Fuzzy-match `pattern` against `text`, or `null` if it doesn't match at all. */
export function fuzzyMatch(pattern: string, text: string): FuzzyMatch | null {
  if (pattern.length === 0) return { score: 0, indices: [] };

  const lowerPattern = pattern.toLowerCase();
  const lowerText = text.toLowerCase();
  const indices: number[] = [];

  let score = 0;
  let searchFrom = 0;
  let previousMatchIndex = -1;

  for (const char of lowerPattern) {
    const foundAt = lowerText.indexOf(char, searchFrom);
    if (foundAt === -1) return null;

    const isConsecutive = foundAt === previousMatchIndex + 1;
    const isWordStart = foundAt === 0 || WORD_BOUNDARY_PATTERN.test(lowerText[foundAt - 1]);
    score += isConsecutive ? 3 : isWordStart ? 2 : 1;
    score -= foundAt - searchFrom; // penalize characters skipped to find this one

    indices.push(foundAt);
    previousMatchIndex = foundAt;
    searchFrom = foundAt + 1;
  }

  return { score, indices };
}

export interface FuzzyResult<T> {
  item: T;
  score: number;
  indices: number[];
}

/**
 * Filter and rank `items` by fuzzy-matching `pattern` against
 * `toText(item)`, best score first. An empty (or whitespace-only) pattern
 * matches everything, preserving `items`' original order.
 */
export function fuzzyFilter<T>(
  items: readonly T[],
  pattern: string,
  toText: (item: T) => string,
): FuzzyResult<T>[] {
  if (pattern.trim().length === 0) {
    return items.map((item) => ({ item, score: 0, indices: [] }));
  }

  const results: FuzzyResult<T>[] = [];
  for (const item of items) {
    const match = fuzzyMatch(pattern, toText(item));
    if (match) results.push({ item, score: match.score, indices: match.indices });
  }
  results.sort((a, b) => b.score - a.score);
  return results;
}
