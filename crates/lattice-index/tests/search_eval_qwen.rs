//! Gated real-Qwen search-eval: warm embed latency + Recall@10/MRR via hybrid search.
//!
//! Default `cargo test` skips this harness (no GGUF, no network). Run locally after
//! building embed-host with llama.cpp:
//!
//! ```sh
//! cargo build -p lattice-embed-host --features llama-cpp
//! export LATTICE_EMBED_LLAMA_GGUF=/path/to/Qwen3-Embedding-0.6B-Q8_0.gguf
//! cargo test -p lattice-index --test search_eval_qwen -- --ignored --nocapture
//! ```
//!
//! Soft latency gate: set `LATTICE_SEARCH_EVAL_MAX_WARM_MS` (e.g. `2000`) to fail when
//! warm `embed_query` p50 exceeds that budget; unset prints metrics only.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use lattice_core::Workspace;
use lattice_embed_host::{
    install_model, socket_path_in, EmbedHostClient, ReconnectableEmbedHostProvider,
};
use lattice_embedding::{
    qwen3_embedding_0_6b_q8_manifest, EmbedQueryRequest, EmbeddingProvider,
};
use lattice_index::{WorkspaceIndex, CHUNKER_VERSION};
use serde::Deserialize;
use tempfile::TempDir;
use tokio::time::sleep;

const ENV_GGUF: &str = "LATTICE_EMBED_LLAMA_GGUF";
const ENV_HOST_BIN: &str = "LATTICE_EMBED_HOST_BIN";
const ENV_MAX_WARM_MS: &str = "LATTICE_SEARCH_EVAL_MAX_WARM_MS";
const WARM_EMBED_ITERS: usize = 5;
const QUERY_DIMS: u32 = 512;

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

struct HostChild(Child);

impl Drop for HostChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
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
        if seen.insert(hit.resource_uri.clone()) {
            out.push(hit.resource_uri.clone());
            if out.len() >= limit {
                break;
            }
        }
    }
    out
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

fn percentile_nearest(sorted: &[f64], pct: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    if sorted.len() == 1 {
        return sorted[0];
    }
    let rank = (pct / 100.0) * (sorted.len() as f64 - 1.0);
    let lo = rank.floor() as usize;
    let hi = rank.ceil() as usize;
    if lo == hi {
        sorted[lo]
    } else {
        let w = rank - lo as f64;
        sorted[lo] * (1.0 - w) + sorted[hi] * w
    }
}

fn require_gguf_path() -> PathBuf {
    let Ok(raw) = std::env::var(ENV_GGUF) else {
        panic!(
            "{ENV_GGUF} is unset; export it to a verified Qwen3-Embedding-0.6B-Q8_0.gguf path \
             before running --ignored (see research/search-eval/README.md)"
        );
    };
    let path = PathBuf::from(raw);
    assert!(
        path.is_file(),
        "{ENV_GGUF} does not point at a file: {}",
        path.display()
    );
    path
}

fn resolve_embed_host_bin() -> PathBuf {
    if let Ok(path) = std::env::var(ENV_HOST_BIN) {
        let path = PathBuf::from(path);
        assert!(
            path.is_file(),
            "{ENV_HOST_BIN} does not point at a file: {}",
            path.display()
        );
        return path;
    }

    let mut candidates = Vec::new();
    if let Ok(dir) = std::env::var("CARGO_TARGET_DIR") {
        candidates.push(PathBuf::from(&dir).join("debug/lattice-embed-host"));
        candidates.push(PathBuf::from(&dir).join("release/lattice-embed-host"));
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    candidates.push(manifest.join("../../target/debug/lattice-embed-host"));
    candidates.push(manifest.join("../../target/release/lattice-embed-host"));
    candidates.push(PathBuf::from("target/debug/lattice-embed-host"));
    candidates.push(PathBuf::from("target/release/lattice-embed-host"));

    for candidate in &candidates {
        if candidate.is_file() {
            return candidate.clone();
        }
    }

    panic!(
        "lattice-embed-host binary not found; build with \
         `cargo build -p lattice-embed-host --features llama-cpp` \
         or set {ENV_HOST_BIN}"
    );
}

fn assert_llama_cpp_backend(bin: &Path) {
    let output = Command::new(bin)
        .arg("backends")
        .output()
        .unwrap_or_else(|err| panic!("failed to run `{} backends`: {err}", bin.display()));
    assert!(
        output.status.success(),
        "`{} backends` failed with {}",
        bin.display(),
        output.status
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.lines().any(|line| line.trim() == "llama-cpp"),
        "lattice-embed-host at {} lacks llama-cpp (got backends:\n{stdout})\n\
         Rebuild: cargo build -p lattice-embed-host --features llama-cpp",
        bin.display()
    );
}

async fn wait_for_socket(path: &Path) {
    for _ in 0..200 {
        if path.exists() && EmbedHostClient::connect(path).await.is_ok() {
            return;
        }
        sleep(Duration::from_millis(50)).await;
    }
    panic!("embed-host socket not ready: {}", path.display());
}

fn spawn_llama_host(bin: &Path, socket: &Path, models_dir: &Path) -> HostChild {
    let child = Command::new(bin)
        .arg("serve")
        .arg("--socket")
        .arg(socket)
        .arg("--backend")
        .arg("llama-cpp")
        .arg("--models-dir")
        .arg(models_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap_or_else(|err| panic!("spawn lattice-embed-host: {err}"));
    HostChild(child)
}

#[test]
#[ignore = "requires LATTICE_EMBED_LLAMA_GGUF + lattice-embed-host built with --features llama-cpp"]
fn research_search_eval_fts_vs_hybrid_qwen() {
    let gguf_path = require_gguf_path();
    let host_bin = resolve_embed_host_bin();
    assert_llama_cpp_backend(&host_bin);

    let fixtures = fixture_root();
    assert!(
        fixtures.join("queries.yaml").is_file(),
        "missing fixtures at {}",
        fixtures.display()
    );

    let workspace_dir = TempDir::new().unwrap();
    let host_dir = TempDir::new().unwrap();
    Workspace::init(workspace_dir.path(), "SearchEvalQwen").unwrap();
    seed_corpus(workspace_dir.path(), &fixtures.join("corpus"));

    let socket = socket_path_in(host_dir.path());
    let models_dir = host_dir.path().join("models");
    let staging = host_dir.path().join("staging");
    fs::create_dir_all(&staging).unwrap();

    let manifest = qwen3_embedding_0_6b_q8_manifest();
    let manifest_path = staging.join("manifest.json");
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).expect("serialize qwen manifest"),
    )
    .unwrap();
    let installed = install_model(&manifest_path, &gguf_path, &models_dir)
        .expect("install Qwen GGUF into models_dir (sha256 must match pinned manifest)");

    let _host = spawn_llama_host(&host_bin, &socket, &models_dir);

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    let _enter = rt.enter();

    let provider = rt.block_on(async {
        wait_for_socket(&socket).await;
        Arc::new(
            ReconnectableEmbedHostProvider::connect(
                &socket,
                &installed.model_dir,
                Some(QUERY_DIMS),
            )
            .await
            .expect("connect ReconnectableEmbedHostProvider"),
        )
    });

    let index = WorkspaceIndex::open(workspace_dir.path()).unwrap();
    index.rebuild(workspace_dir.path()).unwrap();

    let namespace = index
        .register_embedding_namespace(provider.specification(), CHUNKER_VERSION)
        .unwrap();
    let stats = index
        .embed_pending_chunks(namespace.id, provider.as_ref(), 4)
        .expect("embed_pending_chunks via Qwen");
    assert!(
        stats.embedded > 0,
        "expected Qwen embeddings for corpus (embedded={})",
        stats.embedded
    );

    // One cold embed_query, then N warm samples for latency.
    let probe = "capability grants for plugins";
    let cold_ms = {
        let start = Instant::now();
        rt.block_on(async {
            provider
                .embed_query(EmbedQueryRequest {
                    text: probe.into(),
                })
                .await
                .expect("cold embed_query")
        });
        start.elapsed().as_secs_f64() * 1000.0
    };

    let mut warm_ms = Vec::with_capacity(WARM_EMBED_ITERS);
    for _ in 0..WARM_EMBED_ITERS {
        let start = Instant::now();
        rt.block_on(async {
            provider
                .embed_query(EmbedQueryRequest {
                    text: probe.into(),
                })
                .await
                .expect("warm embed_query")
        });
        warm_ms.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    let mut warm_sorted = warm_ms.clone();
    warm_sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let warm_p50 = percentile_nearest(&warm_sorted, 50.0);
    let warm_mean = mean(&warm_ms);

    let queries = load_queries(&fixtures);
    assert!(!queries.queries.is_empty());

    let mut fts_recalls = Vec::new();
    let mut fts_mrrs = Vec::new();
    let mut hybrid_recalls = Vec::new();
    let mut hybrid_mrrs = Vec::new();
    let mut hybrid_latencies_ms = Vec::new();

    println!("search-eval (Qwen3 via lattice-embed-host llama-cpp)");
    println!(
        "  embed_query cold={cold_ms:.1}ms warm_p50={warm_p50:.1}ms warm_mean={warm_mean:.1}ms (n={WARM_EMBED_ITERS})"
    );

    for query in &queries.queries {
        let fts_hits = index
            .hybrid_search(&query.text, 10, None, None)
            .expect("fts-only hybrid_search");
        let fts_ranked = unique_resource_hits(&fts_hits, 10);

        let hybrid_start = Instant::now();
        let hybrid_hits = index
            .hybrid_search(
                &query.text,
                10,
                Some(provider.as_ref()),
                Some(namespace.id),
            )
            .expect("hybrid_search");
        let hybrid_ms = hybrid_start.elapsed().as_secs_f64() * 1000.0;
        hybrid_latencies_ms.push(hybrid_ms);
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
            "  [{id}] hybrid_ms={hybrid_ms:.1} fts={fts:?} hybrid={hybrid:?} relevant={relevant:?}",
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
    let hybrid_mean_ms = mean(&hybrid_latencies_ms);

    println!(
        "summary: FTS Recall@10={fts_recall:.3} MRR={fts_mrr:.3} | Hybrid Recall@10={hybrid_recall:.3} MRR={hybrid_mrr:.3}"
    );
    println!("summary: hybrid_search mean latency={hybrid_mean_ms:.1}ms");

    if let Ok(raw) = std::env::var(ENV_MAX_WARM_MS) {
        let max_ms: f64 = raw.parse().unwrap_or_else(|_| {
            panic!("{ENV_MAX_WARM_MS} must be a number (milliseconds), got {raw:?}")
        });
        assert!(
            warm_p50 <= max_ms,
            "warm embed_query p50 {warm_p50:.1}ms exceeds {ENV_MAX_WARM_MS}={max_ms}ms"
        );
    } else {
        println!(
            "note: {ENV_MAX_WARM_MS} unset — warm latency printed only (suggested gate: 2000)"
        );
    }

    assert!(
        fts_recall > 0.0 || hybrid_recall > 0.0,
        "expected non-zero Recall@10 from FTS or hybrid on fixture corpus (fts={fts_recall}, hybrid={hybrid_recall})"
    );
    assert!(
        fts_recall > 0.0,
        "expected non-zero FTS Recall@10 on exact/phrase fixture queries (got {fts_recall})"
    );
}
