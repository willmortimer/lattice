import type { SearchHit } from "../types";

export type SearchHitBadgeKind = "keyword" | "semantic" | "both";

/** Derive a match-kind hint from hybrid rank fields; FTS-only hits return null. */
export function searchHitBadgeKind(
  hit: Pick<SearchHit, "lexicalRank" | "semanticRank">,
): SearchHitBadgeKind | null {
  const hasLexical = hit.lexicalRank != null;
  const hasSemantic = hit.semanticRank != null;

  if (hasLexical && hasSemantic) return "both";
  if (hasLexical) return "keyword";
  if (hasSemantic) return "semantic";
  return null;
}

export function searchHitBadgeLabel(kind: SearchHitBadgeKind): string {
  switch (kind) {
    case "keyword":
      return "Keyword";
    case "semantic":
      return "Semantic";
    case "both":
      return "Both";
    default: {
      const _exhaustive: never = kind;
      return _exhaustive;
    }
  }
}

/** True when semantic search is on but no hit in the result set has a semantic rank yet. */
export function looksFtsOnlyWhileSemanticEnabled(
  semanticEnabled: boolean,
  hits: SearchHit[],
): boolean {
  return semanticEnabled && hits.length > 0 && hits.every((hit) => hit.semanticRank == null);
}
