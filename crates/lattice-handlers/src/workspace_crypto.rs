//! In-process workspace crypto session (DEK never in webview).

use std::sync::{Mutex, OnceLock};

use lattice_workspace_crypto::{
    MemoryKeystore, WorkspaceCryptoSession,
};
#[cfg(feature = "keychain")]
use lattice_workspace_crypto::KeychainKeystore;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceCryptoStatus {
    pub unlocked: bool,
    pub workspace_id: Option<String>,
}

fn map_err(err: impl std::fmt::Display) -> String {
    err.to_string()
}

#[cfg(feature = "keychain")]
type ActiveKeystore = KeychainKeystore;
#[cfg(not(feature = "keychain"))]
type ActiveKeystore = MemoryKeystore;

fn crypto_session() -> &'static Mutex<WorkspaceCryptoSession<ActiveKeystore>> {
    static SESSION: OnceLock<Mutex<WorkspaceCryptoSession<ActiveKeystore>>> = OnceLock::new();
    SESSION.get_or_init(|| {
        Mutex::new(WorkspaceCryptoSession::new(ActiveKeystore::new()))
    })
}

pub fn workspace_crypto_status() -> WorkspaceCryptoStatus {
    let session = crypto_session()
        .lock()
        .expect("workspace crypto session lock poisoned");
    WorkspaceCryptoStatus {
        unlocked: session.is_unlocked(),
        workspace_id: session.unlocked_workspace_id().map(str::to_string),
    }
}

/// Unlock (or provision) the workspace DEK in Rust memory.
pub fn workspace_crypto_unlock(workspace_id: String) -> Result<WorkspaceCryptoStatus, String> {
    let workspace_id = workspace_id.trim().to_string();
    if workspace_id.is_empty() {
        return Err("workspace id is required".into());
    }
    let mut session = crypto_session()
        .lock()
        .map_err(|_| "workspace crypto session lock poisoned".to_string())?;
    if session.is_unlocked() {
        let current = session.unlocked_workspace_id().unwrap_or_default();
        if current != workspace_id {
            return Err(format!(
                "another workspace is already unlocked ({current}); lock before switching"
            ));
        }
        return Ok(workspace_crypto_status_from(&session));
    }
    if session.unlock(&workspace_id).is_err() {
        session.provision(&workspace_id).map_err(map_err)?;
    }
    Ok(workspace_crypto_status_from(&session))
}

pub fn workspace_crypto_lock() -> Result<WorkspaceCryptoStatus, String> {
    let mut session = crypto_session()
        .lock()
        .map_err(|_| "workspace crypto session lock poisoned".to_string())?;
    session.lock();
    Ok(workspace_crypto_status_from(&session))
}

pub(crate) fn with_unlocked_session<T>(
    workspace_id: &str,
    f: impl FnOnce(&WorkspaceCryptoSession<ActiveKeystore>) -> Result<T, String>,
) -> Result<T, String> {
    let session = crypto_session()
        .lock()
        .map_err(|_| "workspace crypto session lock poisoned".to_string())?;
    if !session.is_unlocked() {
        return Err("workspace is locked; unlock encryption before backup".into());
    }
    if session.unlocked_workspace_id() != Some(workspace_id) {
        return Err("unlocked workspace does not match the open workspace".into());
    }
    f(&session)
}

fn workspace_crypto_status_from(session: &WorkspaceCryptoSession<ActiveKeystore>) -> WorkspaceCryptoStatus {
    WorkspaceCryptoStatus {
        unlocked: session.is_unlocked(),
        workspace_id: session.unlocked_workspace_id().map(str::to_string),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice_workspace_crypto::MemoryKeystore;

    #[test]
    fn memory_session_provision_and_lock() {
        let store = MemoryKeystore::new();
        let mut session = WorkspaceCryptoSession::new(store);
        session.provision("ws-test").unwrap();
        assert!(session.is_unlocked());
        session.lock();
        assert!(!session.is_unlocked());
    }
}
