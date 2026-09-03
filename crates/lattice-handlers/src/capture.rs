use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use lattice_capture_core::{encode_storage_image, png_bytes_from_capture, CapturedImage};
use lattice_commands::{Command as SemanticCommand, CommandEngine, Transaction};
use lattice_storage::{NativeWorkspaceStore, WorkspaceStore};

use crate::error::command_error_to_string;
use crate::path::{join_within_root, validate_workspace_relative};

/// Maximum inbox capture asset size (matches desktop editor asset import limit).
pub const MAX_INBOX_CAPTURE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboxCaptureResult {
    pub page_path: String,
    pub asset_path: String,
}

/// Create a capture inbox page and co-located asset in one transaction.
pub fn create_inbox_capture(
    root: String,
    image_bytes: Vec<u8>,
    file_name: String,
    directory: Option<String>,
) -> Result<InboxCaptureResult, String> {
    if image_bytes.is_empty() {
        return Err("cannot import an empty capture".to_string());
    }
    if image_bytes.len() > MAX_INBOX_CAPTURE_BYTES {
        return Err(format!(
            "capture assets are limited to {} MiB",
            MAX_INBOX_CAPTURE_BYTES / (1024 * 1024)
        ));
    }

    let inbox_dir = directory
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "Inbox".to_string());
    validate_workspace_relative(&inbox_dir)?;

    let page_rel = capture_page_path(&inbox_dir, SystemTime::now());
    validate_workspace_relative(&page_rel)?;

    let safe_name = validate_asset_file_name(&file_name)?;
    let (canonical_root, relative_page) = join_within_root(&root, &page_rel)?;
    let page_dir = relative_page.parent().unwrap_or_else(|| Path::new(""));
    let asset_rel = resolve_collision_free_asset_path(&canonical_root, page_dir, &safe_name)?;
    let asset_rel_to_page = asset_rel
        .strip_prefix(page_dir)
        .unwrap_or(&asset_rel)
        .to_string_lossy()
        .replace('\\', "/");

    let title = "Screen clip".to_string();
    let page_content = format!("# {title}\n\n![]({asset_rel_to_page})\n");

    let mut engine = CommandEngine::open(&canonical_root).map_err(command_error_to_string)?;
    let receipt = engine
        .apply(Transaction::new(
            format!("Capture inbox {}", relative_page.display()),
            vec![
                SemanticCommand::PageCreate {
                    path: relative_page.clone(),
                    content: page_content,
                },
                SemanticCommand::ResourceCreate {
                    path: asset_rel.clone(),
                    content: image_bytes,
                },
            ],
        ))
        .map_err(command_error_to_string)?;

    if receipt.outcomes.len() < 2 {
        return Err("capture inbox transaction did not apply both page and asset".to_string());
    }

    Ok(InboxCaptureResult {
        page_path: relative_page.to_string_lossy().replace('\\', "/"),
        asset_path: asset_rel_to_page,
    })
}

/// Encode a clipboard PNG rendition and ingest it into the Capture Inbox.
pub fn ingest_png_capture(
    root: String,
    png_bytes: Vec<u8>,
    directory: Option<String>,
) -> Result<InboxCaptureResult, String> {
    let (storage_name, storage_bytes) = encode_storage_image(&png_bytes)?;
    create_inbox_capture(root, storage_bytes, storage_name, directory)
}

/// Encode backend pixels and ingest into the Capture Inbox.
pub fn ingest_captured_image(
    root: String,
    captured: &CapturedImage,
    directory: Option<String>,
) -> Result<InboxCaptureResult, String> {
    let png_bytes = png_bytes_from_capture(captured)?;
    ingest_png_capture(root, png_bytes, directory)
}

fn validate_asset_file_name(file_name: &str) -> Result<String, String> {
    let safe_name = Path::new(file_name)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty() && *name != "." && *name != "..")
        .ok_or_else(|| "capture filename is invalid".to_string())?;
    if safe_name != file_name {
        return Err("capture filename must not contain path separators".to_string());
    }
    Ok(safe_name.to_string())
}

fn resolve_collision_free_asset_path(
    canonical_root: &Path,
    page_dir: &Path,
    safe_name: &str,
) -> Result<PathBuf, String> {
    let asset_dir = page_dir.join("assets");
    let file_path = Path::new(safe_name);
    let stem = file_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("capture");
    let extension = file_path.extension().and_then(|value| value.to_str());

    let store = NativeWorkspaceStore::new(canonical_root);
    let mut candidate = asset_dir.join(safe_name);
    let mut suffix = 2usize;
    while store.metadata(&candidate).is_ok() {
        let next_name = match extension {
            Some(extension) => format!("{stem} {suffix}.{extension}"),
            None => format!("{stem} {suffix}"),
        };
        candidate = asset_dir.join(next_name);
        suffix += 1;
    }
    Ok(candidate)
}

/// Filesystem-safe UTC timestamp (`:` and `.` replaced with `-`), millisecond precision.
pub fn capture_page_path(directory: &str, now: SystemTime) -> String {
    let normalized = directory.trim().trim_matches('/').trim_matches('\\');
    let folder = if normalized.is_empty() {
        "Inbox"
    } else {
        normalized
    };
    format!("{folder}/{}.md", file_timestamp(now))
}

fn file_timestamp(now: SystemTime) -> String {
    let duration = now.duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = duration.as_secs() as i64;
    let millis = duration.subsec_millis();
    let days = secs.div_euclid(86_400);
    let tod = secs.rem_euclid(86_400);
    let hour = tod / 3_600;
    let minute = (tod % 3_600) / 60;
    let second = tod % 60;
    let (year, month, day) = utc_civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}-{minute:02}-{second:02}-{millis:03}Z")
}

fn utc_civil_from_days(days_since_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mp < 10 { y } else { y + 1 };
    (y as i32, m as u32, d as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice_core::Workspace;

    fn init_workspace() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "Test Workspace").unwrap();
        dir
    }

    #[test]
    fn ingest_png_capture_writes_page_and_asset() {
        let dir = init_workspace();
        let root = dir.path().to_string_lossy().into_owned();
        let fake_png = minimal_test_png();

        let result = ingest_png_capture(root.clone(), fake_png, Some("Inbox".to_string())).unwrap();

        assert!(result.page_path.starts_with("Inbox/"));
        assert!(result.page_path.ends_with(".md"));
        assert_eq!(result.asset_path, "assets/capture.webp");

        let page_abs = dir.path().join(&result.page_path);
        let asset_abs = page_abs.parent().unwrap().join(&result.asset_path);
        assert!(page_abs.is_file());
        assert!(asset_abs.is_file());

        let markdown = std::fs::read_to_string(page_abs).unwrap();
        assert!(markdown.contains("# Screen clip"));
        assert!(markdown.contains("![](assets/capture.webp)"));
    }

    fn minimal_test_png() -> Vec<u8> {
        use image::codecs::png::PngEncoder;
        use image::{ExtendedColorType, ImageEncoder, Rgba, RgbaImage};
        let img = RgbaImage::from_pixel(1, 1, Rgba([255, 0, 0, 255]));
        let mut buf = Vec::new();
        PngEncoder::new(&mut buf)
            .write_image(img.as_raw(), 1, 1, ExtendedColorType::Rgba8)
            .unwrap();
        buf
    }

    #[test]
    fn create_inbox_capture_writes_page_and_asset() {
        let dir = init_workspace();
        let root = dir.path().to_string_lossy().into_owned();
        let fake_png = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00];

        let result = create_inbox_capture(
            root.clone(),
            fake_png.clone(),
            "capture.webp".to_string(),
            Some("Inbox".to_string()),
        )
        .unwrap();

        assert!(result.page_path.starts_with("Inbox/"));
        assert!(result.page_path.ends_with(".md"));
        assert_eq!(result.asset_path, "assets/capture.webp");

        let page_abs = dir.path().join(&result.page_path);
        let asset_abs = page_abs.parent().unwrap().join(&result.asset_path);
        assert!(page_abs.is_file());
        assert!(asset_abs.is_file());
        assert_eq!(std::fs::read(asset_abs).unwrap(), fake_png);

        let markdown = std::fs::read_to_string(page_abs).unwrap();
        assert!(markdown.contains("# Screen clip"));
        assert!(markdown.contains("![](assets/capture.webp)"));
    }

    #[test]
    fn rejects_empty_capture_bytes() {
        let dir = init_workspace();
        let root = dir.path().to_string_lossy().into_owned();
        let err =
            create_inbox_capture(root, Vec::new(), "capture.webp".to_string(), None).unwrap_err();
        assert!(err.contains("empty"));
    }

    #[test]
    fn file_timestamp_replaces_colons_and_dots() {
        let ts = file_timestamp(SystemTime::UNIX_EPOCH);
        assert!(!ts.contains(':'));
        assert!(!ts.contains('.'));
        assert!(ts.ends_with('Z'));
    }

    #[test]
    fn capture_page_path_normalizes_directory() {
        let now = SystemTime::UNIX_EPOCH;
        assert!(capture_page_path("Inbox", now).starts_with("Inbox/"));
        assert!(capture_page_path("Inbox", now).ends_with(".md"));
        assert!(capture_page_path("  Capture/Quick  ", now).starts_with("Capture/Quick/"));
        assert!(capture_page_path("", now).starts_with("Inbox/"));
    }

    #[test]
    fn rejects_oversized_capture_bytes() {
        let dir = init_workspace();
        let root = dir.path().to_string_lossy().into_owned();
        let oversized = vec![0xFF; MAX_INBOX_CAPTURE_BYTES + 1];
        let err =
            create_inbox_capture(root, oversized, "capture.webp".to_string(), None).unwrap_err();
        assert!(err.contains("8 MiB"));
    }

    #[test]
    fn rejects_invalid_capture_filenames() {
        let dir = init_workspace();
        let root = dir.path().to_string_lossy().into_owned();
        let bytes = vec![0x89, 0x50, 0x4E, 0x47];

        for file_name in ["", ".", "..", "foo/bar.webp", "../capture.webp"] {
            let err =
                create_inbox_capture(root.clone(), bytes.clone(), file_name.to_string(), None)
                    .unwrap_err();
            assert!(
                err.contains("invalid") || err.contains("path separators"),
                "unexpected error for {file_name:?}: {err}"
            );
        }
    }

    #[test]
    fn resolve_collision_free_asset_path_renames_existing_file() {
        let dir = init_workspace();
        let root = dir.path();
        let page_dir = Path::new("Inbox/2026-07-15T20-32-05-123Z");
        let assets_dir = root.join(page_dir).join("assets");
        std::fs::create_dir_all(&assets_dir).unwrap();
        std::fs::write(assets_dir.join("capture.webp"), b"existing").unwrap();

        let resolved = resolve_collision_free_asset_path(root, page_dir, "capture.webp").unwrap();
        assert_eq!(
            resolved,
            PathBuf::from("Inbox/2026-07-15T20-32-05-123Z/assets/capture 2.webp")
        );
    }
}
