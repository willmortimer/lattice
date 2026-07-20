//! Offline search-eval harness using `research/search-eval` fixtures + Fake embeddings.
//!
//! ```sh
//! cargo test -p lattice-index --test search_eval -- --nocapture
//! ```

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use lattice_core::Workspace;
use lattice_embedding::{DistanceMetric, FakeEmbeddingProvider, PoolingStrategy};
use lattice_index::{WorkspaceIndex, CHUNKER_VERSION};
use serde::Deserialize;
use tempfile::TempDir;

#[derive(Debug, Deserialize)]
struct QuerySet {
    queries: Vec<EvalQuery>,
}

#[derive(Debug, Deserialize)]
struct EvalQuery {
    id: String,
    text: String,
    #[serde(default)]
    relevant: Vec<String>,
}

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../research/search-eval")
}

fn load_queries(root: &Path) -> QuerySet {
    let path = root.join("queries.yaml");
    let raw = fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!("failed to read {}: {err}", path.display());
    });
    serde_yaml::from_str(&raw).expect("parse queries.yaml")
}

fn seed_corpus(workspace: &Path, corpus: &Path) {
    for entry in fs::read_dir(corpus).expect("read corpus dir") {
        let entry = entry.expect("corpus entry");
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        let name = path.file_name().expect("corpus filename");
        fs::copy(&path, workspace.join(name)).expect("copy corpus file");
    }
}

fn unique_resource_hits(hits: &[lattice_index::HybridSearchHit], limit: usize) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for hit in hits {
        let path = path_from_resource_uri(&hit.resource_uri);
        if seen.insert(path.clone()) {
            out.push(path);
            if out.len() >= limit {
                break;
            }
        }
    }
    out
}

fn path_from_resource_uri(uri: &str) -> String {
    uri.strip_prefix("lattice://resource/")
        .or_else(|| uri.strip_prefix("file://"))
        .unwrap_or(uri)
        .replace('\\', "/")
}

fn recall_at_k(ranked: &[String], relevant: &[String], k: usize) -> Option<f64> {
    if relevant.is_empty() {
        return None;
    }
    let top: HashSet<&str> = ranked.iter().take(k).map(String::as_str).collect();
    let hits = relevant
        .iter()
        .filter(|doc| top.contains(doc.as_str()))
        .count();
    Some(hits as f64 / relevant.len() as f64)
}

fn mrr(ranked: &[String], relevant: &[String]) -> Option<f64> {
    if relevant.is_empty() {
        return None;
    }
    let relevant: HashSet<&str> = relevant.iter().map(String::as_str).collect();
    for (idx, doc) in ranked.iter().enumerate() {
        if relevant.contains(doc.as_str()) {
            return Some(1.0 / (idx as f64 + 1.0));
        }
    }
    Some(0.0)
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

#[test]
fn research_search_eval_fts_vs_hybrid_fake() {
    let fixtures = fixture_root();
    assert!(
        fixtures.join("queries.yaml").is_file(),
        "missing fixtures at {}",
        fixtures.display()
    );

    let dir = TempDir::new().unwrap();
    Workspace::init(dir.path(), "SearchEval").unwrap();
    seed_corpus(dir.path(), &fixtures.join("corpus"));

    let index = WorkspaceIndex::open(dir.path()).unwrap();
    index.rebuild(dir.path()).unwrap();

    let spec = lattice_embedding::EmbeddingSpecification {
        provider_id: "fake".into(),
        model_id: "fake-eval".into(),
        model_revision: "rev-1".into(),
        artifact_sha256: "sha256:fake-eval".into(),
        dimensions: 16,
        native_dimensions: 16,
        distance: DistanceMetric::Cosine,
        pooling: PoolingStrategy::Last,
        normalized: true,
        instruction_version: "eval-v1".into(),
    };
    let namespace = index
        .register_embedding_namespace(&spec, CHUNKER_VERSION)
        .unwrap();
    let provider = FakeEmbeddingProvider::new(spec);
    let stats = index
        .embed_pending_chunks(namespace.id, &provider, 8)
        .unwrap();
    assert!(stats.embedded > 0, "expected Fake embeddings for corpus");

    let queries = load_queries(&fixtures);
    assert!(
        queries.queries.len() >= 40,
        "expected expanded labeled set (≥40 queries), got {}",
        queries.queries.len()
    );

    let mut fts_recalls = Vec::new();
    let mut fts_mrrs = Vec::new();
    let mut hybrid_recalls = Vec::new();
    let mut hybrid_mrrs = Vec::new();

    println!("search-eval (FakeEmbeddingProvider, offline)");
    for query in &queries.queries {
        let fts_hits = index
            .hybrid_search(&query.text, 10, None, None)
            .expect("fts-only hybrid_search");
        let fts_ranked = unique_resource_hits(&fts_hits, 10);

        let hybrid_hits = index
            .hybrid_search(&query.text, 10, Some(&provider), Some(namespace.id))
            .expect("hybrid_search");
        let hybrid_ranked = unique_resource_hits(&hybrid_hits, 10);

        if let Some(r) = recall_at_k(&fts_ranked, &query.relevant, 10) {
            fts_recalls.push(r);
        }
        if let Some(m) = mrr(&fts_ranked, &query.relevant) {
            fts_mrrs.push(m);
        }
        if let Some(r) = recall_at_k(&hybrid_ranked, &query.relevant, 10) {
            hybrid_recalls.push(r);
        }
        if let Some(m) = mrr(&hybrid_ranked, &query.relevant) {
            hybrid_mrrs.push(m);
        }

        println!(
            "  [{id}] fts={fts:?} hybrid={hybrid:?} relevant={relevant:?}",
            id = query.id,
            fts = fts_ranked,
            hybrid = hybrid_ranked,
            relevant = query.relevant,
        );
    }

    let fts_recall = mean(&fts_recalls);
    let fts_mrr = mean(&fts_mrrs);
    let hybrid_recall = mean(&hybrid_recalls);
    let hybrid_mrr = mean(&hybrid_mrrs);

    println!(
        "summary: FTS Recall@10={fts_recall:.3} MRR={fts_mrr:.3} | Hybrid Recall@10={hybrid_recall:.3} MRR={hybrid_mrr:.3}"
    );

    // Smoke + soft gate: labeled positives should retrieve via FTS on this corpus.
    assert!(
        fts_recall > 0.0 || hybrid_recall > 0.0,
        "expected non-zero Recall@10 from FTS or hybrid on fixture corpus (fts={fts_recall}, hybrid={hybrid_recall})"
    );
    assert!(
        fts_recall >= 0.5,
        "expected FTS Recall@10 ≥ 0.5 on expanded fixture queries (got {fts_recall})"
    );
}
