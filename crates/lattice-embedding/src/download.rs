//! Explicit model acquire: local fixture copy or HTTPS download + sha256 verify.
//!
//! Never call these helpers from a search request path.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::EmbeddingError;
use crate::manifest::{verify_file_sha256, ModelManifest};
use crate::paths::{embeddings_download_dir, embeddings_models_dir, qwen3_embedding_install_dir};
use crate::pinned::{
    qwen3_embedding_0_6b_q8_manifest, QWEN3_EMBEDDING_ARTIFACT, QWEN3_EMBEDDING_DOWNLOAD_URL,
    QWEN3_EMBEDDING_INSTALL_SLUG, QWEN3_EMBEDDING_SHA256, QWEN3_EMBEDDING_SIZE_BYTES,
};

/// When truthy, skip model download/install and keep using Fake (CI / offline).
pub const ENV_SEMANTIC_FAKE: &str = "LATTICE_SEMANTIC_FAKE";

/// Local file path used instead of the pinned HTTPS URL (offline fixture).
pub const ENV_SEMANTIC_MODEL_SOURCE: &str = "LATTICE_SEMANTIC_MODEL_SOURCE";

/// Progress callback: `(bytes_copied, total_bytes_hint)`.
pub type ProgressFn<'a> = dyn FnMut(u64, Option<u64>) + 'a;

/// Result of acquiring and installing the pinned embedding model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcquireResult {
    pub model_dir: PathBuf,
    pub artifact_path: PathBuf,
    pub artifact_sha256: String,
    pub skipped_download: bool,
}

fn acquire_mutex() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// Whether env requests the Fake / offline-CI path (no network).
pub fn semantic_fake_enabled() -> bool {
    env_truthy(ENV_SEMANTIC_FAKE)
}

/// Optional local artifact path from [`ENV_SEMANTIC_MODEL_SOURCE`].
pub fn semantic_model_source_override() -> Option<PathBuf> {
    std::env::var(ENV_SEMANTIC_MODEL_SOURCE)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// True when the pinned model is already installed and sha256-verified.
pub fn pinned_model_is_ready() -> bool {
    let model_dir = qwen3_embedding_install_dir();
    let manifest_path = model_dir.join("manifest.json");
    let artifact_path = model_dir.join(QWEN3_EMBEDDING_ARTIFACT);
    if !manifest_path.is_file() || !artifact_path.is_file() {
        return false;
    }
    verify_file_sha256(&artifact_path, QWEN3_EMBEDDING_SHA256).is_ok()
}

/// Acquire the pinned Qwen3 Q8 GGUF into the models directory.
///
/// - [`ENV_SEMANTIC_FAKE`]: returns `skipped_download` without touching disk.
/// - Already-verified install: returns immediately.
/// - [`ENV_SEMANTIC_MODEL_SOURCE`]: copies a local file (CI fixture).
/// - Otherwise: HTTPS download of the pinned URL.
///
/// Fail-closed on sha256 mismatch (partial/corrupt artifacts are removed).
/// Concurrent callers serialize on a process-wide lock so they cannot clobber
/// the same staging file (which produced jumpy UI progress and "artifact not found").
pub fn acquire_pinned_embedding_model(
    progress: &mut ProgressFn<'_>,
) -> Result<AcquireResult, EmbeddingError> {
    if semantic_fake_enabled() {
        return Ok(AcquireResult {
            model_dir: qwen3_embedding_install_dir(),
            artifact_path: qwen3_embedding_install_dir().join(QWEN3_EMBEDDING_ARTIFACT),
            artifact_sha256: QWEN3_EMBEDDING_SHA256.into(),
            skipped_download: true,
        });
    }

    let _guard = acquire_mutex()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    // Re-check under the lock — a concurrent caller may have finished install.
    if pinned_model_is_ready() {
        let model_dir = qwen3_embedding_install_dir();
        return Ok(AcquireResult {
            artifact_path: model_dir.join(QWEN3_EMBEDDING_ARTIFACT),
            model_dir,
            artifact_sha256: QWEN3_EMBEDDING_SHA256.into(),
            skipped_download: true,
        });
    }

    let manifest = qwen3_embedding_0_6b_q8_manifest();
    let download_dir = embeddings_download_dir();
    fs::create_dir_all(&download_dir).map_err(|error| {
        EmbeddingError::provider(format!(
            "failed to create download dir {}: {error}",
            download_dir.display()
        ))
    })?;
    // Unique staging name so a cancelled/overlapping attempt cannot delete our bytes.
    let staging = download_dir.join(format!(
        "{QWEN3_EMBEDDING_ARTIFACT}.{}.partial",
        staging_token()
    ));
    let _ = fs::remove_file(&staging);

    let mut last_percent = 0u32;
    let mut monotonic = |copied: u64, total: Option<u64>| {
        let percent = download_progress_percent(copied, total);
        if percent >= last_percent {
            last_percent = percent;
            progress(copied, total);
        }
    };

    let download_result = if let Some(source) = semantic_model_source_override() {
        copy_with_progress(
            &source,
            &staging,
            Some(file_len(&source)?),
            &mut monotonic,
        )
    } else {
        download_https_with_progress(
            QWEN3_EMBEDDING_DOWNLOAD_URL,
            &staging,
            Some(QWEN3_EMBEDDING_SIZE_BYTES),
            &mut monotonic,
        )
    };

    if let Err(error) = download_result {
        let _ = fs::remove_file(&staging);
        return Err(error);
    }

    match verify_file_sha256(&staging, &manifest.sha256) {
        Ok(()) => {}
        Err(error) => {
            let _ = fs::remove_file(&staging);
            return Err(error);
        }
    }

    let model_dir = match install_artifact_beside_manifest(
        &manifest,
        &staging,
        &embeddings_models_dir(),
    ) {
        Ok(dir) => dir,
        Err(error) => {
            let _ = fs::remove_file(&staging);
            return Err(error);
        }
    };
    let _ = fs::remove_file(&staging);
    Ok(AcquireResult {
        artifact_path: model_dir.join(&manifest.artifact),
        artifact_sha256: manifest.sha256.clone(),
        model_dir,
        skipped_download: false,
    })
}

/// Install a verified local artifact into `{models_dir}/{slug}/`.
pub fn install_artifact_beside_manifest(
    manifest: &ModelManifest,
    artifact_path: &Path,
    models_dir: &Path,
) -> Result<PathBuf, EmbeddingError> {
    manifest.validate()?;
    if !artifact_path.is_file() {
        return Err(EmbeddingError::ArtifactNotFound {
            path: artifact_path.display().to_string(),
        });
    }
    verify_file_sha256(artifact_path, &manifest.sha256)?;

    let slug = if manifest.model_id == crate::pinned::QWEN3_EMBEDDING_MODEL_ID {
        QWEN3_EMBEDDING_INSTALL_SLUG.to_string()
    } else {
        model_slug(&manifest.model_id)
    };
    let model_dir = models_dir.join(slug);
    fs::create_dir_all(&model_dir).map_err(|error| {
        EmbeddingError::provider(format!(
            "failed to create model dir {}: {error}",
            model_dir.display()
        ))
    })?;

    let dest_manifest = model_dir.join("manifest.json");
    let dest_artifact = model_dir.join(&manifest.artifact);
    let manifest_bytes = serde_json::to_vec_pretty(manifest)
        .map_err(|error| EmbeddingError::manifest(error.to_string()))?;
    fs::write(&dest_manifest, manifest_bytes).map_err(|error| {
        EmbeddingError::provider(format!(
            "failed to write manifest {}: {error}",
            dest_manifest.display()
        ))
    })?;
    // Prefer rename (same volume) so we do not leave a half-copied dest; fall back to copy.
    if fs::rename(artifact_path, &dest_artifact).is_err() {
        fs::copy(artifact_path, &dest_artifact).map_err(|error| {
            EmbeddingError::provider(format!(
                "failed to copy artifact to {}: {error}",
                dest_artifact.display()
            ))
        })?;
    }
    verify_file_sha256(&dest_artifact, &manifest.sha256)?;
    Ok(model_dir)
}

/// Map copied/total bytes into a 0–100 percent for status UI.
pub fn download_progress_percent(copied: u64, total: Option<u64>) -> u32 {
    let Some(total) = total.filter(|value| *value > 0) else {
        return 0;
    };
    ((copied.min(total) as u128 * 100) / total as u128) as u32
}

/// Choose a stable progress denominator: prefer the pinned expected size when the
/// HTTP Content-Length is missing or disagrees (HF redirects / chunked / HTML errors).
pub fn progress_total_hint(content_length: Option<u64>, expected_size: Option<u64>) -> Option<u64> {
    match (content_length, expected_size) {
        (Some(header), Some(expected)) if header > 0 => {
            let delta = header.abs_diff(expected);
            let tolerance = (expected / 100).max(1024);
            if delta <= tolerance {
                Some(header)
            } else {
                Some(expected)
            }
        }
        (_, Some(expected)) if expected > 0 => Some(expected),
        (Some(header), _) if header > 0 => Some(header),
        _ => None,
    }
}

fn staging_token() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{}-{nanos}", std::process::id())
}

fn env_truthy(name: &str) -> bool {
    matches!(
        std::env::var(name).ok().as_deref(),
        Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("YES")
    )
}

fn file_len(path: &Path) -> Result<u64, EmbeddingError> {
    fs::metadata(path)
        .map(|meta| meta.len())
        .map_err(|_| EmbeddingError::ArtifactNotFound {
            path: path.display().to_string(),
        })
}

fn copy_with_progress(
    source: &Path,
    dest: &Path,
    total: Option<u64>,
    progress: &mut ProgressFn<'_>,
) -> Result<(), EmbeddingError> {
    let mut input = File::open(source).map_err(|_| EmbeddingError::ArtifactNotFound {
        path: source.display().to_string(),
    })?;
    let mut output = File::create(dest).map_err(|error| {
        EmbeddingError::provider(format!("failed to create {}: {error}", dest.display()))
    })?;
    stream_copy(&mut input, &mut output, total, progress)
}

fn download_https_with_progress(
    url: &str,
    dest: &Path,
    expected_size: Option<u64>,
    progress: &mut ProgressFn<'_>,
) -> Result<(), EmbeddingError> {
    let response = ureq::get(url).call().map_err(|error| {
        EmbeddingError::provider(format!("download failed for {url}: {error}"))
    })?;
    let header_len = response
        .header("Content-Length")
        .and_then(|value| value.parse::<u64>().ok());
    let total = progress_total_hint(header_len, expected_size);
    let mut reader = response.into_reader();
    let mut output = File::create(dest).map_err(|error| {
        EmbeddingError::provider(format!("failed to create {}: {error}", dest.display()))
    })?;
    stream_copy(&mut reader, &mut output, total, progress)?;
    // If the body ended early vs pinned size, fail before sha (clearer than mismatch).
    if let Some(expected) = expected_size {
        let actual = file_len(dest)?;
        if actual != expected {
            let _ = fs::remove_file(dest);
            return Err(EmbeddingError::provider(format!(
                "download size mismatch for {url}: got {actual} bytes, expected {expected}"
            )));
        }
    }
    Ok(())
}

fn stream_copy(
    input: &mut impl Read,
    output: &mut impl Write,
    total: Option<u64>,
    progress: &mut ProgressFn<'_>,
) -> Result<(), EmbeddingError> {
    let mut buffer = [0u8; 64 * 1024];
    let mut copied = 0u64;
    progress(0, total);
    loop {
        let read = input
            .read(&mut buffer)
            .map_err(|error| EmbeddingError::provider(error.to_string()))?;
        if read == 0 {
            break;
        }
        output
            .write_all(&buffer[..read])
            .map_err(|error| EmbeddingError::provider(error.to_string()))?;
        copied += read as u64;
        progress(copied, total);
    }
    output
        .flush()
        .map_err(|error| EmbeddingError::provider(error.to_string()))?;
    if let Some(total) = total {
        // Emit a final 100% tick when we hit the expected length.
        if copied >= total {
            progress(total, Some(total));
        }
    }
    Ok(())
}

fn model_slug(model_id: &str) -> String {
    let trimmed = model_id.trim();
    let leaf = trimmed
        .rsplit('/')
        .next()
        .unwrap_or(trimmed)
        .to_ascii_lowercase();
    let slug: String = leaf
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect();
    if slug.is_empty() {
        "model".into()
    } else {
        slug
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sha256_hex;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn progress_percent_maps_bounds() {
        assert_eq!(download_progress_percent(0, Some(100)), 0);
        assert_eq!(download_progress_percent(50, Some(100)), 50);
        assert_eq!(download_progress_percent(100, Some(100)), 100);
        assert_eq!(download_progress_percent(10, None), 0);
        assert_eq!(download_progress_percent(150, Some(100)), 100);
    }

    #[test]
    fn progress_total_prefers_pinned_when_header_disagrees() {
        assert_eq!(
            progress_total_hint(Some(1024), Some(QWEN3_EMBEDDING_SIZE_BYTES)),
            Some(QWEN3_EMBEDDING_SIZE_BYTES)
        );
        assert_eq!(
            progress_total_hint(
                Some(QWEN3_EMBEDDING_SIZE_BYTES),
                Some(QWEN3_EMBEDDING_SIZE_BYTES)
            ),
            Some(QWEN3_EMBEDDING_SIZE_BYTES)
        );
        assert_eq!(
            progress_total_hint(None, Some(QWEN3_EMBEDDING_SIZE_BYTES)),
            Some(QWEN3_EMBEDDING_SIZE_BYTES)
        );
    }

    #[test]
    fn acquire_from_local_source_verifies_and_installs() {
        let bytes = b"fixture-qwen3-bytes";
        let sha = sha256_hex(bytes);
        let dir = tempfile::tempdir().unwrap();
        let artifact = dir.path().join("fixture.bin");
        fs::write(&artifact, bytes).unwrap();
        let mut manifest = qwen3_embedding_0_6b_q8_manifest();
        manifest.artifact = "fixture.bin".into();
        manifest.sha256 = sha.clone();
        let models = dir.path().join("embeddings");
        let mut reported = Vec::new();
        copy_with_progress(
            &artifact,
            &dir.path().join("staging.bin"),
            Some(bytes.len() as u64),
            &mut |copied, total| reported.push((copied, total)),
        )
        .unwrap();
        assert!(!reported.is_empty());
        let model_dir = install_artifact_beside_manifest(&manifest, &artifact, &models).unwrap();
        assert!(model_dir.join("manifest.json").is_file());
        assert!(model_dir.join("fixture.bin").is_file());
        verify_file_sha256(&model_dir.join("fixture.bin"), &sha).unwrap();
    }

    #[test]
    fn local_model_source_env_installs_pinned_sha_fixture() {
        let _guard = env_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var(crate::ENV_LATTICE_HOME, dir.path());
        std::env::remove_var(ENV_SEMANTIC_FAKE);

        let fixture = dir.path().join("source.gguf");
        let bytes = b"offline-fixture-for-progress";
        fs::write(&fixture, bytes).unwrap();
        std::env::set_var(ENV_SEMANTIC_MODEL_SOURCE, &fixture);

        let err = acquire_pinned_embedding_model(&mut |_, _| {}).unwrap_err();
        assert!(matches!(
            err,
            EmbeddingError::ArtifactSha256Mismatch { .. }
        ));
        assert!(!qwen3_embedding_install_dir()
            .join(QWEN3_EMBEDDING_ARTIFACT)
            .exists());

        std::env::remove_var(ENV_SEMANTIC_MODEL_SOURCE);
        std::env::remove_var(crate::ENV_LATTICE_HOME);
    }

    #[test]
    fn corrupt_artifact_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let artifact = dir.path().join("bad.bin");
        fs::write(&artifact, b"not-the-expected-bytes").unwrap();
        let mut manifest = qwen3_embedding_0_6b_q8_manifest();
        manifest.artifact = "bad.bin".into();
        let err = install_artifact_beside_manifest(&manifest, &artifact, &dir.path().join("models"))
            .unwrap_err();
        assert!(matches!(
            err,
            EmbeddingError::ArtifactSha256Mismatch { .. }
        ));
        assert!(!dir
            .path()
            .join("models")
            .join(QWEN3_EMBEDDING_INSTALL_SLUG)
            .join("bad.bin")
            .exists());
    }

    #[test]
    fn fake_env_skips_download() {
        let _guard = env_lock().lock().unwrap();
        std::env::set_var(ENV_SEMANTIC_FAKE, "1");
        let mut calls = 0u32;
        let result = acquire_pinned_embedding_model(&mut |_, _| {
            calls += 1;
        })
        .unwrap();
        std::env::remove_var(ENV_SEMANTIC_FAKE);
        assert!(result.skipped_download);
        assert_eq!(calls, 0);
    }
}
