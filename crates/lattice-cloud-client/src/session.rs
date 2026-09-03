use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use lattice_connectors::{
    probe_token_store_writable, production_token_store, TokenMaterial, TokenStore,
};

use crate::client::{CloudApiClient, CloudHttpClient};
use crate::config::cloud_url;
use crate::error::{CloudError, Result};
use crate::types::CloudSessionStatus;

/// OS keychain service name for the lattice-server bearer session.
pub const CLOUD_TOKEN_SERVICE: &str = "lattice.cloud";
/// Keychain account for the desktop cloud bearer token.
pub const CLOUD_USER_TOKEN_KEY: &str = "lattice.cloud.user";
/// Ephemeral probe account; must not overlap [`CLOUD_USER_TOKEN_KEY`].
pub const CLOUD_PROBE_KEY: &str = "lattice.cloud.probe";
/// Optional absolute path to the shared owner-local session file.
pub const CLOUD_SESSION_FILE_ENV: &str = "LATTICE_CLOUD_SESSION_FILE";
const SESSION_FILE_NAME: &str = "cloud-session";
const PROD_LATTICE_HOME_NAME: &str = "Lattice";
const DEBUG_HOME_RELATIVE: &str = "target/dev-home";

pub trait CloudSessionStore: Send + Sync {
    fn load_token(&self) -> Result<Option<String>>;
    fn save_token(&self, token: &str) -> Result<()>;
    fn clear_token(&self) -> Result<()>;
}

#[derive(Debug, Default, Clone)]
pub struct MemoryCloudSessionStore {
    inner: Arc<Mutex<Option<String>>>,
}

impl MemoryCloudSessionStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl CloudSessionStore for MemoryCloudSessionStore {
    fn load_token(&self) -> Result<Option<String>> {
        Ok(self.inner.lock().unwrap().clone())
    }

    fn save_token(&self, token: &str) -> Result<()> {
        *self.inner.lock().unwrap() = Some(token.to_string());
        Ok(())
    }

    fn clear_token(&self) -> Result<()> {
        *self.inner.lock().unwrap() = None;
        Ok(())
    }
}

pub struct KeychainCloudSessionStore {
    store: Box<dyn TokenStore>,
}

impl KeychainCloudSessionStore {
    pub fn new() -> Self {
        Self {
            store: production_token_store(CLOUD_TOKEN_SERVICE),
        }
    }
}

impl Default for KeychainCloudSessionStore {
    fn default() -> Self {
        Self::new()
    }
}

impl CloudSessionStore for KeychainCloudSessionStore {
    fn load_token(&self) -> Result<Option<String>> {
        match self.store.get(CLOUD_USER_TOKEN_KEY) {
            Ok(Some(token)) => Ok(Some(token.access_token)),
            Ok(None) => Ok(load_shared_session_file()),
            Err(_) => Ok(load_shared_session_file()),
        }
    }

    fn save_token(&self, token: &str) -> Result<()> {
        let keychain = self.store.set(
            CLOUD_USER_TOKEN_KEY,
            &TokenMaterial {
                access_token: token.to_string(),
                refresh_token: None,
                expires_in: None,
                token_type: Some("bearer".into()),
            },
        );
        let file = save_shared_session_file(token);
        match (keychain, file) {
            (Ok(()), _) | (Err(_), Ok(())) => Ok(()),
            (Err(err), Err(_)) => Err(CloudError::Credentials(err.to_string())),
        }
    }

    fn clear_token(&self) -> Result<()> {
        let keychain = self
            .store
            .delete(CLOUD_USER_TOKEN_KEY)
            .map_err(|err| CloudError::Credentials(err.to_string()));
        let file = clear_shared_session_file();
        keychain.or(file)
    }
}

/// Owner-local session file so unsigned `latticed mcp` can use the desktop sign-in.
///
/// Keychain ACL often blocks debug binaries from reading tokens written by
/// Lattice.app. The file lives under Lattice home `State/` (mode 0600 on Unix).
#[derive(Debug, Default, Clone)]
pub struct FileCloudSessionStore;

impl CloudSessionStore for FileCloudSessionStore {
    fn load_token(&self) -> Result<Option<String>> {
        Ok(load_shared_session_file())
    }

    fn save_token(&self, token: &str) -> Result<()> {
        save_shared_session_file(token)
    }

    fn clear_token(&self) -> Result<()> {
        clear_shared_session_file()
    }
}

/// Process-wide store: keychain when writable, otherwise the shared session file.
///
/// Prefer this over a process-local memory store so Cursor's stdio `latticed mcp`
/// can read a session saved by the desktop app.
pub fn process_cloud_session_store() -> &'static dyn CloudSessionStore {
    static KEYCHAIN: OnceLock<KeychainCloudSessionStore> = OnceLock::new();
    static FILE: OnceLock<FileCloudSessionStore> = OnceLock::new();
    static USE_FILE: OnceLock<bool> = OnceLock::new();

    let use_file =
        *USE_FILE.get_or_init(|| !probe_token_store_writable(CLOUD_TOKEN_SERVICE, CLOUD_PROBE_KEY));
    if use_file {
        FILE.get_or_init(FileCloudSessionStore::default)
    } else {
        KEYCHAIN.get_or_init(KeychainCloudSessionStore::new)
    }
}

fn user_profile_dir() -> Option<PathBuf> {
    dirs::home_dir().or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
}

fn session_file_under(home: &Path) -> PathBuf {
    home.join("State").join(SESSION_FILE_NAME)
}

/// Paths that may hold the owner session, in search order.
pub fn shared_session_file_candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(override_path) = std::env::var(CLOUD_SESSION_FILE_ENV) {
        let trimmed = override_path.trim();
        if !trimmed.is_empty() {
            paths.push(PathBuf::from(trimmed));
        }
    }
    for env_name in ["LATTICE_HOME", "LATTICE_DEV_HOME"] {
        if let Ok(raw) = std::env::var(env_name) {
            let trimmed = raw.trim();
            if !trimmed.is_empty() {
                paths.push(session_file_under(Path::new(trimmed)));
            }
        }
    }
    if let Some(profile) = user_profile_dir() {
        paths.push(session_file_under(&profile.join(PROD_LATTICE_HOME_NAME)));
    }
    if cfg!(debug_assertions) {
        if let Ok(cwd) = std::env::current_dir() {
            paths.push(session_file_under(&cwd.join(DEBUG_HOME_RELATIVE)));
        }
    }
    paths.sort();
    paths.dedup();
    paths
}

fn load_shared_session_file() -> Option<String> {
    let paths = if let Ok(override_path) = std::env::var(CLOUD_SESSION_FILE_ENV) {
        let trimmed = override_path.trim();
        if trimmed.is_empty() {
            shared_session_file_candidates()
        } else {
            vec![PathBuf::from(trimmed)]
        }
    } else {
        shared_session_file_candidates()
    };
    for path in paths {
        let Ok(raw) = std::fs::read_to_string(&path) else {
            continue;
        };
        let token = raw.trim();
        if token.is_empty() || token.contains('\n') || token.len() > 16_384 {
            continue;
        }
        return Some(token.to_string());
    }
    None
}

fn save_paths_for_write() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(override_path) = std::env::var(CLOUD_SESSION_FILE_ENV) {
        let trimmed = override_path.trim();
        if !trimmed.is_empty() {
            paths.push(PathBuf::from(trimmed));
            return paths;
        }
    }
    for env_name in ["LATTICE_HOME", "LATTICE_DEV_HOME"] {
        if let Ok(raw) = std::env::var(env_name) {
            let trimmed = raw.trim();
            if !trimmed.is_empty() {
                paths.push(session_file_under(Path::new(trimmed)));
            }
        }
    }
    if let Some(profile) = user_profile_dir() {
        paths.push(session_file_under(&profile.join(PROD_LATTICE_HOME_NAME)));
    }
    if cfg!(debug_assertions) {
        if let Ok(cwd) = std::env::current_dir() {
            paths.push(session_file_under(&cwd.join(DEBUG_HOME_RELATIVE)));
        }
    }
    paths.sort();
    paths.dedup();
    paths
}

fn write_session_file(path: &Path, token: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| {
            CloudError::Credentials(format!("cloud session file {path:?}: {err}"))
        })?;
    }
    std::fs::write(path, token)
        .map_err(|err| CloudError::Credentials(format!("cloud session file {path:?}: {err}")))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn save_shared_session_file(token: &str) -> Result<()> {
    let paths = save_paths_for_write();
    if paths.is_empty() {
        return Err(CloudError::Credentials(
            "could not determine a Lattice home for the cloud session file".into(),
        ));
    }
    let mut last_err = None;
    let mut wrote = false;
    for path in paths {
        match write_session_file(&path, token) {
            Ok(()) => wrote = true,
            Err(err) => last_err = Some(err),
        }
    }
    if wrote {
        Ok(())
    } else {
        Err(last_err.unwrap_or_else(|| {
            CloudError::Credentials("could not write cloud session file".into())
        }))
    }
}

fn clear_shared_session_file() -> Result<()> {
    for path in save_paths_for_write() {
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                return Err(CloudError::Credentials(format!(
                    "cloud session file {path:?}: {err}"
                )));
            }
        }
    }
    Ok(())
}

pub fn cloud_session_status<C: CloudHttpClient>(
    client: &CloudApiClient<C>,
    store: &dyn CloudSessionStore,
) -> Result<CloudSessionStatus> {
    let base = client.base_url().to_string();
    let Some(token) = store.load_token()? else {
        return Ok(CloudSessionStatus::signed_out(base));
    };
    match client.me(&token) {
        Ok(me) => Ok(CloudSessionStatus::signed_in_with_entitlements(
            base,
            me.user,
            me.entitlements,
            me.preferences,
        )),
        Err(err) if err.api_status() == Some(401) => {
            let _ = store.clear_token();
            Ok(CloudSessionStatus {
                signed_in: false,
                cloud_url: base,
                user: None,
                entitlements: None,
                preferences: None,
                error: Some(err.to_string()),
            })
        }
        // Keep local session when /v1/me is unreachable; otherwise Settings remount
        // flashes "Sign in with Apple" despite a valid keychain token.
        Err(err) => Ok(CloudSessionStatus {
            signed_in: true,
            cloud_url: base,
            user: None,
            entitlements: None,
            preferences: None,
            error: Some(format!("could not refresh cloud session: {err}")),
        }),
    }
}

pub fn sign_in<C: CloudHttpClient>(
    client: &CloudApiClient<C>,
    store: &dyn CloudSessionStore,
    email: &str,
    password: &str,
) -> Result<CloudSessionStatus> {
    let response = client.password_login(email, password)?;
    store.save_token(&response.token)?;
    Ok(CloudSessionStatus::signed_in(
        client.base_url().to_string(),
        response.user,
    ))
}

/// Complete Sign in with Apple using a native (or web) identity token.
pub fn sign_in_with_apple<C: CloudHttpClient>(
    client: &CloudApiClient<C>,
    store: &dyn CloudSessionStore,
    id_token: &str,
    nonce: Option<&str>,
    user: Option<&str>,
) -> Result<CloudSessionStatus> {
    let response = client.apple_oauth(id_token, nonce, user)?;
    store.save_token(&response.token)?;
    Ok(CloudSessionStatus::signed_in(
        client.base_url().to_string(),
        response.user,
    ))
}

/// Complete browser SIWA handoff: exchange one-time code for a desktop bearer.
pub fn sign_in_with_desktop_handoff<C: CloudHttpClient>(
    client: &CloudApiClient<C>,
    store: &dyn CloudSessionStore,
    code: &str,
) -> Result<CloudSessionStatus> {
    let response = client.desktop_exchange(code, "lattice://oauth/cloud/callback")?;
    store.save_token(&response.token)?;
    Ok(CloudSessionStatus::signed_in(
        client.base_url().to_string(),
        response.user,
    ))
}

/// Bearer for cloud API calls: `LATTICE_CLOUD_TOKEN` wins, else the given store
/// (keychain and/or owner-local session file).
pub fn resolve_cloud_bearer(store: &dyn CloudSessionStore) -> Result<String> {
    if let Some(token) = crate::config::cloud_token_from_env() {
        return Ok(token);
    }
    if let Some(token) = store.load_token()? {
        return Ok(token);
    }
    load_shared_session_file().ok_or_else(|| {
        CloudError::Credentials(
            "not signed in to cloud; sign in via desktop Settings → Cloud account, \
             or set LATTICE_CLOUD_TOKEN"
                .into(),
        )
    })
}

pub fn sign_out<C: CloudHttpClient>(
    client: &CloudApiClient<C>,
    store: &dyn CloudSessionStore,
) -> Result<CloudSessionStatus> {
    let base = client.base_url().to_string();
    if let Some(token) = store.load_token()? {
        if let Err(err) = client.logout(&token) {
            // Best-effort remote revoke; always clear local credentials.
            if err.api_status() != Some(401) {
                let _ = store.clear_token();
                return Ok(CloudSessionStatus {
                    signed_in: false,
                    cloud_url: base,
                    user: None,
                    entitlements: None,
                    preferences: None,
                    error: Some(err.to_string()),
                });
            }
        }
    }
    store.clear_token()?;
    Ok(CloudSessionStatus::signed_out(base))
}

pub fn resolved_cloud_url() -> String {
    cloud_url()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{CloudHttpClient, CloudHttpResponse};

    #[derive(Default)]
    struct EmptyHttp;

    impl CloudHttpClient for EmptyHttp {
        fn request(
            &self,
            _base_url: &str,
            _method: &str,
            _path: &str,
            _body: Option<&serde_json::Value>,
            _bearer: Option<&str>,
        ) -> Result<CloudHttpResponse> {
            Err(CloudError::Http("unused".into()))
        }

        fn request_bytes(
            &self,
            _base_url: &str,
            _method: &str,
            _path: &str,
            _body: Option<&[u8]>,
            _bearer: Option<&str>,
            _headers: &[(&str, &str)],
        ) -> Result<crate::client::CloudHttpBytesResponse> {
            Err(CloudError::Http("unused".into()))
        }
    }

    #[test]
    fn memory_store_round_trip() {
        let store = MemoryCloudSessionStore::new();
        assert!(store.load_token().unwrap().is_none());
        store.save_token("abc").unwrap();
        assert_eq!(store.load_token().unwrap().as_deref(), Some("abc"));
        store.clear_token().unwrap();
        assert!(store.load_token().unwrap().is_none());
    }

    #[test]
    fn keychain_store_uses_cloud_service_constants() {
        assert_eq!(CLOUD_TOKEN_SERVICE, "lattice.cloud");
        assert_eq!(CLOUD_USER_TOKEN_KEY, "lattice.cloud.user");
        assert_eq!(CLOUD_PROBE_KEY, "lattice.cloud.probe");
        assert_ne!(CLOUD_PROBE_KEY, CLOUD_USER_TOKEN_KEY);
        let _store = KeychainCloudSessionStore::new();
    }

    #[test]
    fn signed_out_without_token() {
        let client = CloudApiClient::with_base_url(EmptyHttp, "https://cloud.test");
        let store = MemoryCloudSessionStore::new();
        let status = cloud_session_status(&client, &store).unwrap();
        assert!(!status.signed_in);
        assert_eq!(status.cloud_url, "https://cloud.test");
    }

    #[test]
    fn token_kept_when_me_unreachable() {
        let client = CloudApiClient::with_base_url(EmptyHttp, "https://cloud.test");
        let store = MemoryCloudSessionStore::new();
        store.save_token("bearer-token").unwrap();
        let status = cloud_session_status(&client, &store).unwrap();
        assert!(status.signed_in);
        assert!(status
            .error
            .as_deref()
            .unwrap_or("")
            .contains("could not refresh"));
        assert_eq!(store.load_token().unwrap().as_deref(), Some("bearer-token"));
    }

    #[test]
    fn file_store_round_trip_via_env_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cloud-session");
        std::env::set_var(CLOUD_SESSION_FILE_ENV, &path);
        let store = FileCloudSessionStore;
        store.save_token("file-token").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap().trim(), "file-token");
        assert_eq!(store.load_token().unwrap().as_deref(), Some("file-token"));
        store.clear_token().unwrap();
        assert!(store.load_token().unwrap().is_none());
        std::env::remove_var(CLOUD_SESSION_FILE_ENV);
    }
}
