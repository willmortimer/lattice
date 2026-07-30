import type { SemanticStatus } from "./semantic";
import { isVectorsBehindStatus } from "./semantic";
import type { SearchHit } from "../types";

export type SearchHitBadgeKind = "keyword" | "semantic" | "both";

/** One-line SearchPane copy when Lance vectors lag the workspace snapshot. */
export const SEARCH_VECTORS_BEHIND_BANNER =
  "Vectors behind workspace — results may prefer keywords.";

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

/** True when semantic search is enabled and runtime reports vectors behind workspace. */
export function shouldShowVectorsBehindBanner(
  semanticEnabled: boolean,
  status: SemanticStatus | null,
): boolean {
  return semanticEnabled && status != null && isVectorsBehindStatus(status);
}

/** Format an RRF fused score for compact display (typically 0.01–0.03). */
export function formatFusedScore(score: number): string {
  if (!Number.isFinite(score)) return "—";
  const abs = Math.abs(score);
  if (abs >= 1) return score.toFixed(2);
  if (abs >= 0.1) return score.toFixed(3);
  return score.toFixed(4).replace(/0+$/, "").replace(/\.$/, "");
}

/**
 * Compact fusion label beside the match badge: fused RRF score and/or per-list ranks.
 * Returns null for FTS-only hits with no hybrid metadata.
 */
export function formatSearchHitScore(
  hit: Pick<SearchHit, "fusedScore" | "lexicalRank" | "semanticRank">,
): string | null {
  const parts: string[] = [];
  if (hit.fusedScore != null) {
    parts.push(formatFusedScore(hit.fusedScore));
  }

  const rankParts: string[] = [];
  if (hit.lexicalRank != null) rankParts.push(`K${hit.lexicalRank}`);
  if (hit.semanticRank != null) rankParts.push(`S${hit.semanticRank}`);
  if (rankParts.length > 0) {
    parts.push(rankParts.join("·"));
  }

  return parts.length > 0 ? parts.join(" ") : null;
}
