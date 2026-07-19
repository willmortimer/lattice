use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use lattice_commands::CommandEngine;
use lattice_core::{ResourceCatalog, Workspace};
use lattice_index::{Backlink, ChunkSearchHit, SearchHit, WorkspaceIndex};

use crate::Result;

/// Long-lived open workspace: warm index, command engine, and optional catalog.
pub struct WorkspaceSession {
    root: PathBuf,
    workspace: Workspace,
    command_engine: Mutex<CommandEngine>,
    index: WorkspaceIndex,
    catalog: Mutex<Option<ResourceCatalog>>,
    index_rebuild_count: AtomicU64,
}

impl WorkspaceSession {
    pub(crate) fn open(canonical_root: &Path) -> Result<Self> {
        let workspace = Workspace::open(canonical_root)?;
        let command_engine = CommandEngine::open(canonical_root)?;
        let index = WorkspaceIndex::open(canonical_root)?;
        let catalog = match workspace.scan() {
            Ok(resources) => Some(ResourceCatalog::new(&resources)),
            Err(_) => None,
        };
        Ok(Self {
            root: canonical_root.to_path_buf(),
            workspace,
            command_engine: Mutex::new(command_engine),
            index,
            catalog: Mutex::new(catalog),
            index_rebuild_count: AtomicU64::new(0),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn workspace(&self) -> &Workspace {
        &self.workspace
    }

    pub fn workspace_id(&self) -> &str {
        &self.workspace.manifest().id
    }

    pub fn index(&self) -> &WorkspaceIndex {
        &self.index
    }

    pub fn index_rebuild_count(&self) -> u64 {
        self.index_rebuild_count.load(Ordering::Relaxed)
    }

    pub fn with_command_engine<R>(&self, f: impl FnOnce(&mut CommandEngine) -> R) -> R {
        let mut engine = self.command_engine.lock().expect("command engine poisoned");
        f(&mut engine)
    }

    pub fn with_resource_catalog<R>(&self, f: impl FnOnce(Option<&ResourceCatalog>) -> R) -> R {
        let catalog = self.catalog.lock().expect("catalog poisoned");
        f(catalog.as_ref())
    }

    /// Ensure the warm index has at least one rebuild when empty.
    ///
    /// Returns `true` when this call performed a rebuild.
    pub fn ensure_index_warm(&self) -> Result<bool> {
        if self.index.resource_count()? == 0 {
            self.index.rebuild(&self.root)?;
            self.index_rebuild_count.fetch_add(1, Ordering::Relaxed);
            return Ok(true);
        }
        Ok(false)
    }

    pub fn rebuild_index(&self) -> Result<u64> {
        let stats = self.index.rebuild(&self.root)?;
        self.index_rebuild_count.fetch_add(1, Ordering::Relaxed);
        Ok(stats.pages_indexed as u64)
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>> {
        self.ensure_index_warm()?;
        Ok(self.index.search(query, limit)?)
    }

    pub fn search_chunks(&self, query: &str, limit: usize) -> Result<Vec<ChunkSearchHit>> {
        self.ensure_index_warm()?;
        Ok(self.index.search_chunks(query, limit)?)
    }

    pub fn backlinks(&self, rel_path: &Path) -> Result<Vec<Backlink>> {
        self.ensure_index_warm()?;
        Ok(self.index.backlinks(rel_path)?)
    }
}
