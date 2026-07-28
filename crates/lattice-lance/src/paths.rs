use std::path::{Path, PathBuf};

use lattice_core::OPERATIONAL_DIR;

/// LanceDB table name for workspace search vectors.
pub const SEARCH_ELEMENTS_TABLE: &str = "search-elements";

/// Filename for the workspace search-elements Lance dataset directory.
pub const SEARCH_ELEMENTS_DATASET: &str = "search-elements.lance";

const INDEX_DIR: &str = "index";

/// Resolve the workspace-local LanceDB index directory.
///
/// Returns `{workspace_root}/.lattice/index`. `EmbeddedLanceStore` connects here
/// and uses [`SEARCH_ELEMENTS_TABLE`] as the table name.
pub fn search_elements_index_dir(workspace_root: &Path) -> PathBuf {
    workspace_root.join(OPERATIONAL_DIR).join(INDEX_DIR)
}

/// Resolve the workspace-local path to the search-elements Lance dataset.
///
/// Returns `{workspace_root}/.lattice/index/search-elements.lance`, the on-disk
/// Lance dataset directory for table [`SEARCH_ELEMENTS_TABLE`].
pub fn search_elements_dataset_path(workspace_root: &Path) -> PathBuf {
    search_elements_index_dir(workspace_root).join(SEARCH_ELEMENTS_DATASET)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_elements_dataset_path_under_lattice_index() {
        let workspace = PathBuf::from("/tmp/my-workspace");
        let path = search_elements_dataset_path(&workspace);
        assert_eq!(
            path,
            PathBuf::from("/tmp/my-workspace/.lattice/index/search-elements.lance")
        );
    }
}
