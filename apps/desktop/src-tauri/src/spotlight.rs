//! Workspace catalog for Spotlight / Quick Look helpers.
//!
//! Writes a JSON catalog into the App Group container (when available) so the
//! Quick Look appex and future Core Spotlight indexer share one source. A full
//! `CSSearchableIndex` push is layered on once the Swift helper links
//! CoreSpotlight.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpotlightResource {
    pub path: String,
    pub title: String,
    #[serde(default)]
    pub kind: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpotlightIndexResult {
    pub indexed: usize,
    pub catalog_path: String,
    pub backend: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SpotlightCatalog {
    workspace_root: String,
    updated_at_unix: u64,
    resources: Vec<SpotlightResource>,
}

#[tauri::command]
pub fn spotlight_index_workspace(
    root: String,
    resources: Vec<SpotlightResource>,
) -> Result<SpotlightIndexResult, String> {
    let catalog_dir = catalog_dir()?;
    fs::create_dir_all(&catalog_dir).map_err(|err| err.to_string())?;
    let catalog_path = catalog_dir.join("spotlight-catalog.json");
    let bounded: Vec<SpotlightResource> = resources.into_iter().take(2_000).collect();
    let indexed = bounded.len();
    let catalog = SpotlightCatalog {
        workspace_root: root,
        updated_at_unix: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        resources: bounded,
    };
    let json = serde_json::to_vec_pretty(&catalog).map_err(|err| err.to_string())?;
    fs::write(&catalog_path, json).map_err(|err| err.to_string())?;
    Ok(SpotlightIndexResult {
        indexed,
        catalog_path: catalog_path.display().to_string(),
        backend: "app-group-catalog".into(),
    })
}

fn catalog_dir() -> Result<PathBuf, String> {
    #[cfg(target_os = "macos")]
    {
        // ~/Library/Group Containers/group.dev.lattice.shared/Library/Application Support/Lattice
        if let Some(home) = dirs::home_dir() {
            return Ok(home
                .join("Library/Group Containers")
                .join(lattice_connectors::LATTICE_APP_GROUP)
                .join("Library/Application Support/Lattice"));
        }
    }
    dirs::data_local_dir()
        .map(|p| p.join("Lattice"))
        .ok_or_else(|| "no local data dir".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_catalog() {
        let result = spotlight_index_workspace(
            "/tmp/ws".into(),
            vec![SpotlightResource {
                path: "Notes/A.md".into(),
                title: "A".into(),
                kind: "page".into(),
            }],
        )
        .unwrap();
        assert_eq!(result.indexed, 1);
        assert!(std::path::Path::new(&result.catalog_path).is_file());
        let _ = fs::remove_file(&result.catalog_path);
    }
}
