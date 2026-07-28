use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::config::cloud_url;
use crate::error::{CloudError, Result};
use crate::types::{AuthTokenResponse, MeResponse};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudHttpResponse {
    pub status: u16,
    pub body: String,
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
            other => {
                return Err(CloudError::Http(format!("unsupported method {other}")));
            }
        };
        request = request.set("Accept", "application/json");
        if let Some(token) = bearer {
            request = request.set("Authorization", &format!("Bearer {token}"));
        }
        let response = if let Some(payload) = body {
            let json = serde_json::to_string(payload)
                .map_err(|err| CloudError::Http(err.to_string()))?;
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use super::*;

    #[derive(Default, Clone)]
    struct FakeCloudHttp {
        responses: Arc<Mutex<HashMap<String, CloudHttpResponse>>>,
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
        assert_eq!(status.user.as_ref().map(|user| user.id.as_str()), Some("u1"));
        let signed_out = sign_out(&client, &store).unwrap();
        assert!(!signed_out.signed_in);
        let status = cloud_session_status(&client, &store).unwrap();
        assert!(!status.signed_in);
    }
}
