//! LatticeFS resource authority and materialization for the Inspect panel.

use latticefs_core::{
    resource_stat_or_register, set_resource_authority as latticefs_set_resource_authority,
    ResourceAuthority, ResourceStat,
};

#[tauri::command]
pub fn get_resource_stat(root: String, rel_path: String) -> Result<ResourceStat, String> {
    resource_stat_or_register(std::path::Path::new(&root), &rel_path).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn set_resource_authority(
    root: String,
    rel_path: String,
    authority: ResourceAuthority,
) -> Result<ResourceStat, String> {
    latticefs_set_resource_authority(std::path::Path::new(&root), &rel_path, authority)
        .map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice_core::Workspace;
    use latticefs_core::AuthorityMode;

    #[test]
    fn get_resource_stat_registers_missing_local_file() {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "Inspect").unwrap();
        std::fs::write(dir.path().join("Notes.md"), "# Notes\n").unwrap();

        let stat = get_resource_stat(
            dir.path().to_string_lossy().into_owned(),
            "Notes.md".to_string(),
        )
        .unwrap();

        assert_eq!(stat.path, "Notes.md");
        assert_eq!(stat.authority, AuthorityMode::Local);
    }

    #[test]
    fn set_resource_authority_collaborative_persists_for_get_resource_stat() {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "Inspect").unwrap();
        std::fs::write(dir.path().join("Notes.md"), "# Notes\n").unwrap();

        let root = dir.path().to_string_lossy().into_owned();
        let initial = get_resource_stat(root.clone(), "Notes.md".to_string()).unwrap();
        let doc_id = initial.resource_id;

        let updated = set_resource_authority(
            root.clone(),
            "Notes.md".to_string(),
            ResourceAuthority::Collaborative {
                doc_id,
                materialized_revision: None,
            },
        )
        .unwrap();
        assert_eq!(
            updated.resource_authority,
            ResourceAuthority::Collaborative {
                doc_id,
                materialized_revision: None,
            }
        );

        let reread = get_resource_stat(root, "Notes.md".to_string()).unwrap();
        assert_eq!(reread.resource_authority, updated.resource_authority);
    }
}
