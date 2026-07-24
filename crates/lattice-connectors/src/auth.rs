//! GitHub App device-flow authentication.

use serde::Deserialize;

use crate::credentials::TokenMaterial;
use crate::error::{Error, Result};

pub const GITHUB_DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
pub const GITHUB_OAUTH_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceFlowStart {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceFlowPending {
    pub client_id: String,
    pub device_code: String,
    pub interval: u64,
    pub expires_in: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceFlowPollResult {
    Pending { interval: u64 },
    SlowDown { interval: u64 },
    Complete(TokenMaterial),
    Expired,
    Denied,
    Error(String),
}

/// Pluggable HTTP surface so tests can stub GitHub.
pub trait GitHubAuthClient: Send + Sync {
    fn post_form(&self, url: &str, form: &[(&str, &str)]) -> Result<String>;
}

pub struct HttpGitHubAuthClient;

impl GitHubAuthClient for HttpGitHubAuthClient {
    fn post_form(&self, url: &str, form: &[(&str, &str)]) -> Result<String> {
        let body = ureq::post(url)
            .set("Accept", "application/json")
            .send_form(form)
            .map_err(|err| Error::http(err.to_string()))?;
        body.into_string()
            .map_err(|err| Error::http(err.to_string()))
    }
}

#[derive(Debug, Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: u64,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    #[serde(default)]
    access_token: Option<String>,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
    #[serde(default)]
    token_type: Option<String>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    error_description: Option<String>,
}

pub fn device_flow_start(
    client: &dyn GitHubAuthClient,
    client_id: &str,
) -> Result<(DeviceFlowStart, DeviceFlowPending)> {
    if client_id.trim().is_empty() {
        return Err(Error::auth(
            "GitHub App client id is required (LATTICE_GITHUB_APP_CLIENT_ID)",
        ));
    }
    let body = client.post_form(
        GITHUB_DEVICE_CODE_URL,
        &[("client_id", client_id), ("scope", "")],
    )?;
    let parsed: DeviceCodeResponse =
        serde_json::from_str(&body).map_err(|err| Error::auth(format!("device code: {err}")))?;
    let start = DeviceFlowStart {
        device_code: parsed.device_code.clone(),
        user_code: parsed.user_code,
        verification_uri: parsed.verification_uri,
        expires_in: parsed.expires_in,
        interval: parsed.interval.max(1),
    };
    let pending = DeviceFlowPending {
        client_id: client_id.to_string(),
        device_code: parsed.device_code,
        interval: start.interval,
        expires_in: start.expires_in,
    };
    Ok((start, pending))
}

pub fn device_flow_poll(
    client: &dyn GitHubAuthClient,
    pending: &DeviceFlowPending,
) -> Result<DeviceFlowPollResult> {
    let body = client.post_form(
        GITHUB_OAUTH_TOKEN_URL,
        &[
            ("client_id", pending.client_id.as_str()),
            ("device_code", pending.device_code.as_str()),
            (
                "grant_type",
                "urn:ietf:params:oauth:grant-type:device_code",
            ),
        ],
    )?;
    let parsed: TokenResponse =
        serde_json::from_str(&body).map_err(|err| Error::auth(format!("token poll: {err}")))?;
    if let Some(token) = parsed.access_token {
        return Ok(DeviceFlowPollResult::Complete(TokenMaterial {
            access_token: token,
            refresh_token: parsed.refresh_token,
            expires_in: parsed.expires_in,
            token_type: parsed.token_type,
        }));
    }
    match parsed.error.as_deref() {
        Some("authorization_pending") => Ok(DeviceFlowPollResult::Pending {
            interval: pending.interval,
        }),
        Some("slow_down") => Ok(DeviceFlowPollResult::SlowDown {
            interval: pending.interval.saturating_add(5),
        }),
        Some("expired_token") => Ok(DeviceFlowPollResult::Expired),
        Some("access_denied") => Ok(DeviceFlowPollResult::Denied),
        Some(other) => Ok(DeviceFlowPollResult::Error(
            parsed
                .error_description
                .unwrap_or_else(|| other.to_string()),
        )),
        None => Err(Error::auth("token response missing access_token and error")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct ScriptedClient {
        responses: Mutex<Vec<String>>,
    }

    impl GitHubAuthClient for ScriptedClient {
        fn post_form(&self, _url: &str, _form: &[(&str, &str)]) -> Result<String> {
            let mut guard = self.responses.lock().unwrap();
            if guard.is_empty() {
                return Err(Error::http("no scripted responses left"));
            }
            Ok(guard.remove(0))
        }
    }

    #[test]
    fn device_flow_start_and_complete() {
        let client = ScriptedClient {
            responses: Mutex::new(vec![
                r#"{
                    "device_code":"dc",
                    "user_code":"ABCD-1234",
                    "verification_uri":"https://github.com/login/device",
                    "expires_in":900,
                    "interval":5
                }"#
                .into(),
                r#"{
                    "access_token":"ghu_test",
                    "token_type":"bearer",
                    "expires_in":28800
                }"#
                .into(),
            ]),
        };
        let (start, pending) = device_flow_start(&client, "client123").unwrap();
        assert_eq!(start.user_code, "ABCD-1234");
        let result = device_flow_poll(&client, &pending).unwrap();
        match result {
            DeviceFlowPollResult::Complete(tok) => assert_eq!(tok.access_token, "ghu_test"),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn device_flow_pending_and_denied() {
        let client = ScriptedClient {
            responses: Mutex::new(vec![
                r#"{"error":"authorization_pending"}"#.into(),
                r#"{"error":"access_denied"}"#.into(),
            ]),
        };
        let pending = DeviceFlowPending {
            client_id: "c".into(),
            device_code: "d".into(),
            interval: 5,
            expires_in: 900,
        };
        assert!(matches!(
            device_flow_poll(&client, &pending).unwrap(),
            DeviceFlowPollResult::Pending { interval: 5 }
        ));
        assert!(matches!(
            device_flow_poll(&client, &pending).unwrap(),
            DeviceFlowPollResult::Denied
        ));
    }
}
