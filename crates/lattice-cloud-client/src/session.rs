use std::sync::{Arc, Mutex};

use lattice_connectors::{production_token_store, TokenMaterial, TokenStore};

use crate::client::{CloudApiClient, CloudHttpClient};
use crate::config::cloud_url;
use crate::error::{CloudError, Result};
use crate::types::CloudSessionStatus;

/// OS keychain service name for the lattice-server bearer session.
pub const CLOUD_TOKEN_SERVICE: &str = "lattice.cloud";
/// Keychain account for the desktop cloud bearer token.
pub const CLOUD_USER_TOKEN_KEY: &str = "lattice.cloud.user";

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
        self.store
            .get(CLOUD_USER_TOKEN_KEY)
            .map(|material| material.map(|token| token.access_token))
            .map_err(|err| CloudError::Credentials(err.to_string()))
    }

    fn save_token(&self, token: &str) -> Result<()> {
        self.store
            .set(
                CLOUD_USER_TOKEN_KEY,
                &TokenMaterial {
                    access_token: token.to_string(),
                    refresh_token: None,
                    expires_in: None,
                    token_type: Some("bearer".into()),
                },
            )
            .map_err(|err| CloudError::Credentials(err.to_string()))
    }

    fn clear_token(&self) -> Result<()> {
        self.store
            .delete(CLOUD_USER_TOKEN_KEY)
            .map_err(|err| CloudError::Credentials(err.to_string()))
    }
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

/// Bearer for cloud API calls: `LATTICE_CLOUD_TOKEN` wins, else keychain/session store.
pub fn resolve_cloud_bearer(store: &dyn CloudSessionStore) -> Result<String> {
    if let Some(token) = crate::config::cloud_token_from_env() {
        return Ok(token);
    }
    store.load_token()?.ok_or_else(|| {
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
        assert!(status.error.as_deref().unwrap_or("").contains("could not refresh"));
        assert_eq!(store.load_token().unwrap().as_deref(), Some("bearer-token"));
    }
}
