//! llama-cpp query-embed latency harness (`bench` subcommand).

use std::path::Path;
use std::time::Instant;

use lattice_embedding::{qwen3_embedding_0_6b_q8_manifest, EmbedQueryRequest};
use serde::Serialize;

use crate::backend::{open_backend, BackendKind};

/// Environment variable for the pinned Qwen3 GGUF path.
pub const ENV_GGUF: &str = "LATTICE_EMBED_LLAMA_GGUF";

/// Printed / serialized bench output.
#[derive(Debug, Clone, Serialize)]
pub struct BenchStats {
    pub backend: &'static str,
    pub dimensions: u32,
    pub warmup: u32,
    pub iterations: u32,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub mean_ms: f64,
    pub min_ms: f64,
    pub max_ms: f64,
}

/// Run warm-up + timed query embeds against a local GGUF.
pub async fn run_llama_bench(
    gguf_path: &Path,
    dimensions: u32,
    warmup: u32,
    iterations: u32,
    query: &str,
) -> anyhow::Result<BenchStats> {
    if !gguf_path.is_file() {
        anyhow::bail!(
            "GGUF not found at {} (set {ENV_GGUF} or pass --gguf)",
            gguf_path.display()
        );
    }
    if warmup == 0 && iterations == 0 {
        anyhow::bail!("warmup and iterations cannot both be zero");
    }

    let manifest = qwen3_embedding_0_6b_q8_manifest();
    let backend = open_backend(BackendKind::LlamaCpp, &manifest, gguf_path, dimensions)?;
    let request = EmbedQueryRequest {
        text: query.to_string(),
    };

    for _ in 0..warmup {
        backend
            .embed_query(request.clone())
            .await
            .map_err(|error| anyhow::anyhow!("warmup embed_query failed: {error}"))?;
    }

    let mut samples_ms = Vec::with_capacity(iterations as usize);
    for _ in 0..iterations {
        let start = Instant::now();
        backend
            .embed_query(request.clone())
            .await
            .map_err(|error| anyhow::anyhow!("embed_query failed: {error}"))?;
        samples_ms.push(start.elapsed().as_secs_f64() * 1000.0);
    }

    let mut sorted = samples_ms.clone();
    sorted.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));

    Ok(BenchStats {
        backend: BackendKind::LlamaCpp.as_str(),
        dimensions,
        warmup,
        iterations,
        p50_ms: percentile_nearest(&sorted, 50.0),
        p95_ms: percentile_nearest(&sorted, 95.0),
        mean_ms: mean(&samples_ms),
        min_ms: sorted.first().copied().unwrap_or(0.0),
        max_ms: sorted.last().copied().unwrap_or(0.0),
    })
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
        let weight = rank - lo as f64;
        sorted[lo] * (1.0 - weight) + sorted[hi] * weight
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentile_interpolates() {
        let sorted = vec![10.0, 20.0, 30.0, 40.0];
        assert!((percentile_nearest(&sorted, 50.0) - 25.0).abs() < 1e-9);
        assert!((percentile_nearest(&sorted, 95.0) - 38.5).abs() < 1e-9);
    }
}
