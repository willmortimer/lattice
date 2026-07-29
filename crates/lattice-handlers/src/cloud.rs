//! Cloud account bearer auth handlers (ADR 0067).

use std::sync::OnceLock;

use lattice_cloud_client::{
    CloudApiClient, CloudSessionStatus, CloudSessionStore, HttpCloudClient,
    KeychainCloudSessionStore, MemoryCloudSessionStore, cloud_session_status, default_client,
    sign_in, sign_in_with_apple, sign_out,
};

fn session_store() -> &'static dyn CloudSessionStore {
    static KEYCHAIN: OnceLock<KeychainCloudSessionStore> = OnceLock::new();
    static MEMORY: OnceLock<MemoryCloudSessionStore> = OnceLock::new();
    static USE_MEMORY: OnceLock<bool> = OnceLock::new();

    let use_memory = *USE_MEMORY.get_or_init(|| {
        let store = KeychainCloudSessionStore::new();
        match store.save_token("probe") {
            Ok(()) => {
                let _ = store.clear_token();
                false
            }
            Err(_) => true,
        }
    });
    if use_memory {
        MEMORY.get_or_init(MemoryCloudSessionStore::new)
    } else {
        KEYCHAIN.get_or_init(KeychainCloudSessionStore::new)
    }
}

fn api_client() -> CloudApiClient<HttpCloudClient> {
    default_client()
}

fn map_err(err: impl std::fmt::Display) -> String {
    err.to_string()
}

pub fn cloud_session_status_cmd() -> Result<CloudSessionStatus, String> {
    cloud_session_status(&api_client(), session_store()).map_err(map_err)
}

pub fn cloud_sign_in(email: String, password: String) -> Result<CloudSessionStatus, String> {
    sign_in(
        &api_client(),
        session_store(),
        email.trim(),
        &password,
    )
    .map_err(map_err)
}

/// Native Sign in with Apple: obtain identity token on macOS, then mint a bearer session.
pub fn cloud_sign_in_apple() -> Result<CloudSessionStatus, String> {
    #[cfg(target_os = "macos")]
    {
        let nonce = random_nonce();
        let id_token = lattice_apple_signin_macos::request_identity_token(Some(&nonce))?;
        return sign_in_with_apple(
            &api_client(),
            session_store(),
            &id_token,
            Some(&nonce),
            None,
        )
        .map_err(map_err);
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("Sign in with Apple is only available on macOS".into())
    }
}

fn random_nonce() -> String {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).expect("CSPRNG for SIWA nonce");
    format!("lattice-desktop-{}", hex::encode(bytes))
}

pub fn cloud_sign_out() -> Result<CloudSessionStatus, String> {
    sign_out(&api_client(), session_store()).map_err(map_err)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use lattice_cloud_client::{
        CloudApiClient, CloudHttpClient, CloudHttpResponse, CloudError, MemoryCloudSessionStore,
        cloud_session_status, sign_in, sign_out,
    };

    #[derive(Default, Clone)]
    struct FakeHttp {
        responses: Arc<Mutex<HashMap<String, CloudHttpResponse>>>,
    }

    impl FakeHttp {
        fn insert(&self, method: &str, path: &str, response: CloudHttpResponse) {
            self.responses
                .lock()
                .unwrap()
                .insert(format!("{method} {path}"), response);
        }
    }

    impl CloudHttpClient for FakeHttp {
        fn request(
            &self,
            _base_url: &str,
            method: &str,
            path: &str,
            _body: Option<&serde_json::Value>,
            bearer: Option<&str>,
        ) -> Result<CloudHttpResponse, CloudError> {
            let key = format!("{method} {path}");
            if let Some(response) = self.responses.lock().unwrap().get(&key).cloned() {
                if path == "/v1/me" && bearer != Some("tok") {
                    return Ok(CloudHttpResponse {
                        status: 401,
                        body: r#"{"error":"invalid session"}"#.into(),
                    });
                }
                return Ok(response);
            }
            Err(CloudError::Http(format!("no fake response for {key}")))
        }

        fn request_bytes(
            &self,
            _base_url: &str,
            method: &str,
            path: &str,
            _body: Option<&[u8]>,
            _bearer: Option<&str>,
            _headers: &[(&str, &str)],
        ) -> Result<lattice_cloud_client::CloudHttpBytesResponse, CloudError> {
            Err(CloudError::Http(format!(
                "no fake bytes response for {method} {path}"
            )))
        }
    }

    #[test]
    fn handler_sign_in_and_out() {
        let http = FakeHttp::default();
        http.insert(
            "POST",
            "/v1/auth/password/login",
            CloudHttpResponse {
                status: 200,
                body: r#"{
                    "token": "tok",
                    "user": {
                        "id": "u1",
                        "username": "alice",
                        "display_name": "Alice",
                        "email": "alice@example.com",
                        "created_at": 1
                    }
                }"#
                .into(),
            },
        );
        http.insert(
            "GET",
            "/v1/me",
            CloudHttpResponse {
                status: 200,
                body: r#"{
                    "user": {
                        "id": "u1",
                        "username": "alice",
                        "display_name": "Alice",
                        "email": "alice@example.com",
                        "created_at": 1
                    },
                    "devices": [],
                    "identities": []
                }"#
                .into(),
            },
        );
        http.insert(
            "POST",
            "/v1/auth/logout",
            CloudHttpResponse {
                status: 204,
                body: String::new(),
            },
        );

        let client = CloudApiClient::with_base_url(http, "https://cloud.test");
        let store = MemoryCloudSessionStore::new();
        let signed_in = sign_in(&client, &store, "alice@example.com", "secret").unwrap();
        assert!(signed_in.signed_in);
        let status = cloud_session_status(&client, &store).unwrap();
        assert_eq!(status.user.as_ref().map(|user| user.email.as_deref()), Some(Some("alice@example.com")));
        let signed_out = sign_out(&client, &store).unwrap();
        assert!(!signed_out.signed_in);
    }
}
