use serde::de::DeserializeOwned;
use serde_json::Value;

use latticefs_core::{ContentHash, ResourceId};

use crate::config::cloud_url;
use crate::error::{CloudError, Result};
use crate::types::{
    AuthTokenResponse, BackupMetadataResponse, BackupWrapKeyResponse, CloudWorkspaceRecord,
    MeResponse, PreferencesView, WorkspaceSyncHead,
};

pub const CONTENT_HASH_HEADER: &str = "x-lattice-content-hash";
pub const DEVICE_ID_HEADER: &str = "x-lattice-device-id";
pub const WORKSPACE_ID_HEADER: &str = "x-lattice-workspace-id";
pub const IF_MATCH_HEADER: &str = "if-match";

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct BlobPutResponse {
    pub resource_id: String,
    pub object_key: String,
    pub size: u64,
    pub content_hash: String,
    pub created_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudHttpResponse {
    pub status: u16,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudHttpBytesResponse {
    pub status: u16,
    pub body: Vec<u8>,
    pub content_hash: Option<String>,
}

/// Pluggable HTTP surface for unit tests and the production ureq client.
pub trait CloudHttpClient: Send + Sync {
    fn request(
        &self,
        base_url: &str,
        method: &str,
        path: &str,
        body: Option<&Value>,
        bearer: Option<&str>,
    ) -> Result<CloudHttpResponse>;

    fn request_bytes(
        &self,
        base_url: &str,
        method: &str,
        path: &str,
        body: Option<&[u8]>,
        bearer: Option<&str>,
        headers: &[(&str, &str)],
    ) -> Result<CloudHttpBytesResponse>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct HttpCloudClient;

impl CloudHttpClient for HttpCloudClient {
    fn request(
        &self,
        base_url: &str,
        method: &str,
        path: &str,
        body: Option<&Value>,
        bearer: Option<&str>,
    ) -> Result<CloudHttpResponse> {
        let url = format!("{base_url}{path}");
        let mut request = match method {
            "GET" => ureq::get(&url),
            "POST" => ureq::post(&url),
            "PUT" => ureq::put(&url),
            other => {
                return Err(CloudError::Http(format!("unsupported method {other}")));
            }
        };
        request = request.set("Accept", "application/json");
        if let Some(token) = bearer {
            request = request.set("Authorization", &format!("Bearer {token}"));
        }
        let response = if let Some(payload) = body {
            let json =
                serde_json::to_string(payload).map_err(|err| CloudError::Http(err.to_string()))?;
            request
                .set("Content-Type", "application/json")
                .send_string(&json)
        } else {
            request.call()
        }
        .map_err(|err| CloudError::Http(err.to_string()))?;
        let status = response.status();
        let response_body = response
            .into_string()
            .map_err(|err| CloudError::Http(err.to_string()))?;
        Ok(CloudHttpResponse {
            status,
            body: response_body,
        })
    }

    fn request_bytes(
        &self,
        base_url: &str,
        method: &str,
        path: &str,
        body: Option<&[u8]>,
        bearer: Option<&str>,
        headers: &[(&str, &str)],
    ) -> Result<CloudHttpBytesResponse> {
        let url = format!("{base_url}{path}");
        let mut request = match method {
            "GET" => ureq::get(&url),
            "PUT" => ureq::put(&url),
            other => {
                return Err(CloudError::Http(format!("unsupported method {other}")));
            }
        };
        if let Some(token) = bearer {
            request = request.set("Authorization", &format!("Bearer {token}"));
        }
        for (name, value) in headers {
            request = request.set(*name, *value);
        }
        let response = if let Some(payload) = body {
            request.send_bytes(payload)
        } else {
            request.call()
        }
        .map_err(|err| CloudError::Http(err.to_string()))?;
        let status = response.status();
        let content_hash = response
            .header("X-Lattice-Content-Hash")
            .map(str::to_string);
        let mut reader = response.into_reader();
        let mut response_body = Vec::new();
        std::io::Read::read_to_end(&mut reader, &mut response_body)
            .map_err(|err| CloudError::Http(err.to_string()))?;
        Ok(CloudHttpBytesResponse {
            status,
            body: response_body,
            content_hash,
        })
    }
}

pub struct CloudApiClient<C: CloudHttpClient> {
    http: C,
    base_url: String,
}

impl<C: CloudHttpClient> CloudApiClient<C> {
    pub fn new(http: C) -> Self {
        Self {
            http,
            base_url: cloud_url(),
        }
    }

    pub fn with_base_url(http: C, base_url: impl Into<String>) -> Self {
        Self {
            http,
            base_url: base_url.into().trim_end_matches('/').to_string(),
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn password_login(&self, email: &str, password: &str) -> Result<AuthTokenResponse> {
        let body = serde_json::json!({
            "email": email,
            "password": password,
        });
        self.post_json("/v1/auth/password/login", Some(&body), None)
    }

    /// Native or web Sign in with Apple identity token → lattice-server session.
    pub fn apple_oauth(
        &self,
        id_token: &str,
        nonce: Option<&str>,
        user: Option<&str>,
    ) -> Result<AuthTokenResponse> {
        let body = serde_json::json!({
            "id_token": id_token,
            "nonce": nonce,
            "user": user,
        });
        self.post_json("/v1/auth/oauth/apple", Some(&body), None)
    }

    /// Exchange a one-time desktop handoff code for a bearer session.
    pub fn desktop_exchange(&self, code: &str, redirect_uri: &str) -> Result<AuthTokenResponse> {
        let body = serde_json::json!({
            "code": code,
            "redirect_uri": redirect_uri,
        });
        self.post_json("/v1/auth/desktop/exchange", Some(&body), None)
    }

    pub fn password_register(
        &self,
        email: &str,
        password: &str,
        bootstrap_token: Option<&str>,
    ) -> Result<AuthTokenResponse> {
        let body = serde_json::json!({
            "email": email,
            "password": password,
            "bootstrap_token": bootstrap_token,
        });
        self.post_json("/v1/auth/password/register", Some(&body), None)
    }

    pub fn logout(&self, bearer: &str) -> Result<()> {
        let response = self.http.request(
            &self.base_url,
            "POST",
            "/v1/auth/logout",
            None,
            Some(bearer),
        )?;
        if response.status == 204 || response.status == 200 {
            return Ok(());
        }
        Err(api_error(response.status, &response.body))
    }

    pub fn me(&self, bearer: &str) -> Result<MeResponse> {
        self.get_json("/v1/me", Some(bearer))
    }

    /// Fetch (or create) the account backup wrap key. Idempotent GET.
    pub fn get_backup_wrap_key(&self, bearer: &str) -> Result<BackupWrapKeyResponse> {
        self.get_json("/v1/me/backup-wrap-key", Some(bearer))
    }

    /// Patch account consent flags; omitted fields keep their server-side value.
    pub fn update_preferences(
        &self,
        bearer: &str,
        ai_audit_enabled: Option<bool>,
        anonymous_telemetry_enabled: Option<bool>,
    ) -> Result<PreferencesView> {
        let mut body = serde_json::Map::new();
        if let Some(value) = ai_audit_enabled {
            body.insert("ai_audit_enabled".into(), Value::Bool(value));
        }
        if let Some(value) = anonymous_telemetry_enabled {
            body.insert("anonymous_telemetry_enabled".into(), Value::Bool(value));
        }
        if body.is_empty() {
            return Err(CloudError::Http(
                "at least one preference field is required".into(),
            ));
        }
        self.put_json(
            "/v1/me/preferences",
            Some(&Value::Object(body)),
            Some(bearer),
        )
    }

    /// Best-effort anonymous product telemetry batch. The bearer is optional:
    /// signed-out installs still report coarse events under `install_id`.
    pub fn post_telemetry_events(
        &self,
        bearer: Option<&str>,
        install_id: &str,
        events: &[(&str, Option<Value>)],
    ) -> Result<()> {
        let payload = serde_json::json!({
            "install_id": install_id,
            "events": events
                .iter()
                .map(|(name, properties)| {
                    serde_json::json!({ "name": name, "properties": properties })
                })
                .collect::<Vec<_>>(),
        });
        let response = self.http.request(
            &self.base_url,
            "POST",
            "/v1/telemetry/events",
            Some(&payload),
            bearer,
        )?;
        if (200..300).contains(&response.status) {
            return Ok(());
        }
        Err(api_error(response.status, &response.body))
    }

    pub fn put_blob(
        &self,
        bearer: &str,
        resource_id: ResourceId,
        data: &[u8],
    ) -> Result<ContentHash> {
        self.put_workspace_blob(bearer, None, resource_id, data, None)
    }

    /// `PUT /v1/blobs/{resource_id}` with optional workspace sync-head and `If-Match`.
    pub fn put_workspace_blob(
        &self,
        bearer: &str,
        workspace_id: Option<&str>,
        resource_id: ResourceId,
        data: &[u8],
        if_match: Option<&str>,
    ) -> Result<ContentHash> {
        let hash =
            ContentHash::from_bytes(data).map_err(|err| CloudError::Http(err.to_string()))?;
        let hash_hex = content_hash_hex(&hash);
        let path = format!("/v1/blobs/{resource_id}");
        let mut headers = vec![
            ("Content-Type", "application/octet-stream"),
            ("X-Lattice-Content-Hash", hash_hex),
        ];
        let workspace_header;
        if let Some(workspace_id) = workspace_id.map(str::trim).filter(|id| !id.is_empty()) {
            workspace_header = workspace_id.to_string();
            headers.push((WORKSPACE_ID_HEADER, workspace_header.as_str()));
        }
        let if_match_header;
        if let Some(expected) = if_match.map(str::trim).filter(|hash| !hash.is_empty()) {
            if_match_header = normalize_if_match(expected);
            headers.push((IF_MATCH_HEADER, if_match_header.as_str()));
        }
        let response = self.http.request_bytes(
            &self.base_url,
            "PUT",
            &path,
            Some(data),
            Some(bearer),
            &headers,
        )?;
        if response.status == 201 || response.status == 200 {
            return parse_blob_put_response(&response.body, hash_hex);
        }
        Err(bytes_api_error(response))
    }

    /// `GET /v1/workspaces/{id}/sync-heads` for planner input.
    pub fn get_sync_heads(
        &self,
        bearer: &str,
        workspace_id: &str,
    ) -> Result<Vec<WorkspaceSyncHead>> {
        let path = format!("/v1/workspaces/{workspace_id}/sync-heads");
        self.get_json(&path, Some(bearer))
    }

    pub fn create_workspace(
        &self,
        bearer: &str,
        name: &str,
        local_workspace_id: Option<&str>,
    ) -> Result<CloudWorkspaceRecord> {
        let body = serde_json::json!({
            "name": name,
            "local_workspace_id": local_workspace_id,
        });
        self.post_json("/v1/workspaces", Some(&body), Some(bearer))
    }

    pub fn list_workspaces(&self, bearer: &str) -> Result<Vec<CloudWorkspaceRecord>> {
        self.get_json("/v1/workspaces", Some(bearer))
    }

    /// Store opaque client-encrypted backup ciphertext for an owned workspace.
    pub fn put_workspace_backup(
        &self,
        bearer: &str,
        workspace_id: &str,
        ciphertext: &[u8],
        device_id: Option<&str>,
    ) -> Result<BackupMetadataResponse> {
        let hash =
            ContentHash::from_bytes(ciphertext).map_err(|err| CloudError::Http(err.to_string()))?;
        let hash_hex = content_hash_hex(&hash);
        let path = format!("/v1/workspaces/{workspace_id}/backups");
        let mut headers = vec![
            ("Content-Type", "application/octet-stream"),
            (CONTENT_HASH_HEADER, hash_hex),
        ];
        let device_header;
        if let Some(device_id) = device_id.map(str::trim).filter(|id| !id.is_empty()) {
            device_header = device_id.to_string();
            headers.push((DEVICE_ID_HEADER, device_header.as_str()));
        }
        let response = self.http.request_bytes(
            &self.base_url,
            "PUT",
            &path,
            Some(ciphertext),
            Some(bearer),
            &headers,
        )?;
        if response.status == 201 {
            return decode_json(
                response.status,
                std::str::from_utf8(&response.body)
                    .map_err(|err| CloudError::InvalidResponse(err.to_string()))?,
            );
        }
        Err(bytes_api_error(response))
    }

    pub fn list_workspace_backups(
        &self,
        bearer: &str,
        workspace_id: &str,
    ) -> Result<Vec<BackupMetadataResponse>> {
        let path = format!("/v1/workspaces/{workspace_id}/backups");
        self.get_json(&path, Some(bearer))
    }

    /// Fetch opaque backup ciphertext for an owned workspace.
    ///
    /// When the response includes `X-Lattice-Content-Hash`, verifies the body
    /// SHA-256 matches (same as [`Self::get_blob`]).
    pub fn get_workspace_backup(
        &self,
        bearer: &str,
        workspace_id: &str,
        backup_id: &str,
    ) -> Result<Vec<u8>> {
        let path = format!("/v1/workspaces/{workspace_id}/backups/{backup_id}");
        let response =
            self.http
                .request_bytes(&self.base_url, "GET", &path, None, Some(bearer), &[])?;
        if response.status == 200 {
            verify_response_content_hash(&response)?;
            return Ok(response.body);
        }
        Err(bytes_api_error(response))
    }

    pub fn get_blob(&self, bearer: &str, resource_id: ResourceId) -> Result<Vec<u8>> {
        let path = format!("/v1/blobs/{resource_id}");
        let response =
            self.http
                .request_bytes(&self.base_url, "GET", &path, None, Some(bearer), &[])?;
        if response.status == 200 {
            verify_response_content_hash(&response)?;
            return Ok(response.body);
        }
        Err(bytes_api_error(response))
    }

    fn post_json<T: DeserializeOwned>(
        &self,
        path: &str,
        body: Option<&Value>,
        bearer: Option<&str>,
    ) -> Result<T> {
        let response = self
            .http
            .request(&self.base_url, "POST", path, body, bearer)?;
        decode_json(response.status, &response.body)
    }

    fn put_json<T: DeserializeOwned>(
        &self,
        path: &str,
        body: Option<&Value>,
        bearer: Option<&str>,
    ) -> Result<T> {
        let response = self
            .http
            .request(&self.base_url, "PUT", path, body, bearer)?;
        decode_json(response.status, &response.body)
    }

    fn get_json<T: DeserializeOwned>(&self, path: &str, bearer: Option<&str>) -> Result<T> {
        let response = self
            .http
            .request(&self.base_url, "GET", path, None, bearer)?;
        decode_json(response.status, &response.body)
    }
}

pub type DefaultCloudApiClient = CloudApiClient<HttpCloudClient>;

pub fn default_client() -> DefaultCloudApiClient {
    CloudApiClient::new(HttpCloudClient)
}

fn decode_json<T: DeserializeOwned>(status: u16, body: &str) -> Result<T> {
    if !(200..300).contains(&status) {
        return Err(api_error(status, body));
    }
    if body.trim().is_empty() {
        return Err(CloudError::InvalidResponse("empty response body".into()));
    }
    serde_json::from_str(body).map_err(|err| CloudError::InvalidResponse(err.to_string()))
}

fn api_error(status: u16, body: &str) -> CloudError {
    let message = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|value| {
            value
                .get("error")
                .and_then(|error| error.as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| {
            if body.trim().is_empty() {
                format!("HTTP {status}")
            } else {
                body.to_string()
            }
        });
    CloudError::Api { status, message }
}

fn content_hash_hex(hash: &ContentHash) -> &str {
    hash.as_str()
        .strip_prefix("sha256:")
        .unwrap_or(hash.as_str())
}

fn verify_response_content_hash(response: &CloudHttpBytesResponse) -> Result<()> {
    let Some(header_hash) = response.content_hash.as_deref() else {
        return Ok(());
    };
    let body_hash = ContentHash::from_bytes(&response.body)
        .map_err(|err| CloudError::InvalidResponse(err.to_string()))?;
    let expected = ContentHash::new(format!("sha256:{header_hash}"))
        .map_err(|err| CloudError::InvalidResponse(err.to_string()))?;
    if body_hash != expected {
        return Err(CloudError::InvalidResponse(format!(
            "response hash mismatch: header {header_hash}, body {}",
            content_hash_hex(&body_hash)
        )));
    }
    Ok(())
}

fn normalize_if_match(raw: &str) -> String {
    let trimmed = raw.trim();
    let unquoted = trimmed
        .strip_prefix('"')
        .and_then(|inner| inner.strip_suffix('"'))
        .unwrap_or(trimmed);
    unquoted
        .strip_prefix("sha256:")
        .or_else(|| unquoted.strip_prefix("SHA256:"))
        .unwrap_or(unquoted)
        .to_ascii_lowercase()
}

fn parse_blob_put_response(body: &[u8], expected_hash_hex: &str) -> Result<ContentHash> {
    let body =
        std::str::from_utf8(body).map_err(|err| CloudError::InvalidResponse(err.to_string()))?;
    let metadata: BlobPutResponse =
        serde_json::from_str(body).map_err(|err| CloudError::InvalidResponse(err.to_string()))?;
    if metadata.content_hash != expected_hash_hex {
        return Err(CloudError::InvalidResponse(format!(
            "response hash mismatch: expected {expected_hash_hex}, got {}",
            metadata.content_hash
        )));
    }
    ContentHash::new(format!("sha256:{}", metadata.content_hash))
        .map_err(|err| CloudError::InvalidResponse(err.to_string()))
}

fn bytes_api_error(response: CloudHttpBytesResponse) -> CloudError {
    let message = std::str::from_utf8(&response.body)
        .ok()
        .and_then(|body| {
            serde_json::from_str::<Value>(body).ok().and_then(|value| {
                value
                    .get("error")
                    .and_then(|error| error.as_str())
                    .map(str::to_string)
            })
        })
        .unwrap_or_else(|| {
            if response.body.is_empty() {
                format!("HTTP {}", response.status)
            } else {
                String::from_utf8_lossy(&response.body).into_owned()
            }
        });
    CloudError::Api {
        status: response.status,
        message,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use super::*;

    #[derive(Default, Clone)]
    struct FakeCloudHttp {
        responses: Arc<Mutex<HashMap<String, CloudHttpResponse>>>,
        bytes_responses: Arc<Mutex<HashMap<String, CloudHttpBytesResponse>>>,
    }

    impl FakeCloudHttp {
        fn insert(&self, method: &str, path: &str, response: CloudHttpResponse) {
            self.responses
                .lock()
                .unwrap()
                .insert(format!("{method} {path}"), response);
        }
    }

    impl CloudHttpClient for FakeCloudHttp {
        fn request(
            &self,
            _base_url: &str,
            method: &str,
            path: &str,
            _body: Option<&Value>,
            bearer: Option<&str>,
        ) -> Result<CloudHttpResponse> {
            let key = format!("{method} {path}");
            if let Some(response) = self.responses.lock().unwrap().get(&key).cloned() {
                if path == "/v1/me" && bearer != Some("good-token") {
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
            bearer: Option<&str>,
            _headers: &[(&str, &str)],
        ) -> Result<CloudHttpBytesResponse> {
            let key = format!("{method} {path}");
            if bearer != Some("good-token") {
                return Ok(CloudHttpBytesResponse {
                    status: 401,
                    body: br#"{"error":"invalid session"}"#.to_vec(),
                    content_hash: None,
                });
            }
            self.bytes_responses
                .lock()
                .unwrap()
                .get(&key)
                .cloned()
                .ok_or_else(|| CloudError::Http(format!("no fake bytes response for {key}")))
        }
    }

    #[test]
    fn password_login_parses_token_response() {
        let http = FakeCloudHttp::default();
        http.insert(
            "POST",
            "/v1/auth/password/login",
            CloudHttpResponse {
                status: 200,
                body: r#"{
                    "token": "tok-1",
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
        let client = CloudApiClient::with_base_url(http, "https://cloud.test");
        let response = client
            .password_login("alice@example.com", "secret")
            .unwrap();
        assert_eq!(response.token, "tok-1");
        assert_eq!(response.user.email.as_deref(), Some("alice@example.com"));
    }

    #[test]
    fn me_rejects_unauthorized() {
        let http = FakeCloudHttp::default();
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
        let client = CloudApiClient::with_base_url(http, "https://cloud.test");
        let err = client.me("bad-token").unwrap_err();
        assert_eq!(err.api_status(), Some(401));
    }

    #[test]
    fn get_backup_wrap_key_parses_hex() {
        let http = FakeCloudHttp::default();
        http.insert(
            "GET",
            "/v1/me/backup-wrap-key",
            CloudHttpResponse {
                status: 200,
                body: r#"{"wrap_key":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#.into(),
            },
        );
        let client = CloudApiClient::with_base_url(http, "https://cloud.test");
        let response = client.get_backup_wrap_key("good-token").unwrap();
        assert_eq!(response.wrap_key.len(), 64);
    }

    #[test]
    fn session_status_round_trip_with_fake_http() {
        use crate::session::{cloud_session_status, sign_in, sign_out, MemoryCloudSessionStore};

        let http = FakeCloudHttp::default();
        http.insert(
            "POST",
            "/v1/auth/password/login",
            CloudHttpResponse {
                status: 200,
                body: r#"{
                    "token": "good-token",
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
        assert_eq!(
            status.user.as_ref().map(|user| user.id.as_str()),
            Some("u1")
        );
        let signed_out = sign_out(&client, &store).unwrap();
        assert!(!signed_out.signed_in);
        let status = cloud_session_status(&client, &store).unwrap();
        assert!(!status.signed_in);
    }

    #[test]
    fn put_blob_accepts_201_created() {
        let http = FakeCloudHttp::default();
        let resource_id = ResourceId::new();
        let data = b"opaque-cloud-bytes";
        let hash = ContentHash::from_bytes(data).unwrap();
        let hash_hex = content_hash_hex(&hash);
        http.bytes_responses.lock().unwrap().insert(
            format!("PUT /v1/blobs/{resource_id}"),
            CloudHttpBytesResponse {
                status: 201,
                body: format!(
                    r#"{{"resource_id":"{resource_id}","object_key":"blobs/u1/sha256/{hash_hex}","size":{},"content_hash":"{hash_hex}","created_at":1}}"#,
                    data.len()
                )
                .into_bytes(),
                content_hash: None,
            },
        );
        let client = CloudApiClient::with_base_url(http, "https://cloud.test");
        let returned = client.put_blob("good-token", resource_id, data).unwrap();
        assert_eq!(returned, hash);
    }

    #[test]
    fn put_blob_accepts_200_same_hash_retry() {
        let http = FakeCloudHttp::default();
        let resource_id = ResourceId::new();
        let data = b"opaque-cloud-bytes";
        let hash = ContentHash::from_bytes(data).unwrap();
        let hash_hex = content_hash_hex(&hash);
        http.bytes_responses.lock().unwrap().insert(
            format!("PUT /v1/blobs/{resource_id}"),
            CloudHttpBytesResponse {
                status: 200,
                body: format!(
                    r#"{{"resource_id":"{resource_id}","object_key":"blobs/u1/sha256/{hash_hex}","size":{},"content_hash":"{hash_hex}","created_at":1}}"#,
                    data.len()
                )
                .into_bytes(),
                content_hash: None,
            },
        );
        let client = CloudApiClient::with_base_url(http, "https://cloud.test");
        let returned = client.put_blob("good-token", resource_id, data).unwrap();
        assert_eq!(returned, hash);
    }

    #[test]
    fn get_sync_heads_parses_workspace_manifest() {
        let http = FakeCloudHttp::default();
        let workspace_id = "cloud-ws-1";
        http.insert(
            "GET",
            &format!("/v1/workspaces/{workspace_id}/sync-heads"),
            CloudHttpResponse {
                status: 200,
                body: r#"[
                    {
                        "resource_id": "0195a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b",
                        "content_hash": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
                        "updated_at": 1753660800
                    }
                ]"#
                .into(),
            },
        );
        let client = CloudApiClient::with_base_url(http, "https://cloud.test");
        let heads = client.get_sync_heads("good-token", workspace_id).unwrap();
        assert_eq!(heads.len(), 1);
        assert_eq!(heads[0].updated_at, 1753660800);
    }

    #[test]
    fn put_workspace_blob_accepts_workspace_scoped_put() {
        let http = FakeCloudHttp::default();
        let workspace_id = "cloud-ws-1";
        let resource_id = ResourceId::new();
        let data = b"sync-bytes";
        let hash = ContentHash::from_bytes(data).unwrap();
        let hash_hex = content_hash_hex(&hash);
        let prior_hash = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        http.bytes_responses.lock().unwrap().insert(
            format!("PUT /v1/blobs/{resource_id}"),
            CloudHttpBytesResponse {
                status: 200,
                body: format!(
                    r#"{{"resource_id":"{resource_id}","object_key":"blobs/u1/sha256/{hash_hex}","size":{},"content_hash":"{hash_hex}","created_at":1}}"#,
                    data.len()
                )
                .into_bytes(),
                content_hash: None,
            },
        );
        let client = CloudApiClient::with_base_url(http, "https://cloud.test");
        let returned = client
            .put_workspace_blob(
                "good-token",
                Some(workspace_id),
                resource_id,
                data,
                Some(&format!("sha256:{prior_hash}")),
            )
            .unwrap();
        assert_eq!(returned, hash);
    }

    #[test]
    fn put_blob_rejects_200_hash_mismatch() {
        let http = FakeCloudHttp::default();
        let resource_id = ResourceId::new();
        let data = b"opaque-cloud-bytes";
        let hash = ContentHash::from_bytes(data).unwrap();
        let hash_hex = content_hash_hex(&hash);
        let wrong_hash = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        http.bytes_responses.lock().unwrap().insert(
            format!("PUT /v1/blobs/{resource_id}"),
            CloudHttpBytesResponse {
                status: 200,
                body: format!(
                    r#"{{"resource_id":"{resource_id}","object_key":"blobs/u1/sha256/{wrong_hash}","size":{},"content_hash":"{wrong_hash}","created_at":1}}"#,
                    data.len()
                )
                .into_bytes(),
                content_hash: None,
            },
        );
        let client = CloudApiClient::with_base_url(http, "https://cloud.test");
        let err = client
            .put_blob("good-token", resource_id, data)
            .unwrap_err();
        assert!(matches!(err, CloudError::InvalidResponse(_)));
        assert!(err.to_string().contains("hash mismatch"));
        assert!(err.to_string().contains(hash_hex));
    }

    #[test]
    fn put_workspace_backup_accepts_201_created() {
        let http = FakeCloudHttp::default();
        let workspace_id = "cloud-ws-1";
        let data = b"opaque-client-ciphertext";
        let hash = ContentHash::from_bytes(data).unwrap();
        let hash_hex = content_hash_hex(&hash);
        http.bytes_responses.lock().unwrap().insert(
            format!("PUT /v1/workspaces/{workspace_id}/backups"),
            CloudHttpBytesResponse {
                status: 201,
                body: format!(
                    r#"{{"id":"bk-1","workspace_id":"{workspace_id}","device_id":null,"object_key":"backups/{workspace_id}/bk-1","size":{},"content_hash":"{hash_hex}","created_at":1}}"#,
                    data.len()
                )
                .into_bytes(),
                content_hash: None,
            },
        );
        let client = CloudApiClient::with_base_url(http, "https://cloud.test");
        let metadata = client
            .put_workspace_backup("good-token", workspace_id, data, None)
            .unwrap();
        assert_eq!(metadata.content_hash, hash_hex);
        assert_eq!(metadata.size, data.len() as i64);
    }

    #[test]
    fn get_workspace_backup_verifies_content_hash_header() {
        let http = FakeCloudHttp::default();
        let workspace_id = "cloud-ws-1";
        let backup_id = "bk-1";
        let data = b"opaque-client-ciphertext";
        let hash = ContentHash::from_bytes(data).unwrap();
        let hash_hex = content_hash_hex(&hash).to_string();
        http.bytes_responses.lock().unwrap().insert(
            format!("GET /v1/workspaces/{workspace_id}/backups/{backup_id}"),
            CloudHttpBytesResponse {
                status: 200,
                body: data.to_vec(),
                content_hash: Some(hash_hex.clone()),
            },
        );
        let client = CloudApiClient::with_base_url(http.clone(), "https://cloud.test");
        let body = client
            .get_workspace_backup("good-token", workspace_id, backup_id)
            .unwrap();
        assert_eq!(body, data);

        http.bytes_responses.lock().unwrap().insert(
            format!("GET /v1/workspaces/{workspace_id}/backups/{backup_id}"),
            CloudHttpBytesResponse {
                status: 200,
                body: data.to_vec(),
                content_hash: Some("deadbeef".into()),
            },
        );
        let err = client
            .get_workspace_backup("good-token", workspace_id, backup_id)
            .unwrap_err();
        assert!(matches!(err, CloudError::InvalidResponse(_)));
        assert!(err.to_string().contains("hash mismatch"));
    }
}
