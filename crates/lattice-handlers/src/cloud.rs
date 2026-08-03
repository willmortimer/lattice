//! Cloud account bearer auth handlers (ADR 0067).

use std::sync::OnceLock;

use lattice_connectors::probe_token_store_writable;

use lattice_cloud_client::{
    resolve_cloud_bearer, CloudApiClient, CloudSessionStatus, CloudSessionStore, HttpCloudClient,
    KeychainCloudSessionStore, MemoryCloudSessionStore, PreferencesView, cloud_session_status,
    default_client, sign_in, sign_in_with_apple, sign_in_with_desktop_handoff, sign_out,
    CLOUD_PROBE_KEY, CLOUD_TOKEN_SERVICE,
};

fn session_store() -> &'static dyn CloudSessionStore {
    static KEYCHAIN: OnceLock<KeychainCloudSessionStore> = OnceLock::new();
    static MEMORY: OnceLock<MemoryCloudSessionStore> = OnceLock::new();
    static USE_MEMORY: OnceLock<bool> = OnceLock::new();

    let use_memory = *USE_MEMORY.get_or_init(|| {
        !probe_token_store_writable(CLOUD_TOKEN_SERVICE, CLOUD_PROBE_KEY)
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

/// Bearer for agent spawn when Lattice paid mode is active (never log the return value).
pub fn resolve_cloud_bearer_cmd() -> Result<String, String> {
    resolve_cloud_bearer(session_store()).map_err(map_err)
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
        Err("Native Sign in with Apple is only available on macOS; use browser SIWA".into())
    }
}

/// Begin browser SIWA for public / Windows / Developer ID builds.
///
/// Returns the URL the desktop shell should open in the system browser. The
/// expected `state` is stored until [`cloud_complete_desktop_handoff`] runs.
pub fn cloud_begin_browser_siwa(app_base_url: Option<String>) -> Result<String, String> {
    let state = random_nonce();
    store_pending_desktop_state(state.clone())?;
    let base = app_base_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("https://app.lattice-notes.com")
        .trim_end_matches('/');
    Ok(format!("{base}/auth/desktop?state={state}"))
}

/// Finish browser SIWA after `lattice://oauth/cloud/callback?code=&state=`.
pub fn cloud_complete_desktop_handoff(
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
) -> Result<CloudSessionStatus, String> {
    if let Some(message) = error.map(|value| value.trim().to_string()).filter(|value| !value.is_empty())
    {
        let _ = take_pending_desktop_state();
        return Err(message);
    }
    let expected = take_pending_desktop_state()?
        .ok_or_else(|| "no pending desktop cloud sign-in; start browser SIWA again".to_string())?;
    let provided = state
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "desktop cloud callback missing state".to_string())?;
    if provided != expected {
        return Err("desktop cloud sign-in state mismatch; try again".into());
    }
    let code = code
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "desktop cloud callback missing handoff code".to_string())?;
    sign_in_with_desktop_handoff(&api_client(), session_store(), code).map_err(map_err)
}

fn pending_desktop_state() -> &'static std::sync::Mutex<Option<String>> {
    static PENDING: OnceLock<std::sync::Mutex<Option<String>>> = OnceLock::new();
    PENDING.get_or_init(|| std::sync::Mutex::new(None))
}

fn store_pending_desktop_state(state: String) -> Result<(), String> {
    *pending_desktop_state()
        .lock()
        .map_err(|_| "desktop cloud state lock poisoned".to_string())? = Some(state);
    Ok(())
}

fn take_pending_desktop_state() -> Result<Option<String>, String> {
    Ok(pending_desktop_state()
        .lock()
        .map_err(|_| "desktop cloud state lock poisoned".to_string())?
        .take())
}

fn random_nonce() -> String {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).expect("CSPRNG for SIWA nonce");
    format!("lattice-desktop-{}", hex::encode(bytes))
}

pub fn cloud_sign_out() -> Result<CloudSessionStatus, String> {
    sign_out(&api_client(), session_store()).map_err(map_err)
}

pub fn cloud_update_preferences(
    ai_audit_enabled: Option<bool>,
    anonymous_telemetry_enabled: Option<bool>,
) -> Result<PreferencesView, String> {
    let token = resolve_cloud_bearer(session_store()).map_err(map_err)?;
    api_client()
        .update_preferences(&token, ai_audit_enabled, anonymous_telemetry_enabled)
        .map_err(map_err)
}

/// Best-effort coarse product telemetry (caller enforces local consent).
pub fn product_telemetry_emit(
    name: String,
    properties: Option<serde_json::Value>,
    anonymous_telemetry_enabled: bool,
) -> Result<(), String> {
    if !anonymous_telemetry_enabled {
        return Ok(());
    }
    let name = name.trim();
    const ALLOWED: &[&str] = &[
        "app_launch",
        "settings_opened",
        "agent_panel_opened",
        "agent_run_completed",
    ];
    if !ALLOWED.contains(&name) {
        return Err(format!("unsupported telemetry event '{name}'"));
    }
    let install_id = load_or_create_install_id()?;
    let bearer = session_store().load_token().ok().flatten();
    let props = sanitize_telemetry_properties(properties);
    let _ = api_client().post_telemetry_events(
        bearer.as_deref(),
        &install_id,
        &[(name, props)],
    );
    Ok(())
}

fn sanitize_telemetry_properties(
    properties: Option<serde_json::Value>,
) -> Option<serde_json::Value> {
    let Some(serde_json::Value::Object(map)) = properties else {
        return None;
    };
    let forbidden = [
        "path", "paths", "prompt", "prompts", "excerpt", "excerpts", "filename", "filenames",
        "content", "body", "text", "message", "messages",
    ];
    let mut out = serde_json::Map::new();
    for (key, value) in map {
        let normalized = key.to_ascii_lowercase();
        if forbidden.iter().any(|item| normalized == *item || normalized.contains(item)) {
            continue;
        }
        match value {
            serde_json::Value::Null
            | serde_json::Value::Bool(_)
            | serde_json::Value::Number(_) => {
                out.insert(key, value);
            }
            serde_json::Value::String(ref s) if s.len() <= 64 => {
                out.insert(key, value);
            }
            _ => {}
        }
        if out.len() >= 16 {
            break;
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(out))
    }
}

fn load_or_create_install_id() -> Result<String, String> {
    use std::fs;
    use std::sync::OnceLock;
    static CACHED: OnceLock<String> = OnceLock::new();
    if let Some(id) = CACHED.get() {
        return Ok(id.clone());
    }
    let home = std::env::var("HOME").map_err(|_| "HOME unset".to_string())?;
    let path = std::path::PathBuf::from(home).join(".lattice").join("install_id");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let id = if path.is_file() {
        let existing = fs::read_to_string(&path).map_err(|err| err.to_string())?;
        let trimmed = existing.trim();
        if trimmed.is_empty() {
            let fresh = format!("install-{}", hex::encode(random_16()));
            fs::write(&path, &fresh).map_err(|err| err.to_string())?;
            fresh
        } else {
            trimmed.to_string()
        }
    } else {
        let fresh = format!("install-{}", hex::encode(random_16()));
        fs::write(&path, &fresh).map_err(|err| err.to_string())?;
        fresh
    };
    let _ = CACHED.set(id.clone());
    Ok(id)
}

fn random_16() -> [u8; 16] {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).expect("CSPRNG for install id");
    bytes
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
