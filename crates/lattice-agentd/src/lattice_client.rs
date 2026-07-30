//! Typed client for latticed's authenticated localhost HTTP API.
//!
//! Mirrors Node `apps/agentd/src/lattice-client.ts`.

use std::time::Duration;

use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LatticeApiError {
    #[error("Lattice API {status} for {path}: {message} (code={code})")]
    Http {
        status: u16,
        code: String,
        message: String,
        path: String,
    },
    #[error("Lattice API transport error: {0}")]
    Transport(String),
}

#[derive(Debug, Clone)]
pub struct LatticeToolClient {
    base_url: String,
    auth_token: String,
    http: reqwest::Client,
}

impl LatticeToolClient {
    pub fn new(base_url: impl Into<String>, auth_token: impl Into<String>) -> Result<Self, LatticeApiError> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|err| LatticeApiError::Transport(err.to_string()))?;
        Ok(Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            auth_token: auth_token.into(),
            http,
        })
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub async fn post(&self, path: &str, body: Value) -> Result<Value, LatticeApiError> {
        let path = if path.starts_with('/') {
            path.to_string()
        } else {
            format!("/{path}")
        };
        let url = format!("{}{path}", self.base_url);

        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.auth_token))
                .map_err(|err| LatticeApiError::Transport(err.to_string()))?,
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let response = self
            .http
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(|err| LatticeApiError::Transport(err.to_string()))?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|err| LatticeApiError::Transport(err.to_string()))?;

        let parsed: Value = if text.is_empty() {
            Value::Null
        } else {
            serde_json::from_str(&text).unwrap_or_else(|_| serde_json::json!({ "raw": text }))
        };

        if !status.is_success() {
            let (code, message) = match parsed.get("error") {
                Some(err) => (
                    err.get("code")
                        .and_then(|v| v.as_str())
                        .unwrap_or("http_error")
                        .to_string(),
                    err.get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or_else(|| {
                            if text.is_empty() {
                                "Lattice API error"
                            } else {
                                text.as_str()
                            }
                        })
                        .to_string(),
                ),
                None => (
                    "http_error".to_string(),
                    format!("Lattice API {} for {path}", status.as_u16()),
                ),
            };
            return Err(LatticeApiError::Http {
                status: status.as_u16(),
                code,
                message,
                path,
            });
        }

        Ok(parsed)
    }

    pub async fn search(&self, body: Value) -> Result<Value, LatticeApiError> {
        self.post("/v1/search", body).await
    }

    /// Store a workspace-local agent memory row via latticed (not Lance directly).
    pub async fn remember(&self, body: Value) -> Result<Value, LatticeApiError> {
        self.post("/v1/agent_memory/remember", body).await
    }

    /// Recall workspace-local agent memories via latticed (not Lance directly).
    pub async fn recall(&self, body: Value) -> Result<Value, LatticeApiError> {
        self.post("/v1/agent_memory/recall", body).await
    }

    pub async fn read(&self, body: Value) -> Result<Value, LatticeApiError> {
        self.post("/v1/read", body).await
    }

    pub async fn related(&self, body: Value) -> Result<Value, LatticeApiError> {
        self.post("/v1/related", body).await
    }

    pub async fn build_context(&self, body: Value) -> Result<Value, LatticeApiError> {
        self.post("/v1/build_context", body).await
    }

    pub async fn get_dataset_schema(&self, body: Value) -> Result<Value, LatticeApiError> {
        self.post("/v1/datasets/schema", body).await
    }

    pub async fn profile_dataset(&self, body: Value) -> Result<Value, LatticeApiError> {
        self.post("/v1/datasets/profile", body).await
    }

    pub async fn create_proposal(&self, body: Value) -> Result<Value, LatticeApiError> {
        self.post("/v1/proposals/create", body).await
    }

    pub async fn list_proposals(&self, body: Value) -> Result<Value, LatticeApiError> {
        self.post("/v1/proposals/list", body).await
    }

    pub async fn get_proposal(&self, body: Value) -> Result<Value, LatticeApiError> {
        self.post("/v1/proposals/get", body).await
    }

    pub async fn propose_page(&self, body: Value) -> Result<Value, LatticeApiError> {
        self.post("/v1/proposals/propose_page", body).await
    }

    pub async fn propose_resource(&self, body: Value) -> Result<Value, LatticeApiError> {
        self.post("/v1/proposals/propose_resource", body).await
    }

    pub async fn propose_workflow(&self, body: Value) -> Result<Value, LatticeApiError> {
        self.post("/v1/proposals/propose_workflow", body).await
    }

    pub async fn propose_interface(&self, body: Value) -> Result<Value, LatticeApiError> {
        self.post("/v1/proposals/propose_interface", body).await
    }

    pub async fn propose_artifact(&self, body: Value) -> Result<Value, LatticeApiError> {
        self.post("/v1/proposals/propose_artifact", body).await
    }
}

/// Build a client from env when Lattice HTTP tools are configured.
pub fn lattice_client_from_env() -> Option<LatticeToolClient> {
    let base_url = std::env::var("LATTICE_API_BASE_URL")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())?;
    let auth_token = std::env::var("LATTICE_AUTH_TOKEN")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())?;
    LatticeToolClient::new(base_url, auth_token).ok()
}

/// Resolve client from explicit overrides (tests) or process env.
pub fn lattice_client_from_config(
    base_url: Option<&str>,
    auth_token: Option<&str>,
) -> Option<LatticeToolClient> {
    match (base_url, auth_token) {
        (Some(base), Some(token))
            if !base.trim().is_empty() && !token.trim().is_empty() =>
        {
            LatticeToolClient::new(base.trim(), token.trim()).ok()
        }
        (None, None) => lattice_client_from_env(),
        // Partial override: fall back to env for the missing half.
        (base, token) => {
            let base = base
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_string)
                .or_else(|| {
                    std::env::var("LATTICE_API_BASE_URL")
                        .ok()
                        .map(|v| v.trim().to_string())
                        .filter(|v| !v.is_empty())
                })?;
            let token = token
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_string)
                .or_else(|| {
                    std::env::var("LATTICE_AUTH_TOKEN")
                        .ok()
                        .map(|v| v.trim().to_string())
                        .filter(|v| !v.is_empty())
                })?;
            LatticeToolClient::new(base, token).ok()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_env_requires_both() {
        // Do not assert on real process env; only the constructor shape.
        let client = LatticeToolClient::new("http://127.0.0.1:18787/", "tok").unwrap();
        assert_eq!(client.base_url(), "http://127.0.0.1:18787");
    }
}
