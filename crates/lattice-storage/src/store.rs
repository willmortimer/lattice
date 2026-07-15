use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::SystemTime;

use crate::revision::ResourceRevision;
use crate::{Error, Result};

/// One entry yielded by [`WorkspaceStore::list`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceEntry {
    /// Workspace-relative path.
    pub path: PathBuf,
    pub is_dir: bool,
}

/// Metadata for a single path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceMetadata {
    pub revision: ResourceRevision,
    pub is_dir: bool,
}

/// Storage abstraction per `docs/05`. All paths are workspace-relative and
/// must stay within the store root; escaping paths are rejected.
pub trait WorkspaceStore: Send + Sync {
    fn read(&self, path: &Path) -> Result<Vec<u8>>;
    fn write_atomic(&self, path: &Path, data: &[u8]) -> Result<ResourceRevision>;
    fn list(&self, path: &Path) -> Result<Vec<ResourceEntry>>;
    fn metadata(&self, path: &Path) -> Result<ResourceMetadata>;
    fn rename(&self, from: &Path, to: &Path) -> Result<()>;
    fn remove(&self, path: &Path) -> Result<()>;
}

/// Normalize a workspace-relative path, rejecting anything that would escape
/// the root. `CurDir` components are dropped; `ParentDir`, absolute roots, and
/// Windows prefixes are hard errors. No symlink resolution (not needed for v0).
pub(crate) fn normalize_relative(path: &Path) -> Result<PathBuf> {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(Error::OutsideWorkspace {
                    path: path.to_path_buf(),
                });
            }
        }
    }
    Ok(out)
}

/// Process-unique-ish suffix for temp files, so concurrent writers in the
/// same directory never collide on the sibling temp name.
fn temp_suffix() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{}-{}-{}", std::process::id(), nanos, seq)
}

// -------------------------------------------------------------------------
// NativeWorkspaceStore
// -------------------------------------------------------------------------

/// A [`WorkspaceStore`] backed by a directory on the real filesystem.
pub struct NativeWorkspaceStore {
    root: PathBuf,
}

impl NativeWorkspaceStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        NativeWorkspaceStore { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Resolve a workspace-relative path to an absolute path under the root,
    /// rejecting escapes.
    fn resolve(&self, path: &Path) -> Result<PathBuf> {
        Ok(self.root.join(normalize_relative(path)?))
    }

    /// The current on-disk revision of `path`, or `None` if it does not exist.
    /// Used by [`crate::BufferedWriter`] for optimistic conflict checks.
    pub(crate) fn current_revision(&self, path: &Path) -> Result<Option<ResourceRevision>> {
        match self.metadata(path) {
            Ok(meta) => Ok(Some(meta.revision)),
            Err(Error::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {
                Ok(None)
            }
            Err(other) => Err(other),
        }
    }
}

impl WorkspaceStore for NativeWorkspaceStore {
    fn read(&self, path: &Path) -> Result<Vec<u8>> {
        let full = self.resolve(path)?;
        std::fs::read(&full).map_err(|e| Error::io(&full, e))
    }

    fn write_atomic(&self, path: &Path, data: &[u8]) -> Result<ResourceRevision> {
        let full = self.resolve(path)?;
        let parent = full.parent().ok_or_else(|| Error::OutsideWorkspace {
            path: path.to_path_buf(),
        })?;
        std::fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;

        let name =
            full.file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| Error::OutsideWorkspace {
                    path: path.to_path_buf(),
                })?;

        // Permissions of the file being replaced, so an atomic replace does
        // not silently reset the mode a user or tool set.
        let existing_permissions = std::fs::metadata(&full).ok().map(|m| m.permissions());

        let temp = parent.join(format!(".{name}.lattice-tmp-{}", temp_suffix()));

        // Write the temp file, flush its bytes to disk, then rename over the
        // target. A crash before the rename leaves the target untouched.
        let write_result = (|| -> std::io::Result<()> {
            use std::io::Write;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)?;
            file.write_all(data)?;
            if let Some(permissions) = &existing_permissions {
                file.set_permissions(permissions.clone())?;
            }
            file.sync_all()?;
            Ok(())
        })();
        if let Err(e) = write_result {
            let _ = std::fs::remove_file(&temp);
            return Err(Error::io(&temp, e));
        }

        if let Err(e) = std::fs::rename(&temp, &full) {
            let _ = std::fs::remove_file(&temp);
            return Err(Error::io(&full, e));
        }

        // Best-effort durability of the directory entry itself.
        if let Ok(dir) = std::fs::File::open(parent) {
            let _ = dir.sync_all();
        }

        let modified = std::fs::metadata(&full)
            .and_then(|m| m.modified())
            .unwrap_or_else(|_| SystemTime::now());
        Ok(ResourceRevision::compute(data, modified))
    }

    fn list(&self, path: &Path) -> Result<Vec<ResourceEntry>> {
        let full = self.resolve(path)?;
        let base = normalize_relative(path)?;
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(&full).map_err(|e| Error::io(&full, e))? {
            let entry = entry.map_err(|e| Error::io(&full, e))?;
            let file_type = entry.file_type().map_err(|e| Error::io(entry.path(), e))?;
            entries.push(ResourceEntry {
                path: base.join(entry.file_name()),
                is_dir: file_type.is_dir(),
            });
        }
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(entries)
    }

    fn metadata(&self, path: &Path) -> Result<ResourceMetadata> {
        let full = self.resolve(path)?;
        let meta = std::fs::metadata(&full).map_err(|e| Error::io(&full, e))?;
        let modified = meta.modified().unwrap_or_else(|_| SystemTime::now());
        if meta.is_dir() {
            return Ok(ResourceMetadata {
                revision: ResourceRevision::compute(&[], modified),
                is_dir: true,
            });
        }
        let data = std::fs::read(&full).map_err(|e| Error::io(&full, e))?;
        Ok(ResourceMetadata {
            revision: ResourceRevision::compute(&data, modified),
            is_dir: false,
        })
    }

    fn rename(&self, from: &Path, to: &Path) -> Result<()> {
        let from_full = self.resolve(from)?;
        let to_full = self.resolve(to)?;
        if let Some(parent) = to_full.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
        }
        std::fs::rename(&from_full, &to_full).map_err(|e| Error::io(&to_full, e))
    }

    fn remove(&self, path: &Path) -> Result<()> {
        let full = self.resolve(path)?;
        let meta = std::fs::metadata(&full).map_err(|e| Error::io(&full, e))?;
        if meta.is_dir() {
            std::fs::remove_dir_all(&full).map_err(|e| Error::io(&full, e))
        } else {
            std::fs::remove_file(&full).map_err(|e| Error::io(&full, e))
        }
    }
}

// -------------------------------------------------------------------------
// MemoryWorkspaceStore
// -------------------------------------------------------------------------

struct MemFile {
    data: Vec<u8>,
    modified: SystemTime,
}

/// An in-memory [`WorkspaceStore`] for tests, previews, and ephemeral scratch.
/// Directories are implicit: any prefix of a stored file path is a directory.
#[derive(Default)]
pub struct MemoryWorkspaceStore {
    files: Mutex<BTreeMap<PathBuf, MemFile>>,
}

impl MemoryWorkspaceStore {
    pub fn new() -> Self {
        MemoryWorkspaceStore::default()
    }
}

impl WorkspaceStore for MemoryWorkspaceStore {
    fn read(&self, path: &Path) -> Result<Vec<u8>> {
        let key = normalize_relative(path)?;
        let files = self.files.lock().unwrap();
        files
            .get(&key)
            .map(|f| f.data.clone())
            .ok_or_else(|| Error::not_found(path.to_path_buf()))
    }

    fn write_atomic(&self, path: &Path, data: &[u8]) -> Result<ResourceRevision> {
        let key = normalize_relative(path)?;
        let modified = SystemTime::now();
        let mut files = self.files.lock().unwrap();
        files.insert(
            key,
            MemFile {
                data: data.to_vec(),
                modified,
            },
        );
        Ok(ResourceRevision::compute(data, modified))
    }

    fn list(&self, path: &Path) -> Result<Vec<ResourceEntry>> {
        let base = normalize_relative(path)?;
        let files = self.files.lock().unwrap();
        let mut seen_dirs = std::collections::BTreeSet::new();
        let mut entries = Vec::new();
        for key in files.keys() {
            let Ok(rest) = key.strip_prefix(&base) else {
                continue;
            };
            let mut components = rest.components();
            let Some(first) = components.next() else {
                continue; // key == base: a file, not a child
            };
            let child = base.join(first.as_os_str());
            if components.next().is_some() {
                // Deeper than one level: `first` names a subdirectory.
                if seen_dirs.insert(child.clone()) {
                    entries.push(ResourceEntry {
                        path: child,
                        is_dir: true,
                    });
                }
            } else {
                entries.push(ResourceEntry {
                    path: child,
                    is_dir: false,
                });
            }
        }
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(entries)
    }

    fn metadata(&self, path: &Path) -> Result<ResourceMetadata> {
        let key = normalize_relative(path)?;
        let files = self.files.lock().unwrap();
        if let Some(file) = files.get(&key) {
            return Ok(ResourceMetadata {
                revision: ResourceRevision::compute(&file.data, file.modified),
                is_dir: false,
            });
        }
        // Root, or any path that is a strict prefix of a stored file, is a dir.
        let is_dir =
            key.as_os_str().is_empty() || files.keys().any(|k| k.starts_with(&key) && k != &key);
        if is_dir {
            Ok(ResourceMetadata {
                revision: ResourceRevision::compute(&[], SystemTime::now()),
                is_dir: true,
            })
        } else {
            Err(Error::not_found(path.to_path_buf()))
        }
    }

    fn rename(&self, from: &Path, to: &Path) -> Result<()> {
        let from_key = normalize_relative(from)?;
        let to_key = normalize_relative(to)?;
        let mut files = self.files.lock().unwrap();
        if let Some(file) = files.remove(&from_key) {
            files.insert(to_key, file);
            return Ok(());
        }
        // Directory move: re-key everything under the prefix.
        let moved: Vec<PathBuf> = files
            .keys()
            .filter(|k| k.starts_with(&from_key))
            .cloned()
            .collect();
        if moved.is_empty() {
            return Err(Error::not_found(from.to_path_buf()));
        }
        for key in moved {
            let file = files.remove(&key).expect("key present");
            let rest = key.strip_prefix(&from_key).expect("prefix matched");
            files.insert(to_key.join(rest), file);
        }
        Ok(())
    }

    fn remove(&self, path: &Path) -> Result<()> {
        let key = normalize_relative(path)?;
        let mut files = self.files.lock().unwrap();
        if files.remove(&key).is_some() {
            return Ok(());
        }
        let removed: Vec<PathBuf> = files
            .keys()
            .filter(|k| k.starts_with(&key))
            .cloned()
            .collect();
        if removed.is_empty() {
            return Err(Error::not_found(path.to_path_buf()));
        }
        for key in removed {
            files.remove(&key);
        }
        Ok(())
    }
}
