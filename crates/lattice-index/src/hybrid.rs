//! Reciprocal-rank fusion and light diversification for hybrid retrieval.

use std::collections::{HashMap, HashSet};

use crate::provenance::{ExportPolicy, SearchProvenance, Sensitivity};
use crate::semantic::{ChunkHydrationRow, SemanticHit};
use crate::types::{ChunkSearchHit, HybridSearchHit};

/// Durable resource URI for a workspace-relative path (forward-slash form).
pub fn resource_uri_from_path(path: &str) -> String {
    format!("lattice://resource/{}", path.replace('\\', "/"))
}

/// Reciprocal-rank fusion constant (Cormack et al.).
pub const RRF_K: u32 = 60;

/// Soft cap on chunks returned from a single resource after fusion.
pub const DEFAULT_MAX_CHUNKS_PER_RESOURCE: usize = 3;

/// Per-chunk RRF accumulation before hydration.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FusionAccum {
    pub lexical_rank: Option<u32>,
    pub semantic_rank: Option<u32>,
    pub fused_score: f32,
}

/// Fuse ranked lexical and semantic lists with RRF (k=60).
pub fn reciprocal_rank_fuse(
    lexical: &[(String, u32)],
    semantic: &[(String, u32)],
) -> Vec<(String, FusionAccum)> {
    let mut scores: HashMap<String, FusionAccum> = HashMap::new();
    for (chunk_id, rank) in lexical {
        let entry = scores.entry(chunk_id.clone()).or_default();
        entry.lexical_rank = Some(*rank);
        entry.fused_score += 1.0 / (RRF_K as f32 + *rank as f32);
    }
    for (chunk_id, rank) in semantic {
        let entry = scores.entry(chunk_id.clone()).or_default();
        entry.semantic_rank = Some(*rank);
        entry.fused_score += 1.0 / (RRF_K as f32 + *rank as f32);
    }
    let mut fused: Vec<(String, FusionAccum)> = scores.into_iter().collect();
    fused.sort_by(|a, b| {
        b.1.fused_score
            .partial_cmp(&a.1.fused_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });
    fused
}

/// Keep at most `max_per_resource` chunks per resource path while preserving order.
pub fn diversify_by_resource(
    hits: Vec<HybridSearchHit>,
    max_per_resource: usize,
) -> Vec<HybridSearchHit> {
    if max_per_resource == 0 {
        return Vec::new();
    }
    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut out = Vec::with_capacity(hits.len());
    for hit in hits {
        let count = counts.entry(hit.resource_uri.clone()).or_insert(0);
        if *count >= max_per_resource {
            continue;
        }
        *count += 1;
        out.push(hit);
    }
    out
}

pub(crate) fn lexical_rank_list(hits: &[ChunkSearchHit]) -> Vec<(String, u32)> {
    hits.iter()
        .enumerate()
        .map(|(index, hit)| (hit.chunk_id.clone(), (index + 1) as u32))
        .collect()
}

pub(crate) fn semantic_rank_list(hits: &[SemanticHit]) -> Vec<(String, u32)> {
    hits.iter()
        .map(|hit| (hit.chunk_id.clone(), hit.rank))
        .collect()
}

pub(crate) fn hydrate_fused_hits(
    fused: Vec<(String, FusionAccum)>,
    rows: HashMap<String, ChunkHydrationRow>,
    provenance_base: ProvenanceBase,
    limit: usize,
) -> Vec<HybridSearchHit> {
    let mut hits = Vec::new();
    for (chunk_id, accum) in fused {
        let Some(row) = rows.get(&chunk_id) else {
            continue;
        };
        let sensitivity = Sensitivity::parse(&row.sensitivity);
        // Highest sensitivity tier is never returned in search results.
        if sensitivity == Sensitivity::Secret {
            continue;
        }
        let excerpt = excerpt_from_text(&row.text, 240);
        hits.push(HybridSearchHit {
            resource_uri: resource_uri_from_path(&row.path),
            resource_id: row.resource_id.to_string(),
            chunk_id,
            title: row.title.clone(),
            heading_path: row.heading_path.clone(),
            excerpt,
            source_start_byte: row.source_start_byte,
            source_end_byte: row.source_end_byte,
            lexical_rank: accum.lexical_rank,
            semantic_rank: accum.semantic_rank,
            fused_score: accum.fused_score,
            provenance: SearchProvenance {
                content_hash: row.content_hash.clone(),
                chunker_version: row.chunker_version.clone(),
                namespace_key: provenance_base.namespace_key.clone(),
                model_id: provenance_base.model_id.clone(),
                model_revision: provenance_base.model_revision.clone(),
                instruction_version: provenance_base.instruction_version.clone(),
            },
            sensitivity,
            export_policy: ExportPolicy::parse(&row.export_policy),
        });
        if hits.len() >= limit * 4 {
            // Collect a buffer before diversification truncates.
            break;
        }
    }
    hits
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ProvenanceBase {
    pub namespace_key: Option<String>,
    pub model_id: Option<String>,
    pub model_revision: Option<String>,
    pub instruction_version: Option<String>,
}

pub(crate) fn fts_only_hits(
    lexical: &[ChunkSearchHit],
    rows: HashMap<String, ChunkHydrationRow>,
    limit: usize,
) -> Vec<HybridSearchHit> {
    let fused = reciprocal_rank_fuse(&lexical_rank_list(lexical), &[]);
    let hits = hydrate_fused_hits(fused, rows, ProvenanceBase::default(), limit);
    diversify_by_resource(hits, DEFAULT_MAX_CHUNKS_PER_RESOURCE)
        .into_iter()
        .take(limit)
        .collect()
}

fn excerpt_from_text(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    let mut out = trimmed.chars().take(max_chars).collect::<String>();
    out.push('…');
    out
}

/// Collect unique chunk ids from fusion order for hydration.
pub(crate) fn fused_chunk_ids(fused: &[(String, FusionAccum)]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut ids = Vec::new();
    for (chunk_id, _) in fused {
        if seen.insert(chunk_id.clone()) {
            ids.push(chunk_id.clone());
        }
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rrf_prefers_items_present_in_both_lists() {
        let lexical = vec![("a".into(), 1), ("b".into(), 2), ("c".into(), 3)];
        let semantic = vec![("c".into(), 1), ("a".into(), 2), ("d".into(), 3)];
        let fused = reciprocal_rank_fuse(&lexical, &semantic);
        assert_eq!(fused[0].0, "a");
        assert!(fused[0].1.fused_score > fused[1].1.fused_score);
        assert!(fused.iter().any(|(id, accum)| {
            id == "c" && accum.lexical_rank == Some(3) && accum.semantic_rank == Some(1)
        }));
    }

    #[test]
    fn diversify_caps_per_resource() {
        let mk = |uri: &str, chunk: &str, score: f32| HybridSearchHit {
            resource_uri: uri.into(),
            resource_id: "1".into(),
            chunk_id: chunk.into(),
            title: "t".into(),
            heading_path: vec![],
            excerpt: "e".into(),
            source_start_byte: 0,
            source_end_byte: 1,
            lexical_rank: Some(1),
            semantic_rank: None,
            fused_score: score,
            provenance: SearchProvenance {
                content_hash: "sha256:x".into(),
                chunker_version: "v1".into(),
                namespace_key: None,
                model_id: None,
                model_revision: None,
                instruction_version: None,
            },
            sensitivity: Sensitivity::Workspace,
            export_policy: ExportPolicy::Ask,
        };
        let hits = vec![
            mk("a.md", "a1", 3.0),
            mk("a.md", "a2", 2.9),
            mk("a.md", "a3", 2.8),
            mk("a.md", "a4", 2.7),
            mk("b.md", "b1", 2.6),
        ];
        let diversified = diversify_by_resource(hits, 2);
        assert_eq!(diversified.len(), 3);
        assert_eq!(
            diversified
                .iter()
                .filter(|hit| hit.resource_uri == "a.md")
                .count(),
            2
        );
        assert!(diversified.iter().any(|hit| hit.chunk_id == "b1"));
    }
}
