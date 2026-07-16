use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use lattice_core::{ResourceCatalog, ResourceLinkResolution, ResourceLinkTarget, Workspace};

#[derive(Default)]
pub struct ResourceCatalogState(Mutex<HashMap<String, ResourceCatalog>>);

impl ResourceCatalogState {
    fn refresh(&self, root: &str) -> Result<(), String> {
        let workspace = Workspace::open(Path::new(root)).map_err(|error| error.to_string())?;
        let resources = workspace.scan().map_err(|error| error.to_string())?;
        self.0
            .lock()
            .map_err(|_| "resource catalog lock poisoned".to_string())?
            .insert(root.to_string(), ResourceCatalog::new(&resources));
        Ok(())
    }

    fn with_catalog<T>(
        &self,
        root: &str,
        use_catalog: impl FnOnce(&ResourceCatalog) -> T,
    ) -> Result<T, String> {
        let missing = !self
            .0
            .lock()
            .map_err(|_| "resource catalog lock poisoned".to_string())?
            .contains_key(root);
        if missing {
            self.refresh(root)?;
        }
        let catalogs = self
            .0
            .lock()
            .map_err(|_| "resource catalog lock poisoned".to_string())?;
        let catalog = catalogs
            .get(root)
            .ok_or_else(|| "resource catalog unavailable".to_string())?;
        Ok(use_catalog(catalog))
    }
}

#[tauri::command]
pub fn refresh_resource_catalog(
    root: String,
    state: tauri::State<ResourceCatalogState>,
) -> Result<(), String> {
    state.refresh(&root)
}

#[tauri::command]
pub fn search_resource_links(
    root: String,
    query: String,
    limit: usize,
    state: tauri::State<ResourceCatalogState>,
) -> Result<Vec<ResourceLinkTarget>, String> {
    state.with_catalog(&root, |catalog| catalog.search(&query, limit))
}

#[tauri::command]
pub fn resolve_resource_link(
    root: String,
    source_path: Option<String>,
    target: String,
    state: tauri::State<ResourceCatalogState>,
) -> Result<ResourceLinkResolution, String> {
    state.with_catalog(&root, |catalog| {
        catalog.resolve(source_path.as_deref().map(Path::new), &target)
    })
}
