//! Generic OAuth authorization-code + PKCE sessions for desktop connectors.
//!
//! Redirect modes:
//! - [`OAuthRedirectMode::Loopback`] — `http://127.0.0.1:<port>/callback`
//!   (required by providers like GitHub that reject custom schemes)
//! - [`OAuthRedirectMode::CustomScheme`] — `lattice://oauth/callback`
//!   (preferred when the IdP allows it; completed via deep link ingest)

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::auth::{HttpOAuthClient, OAuthHttpClient};
use crate::credentials::TokenMaterial;
use crate::error::{Error, Result};

/// Custom URL scheme registered by the desktop shell for OAuth callbacks.
pub const LATTICE_OAUTH_SCHEME: &str = "lattice";
/// Canonical custom-scheme redirect URI for connectors that allow non-http redirects.
pub const LATTICE_OAUTH_CALLBACK_URI: &str = "lattice://oauth/callback";
/// Shared loopback port so IdP callback URLs can be registered once.
pub const DEFAULT_OAUTH_LOOPBACK_PORT: u16 = 17872;

pub const GITHUB_AUTHORIZE_URL: &str = "https://github.com/login/oauth/authorize";
pub const GITLAB_AUTHORIZE_URL: &str = "https://gitlab.com/oauth/authorize";
pub const GITLAB_OAUTH_TOKEN_URL: &str = "https://gitlab.com/oauth/token";

/// How the authorization server redirects back to Lattice after consent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OAuthRedirectMode {
    Loopback { port: u16 },
    CustomScheme,
}

impl OAuthRedirectMode {
    pub fn redirect_uri(self) -> String {
        match self {
            Self::Loopback { port } => format!("http://127.0.0.1:{port}/callback"),
            Self::CustomScheme => LATTICE_OAUTH_CALLBACK_URI.to_string(),
        }
    }
}

/// Provider-agnostic OAuth client configuration for [`oauth_begin`].
#[derive(Debug, Clone)]
pub struct OAuthClientConfig {
    pub provider_id: String,
    pub authorize_url: String,
    pub token_url: String,
    pub client_id: String,
    pub scopes: Vec<String>,
    pub redirect: OAuthRedirectMode,
}

/// Result of starting an OAuth session (URL for [`AuthPresenter`](crate) / system browser).
#[derive(Debug, Clone)]
pub struct OAuthSessionStart {
    pub session_id: String,
    pub provider_id: String,
    pub authorize_url: String,
    pub redirect_uri: String,
    pub redirect_mode: OAuthRedirectMode,
}

/// Back-compat alias used by earlier GitHub-only callers.
pub type OAuthLoopbackStart = OAuthSessionStart;

struct PendingOAuth {
    state: String,
    code_verifier: String,
    redirect_uri: String,
    token_url: String,
    provider_id: String,
    rx: Receiver<CallbackPayload>,
}

#[derive(Debug)]
enum CallbackPayload {
    Code { code: String, state: String },
    Error(String),
}

static PENDING: OnceLock<Mutex<HashMap<String, PendingOAuth>>> = OnceLock::new();
/// Senders keyed by OAuth `state` so deep-link ingest works while finish waits.
static SENDERS_BY_STATE: OnceLock<Mutex<HashMap<String, Sender<CallbackPayload>>>> = OnceLock::new();

fn pending_map() -> &'static Mutex<HashMap<String, PendingOAuth>> {
    PENDING.get_or_init(|| Mutex::new(HashMap::new()))
}

fn senders_by_state() -> &'static Mutex<HashMap<String, Sender<CallbackPayload>>> {
    SENDERS_BY_STATE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn random_url_safe(nbytes: usize) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut raw = Vec::with_capacity(nbytes);
    while raw.len() < nbytes {
        let id = uuid::Uuid::now_v7();
        raw.extend_from_slice(id.as_bytes());
        let mut hasher = DefaultHasher::new();
        id.hash(&mut hasher);
        raw.extend_from_slice(&hasher.finish().to_le_bytes());
    }
    raw.truncate(nbytes);
    base64_url_encode(&raw)
}

fn base64_url_encode(bytes: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity((bytes.len() + 2) / 3 * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            out.push(T[((n >> 6) & 63) as usize] as char);
        }
        if chunk.len() > 2 {
            out.push(T[(n & 63) as usize] as char);
        }
    }
    out
}

fn pkce_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    base64_url_encode(&digest)
}

fn html_page(title: &str, body: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{title}</title></head>\
         <body style=\"font-family:system-ui;padding:2rem\"><h1>{title}</h1><p>{body}</p>\
         <p>You can close this window and return to Lattice.</p></body></html>"
    )
}

fn respond(stream: &mut TcpStream, status: &str, body: &str) {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
}

fn parse_query(query: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for pair in query.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            map.insert(urlencoding_decode(k), urlencoding_decode(v));
        }
    }
    map
}

fn urlencoding_decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let hex = &s[i + 1..i + 3];
                if let Ok(v) = u8::from_str_radix(hex, 16) {
                    out.push(v as char);
                    i += 3;
                } else {
                    out.push('%');
                    i += 1;
                }
            }
            c => {
                out.push(c as char);
                i += 1;
            }
        }
    }
    out
}

fn urlencoding_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn deliver_callback(expected_state: &str, params: &HashMap<String, String>, tx: &Sender<CallbackPayload>) {
    if let Some(err) = params.get("error") {
        let desc = params
            .get("error_description")
            .cloned()
            .unwrap_or_else(|| err.clone());
        let _ = tx.send(CallbackPayload::Error(desc));
        return;
    }
    let Some(code) = params.get("code").cloned() else {
        let _ = tx.send(CallbackPayload::Error("missing code".into()));
        return;
    };
    let state = params.get("state").cloned().unwrap_or_default();
    if state != expected_state {
        let _ = tx.send(CallbackPayload::Error("state mismatch".into()));
        return;
    }
    let _ = tx.send(CallbackPayload::Code { code, state });
}

fn handle_loopback_client(mut stream: TcpStream, expected_state: &str, tx: &Sender<CallbackPayload>) {
    let mut buf = [0u8; 4096];
    let n = match stream.read(&mut buf) {
        Ok(n) => n,
        Err(err) => {
            let _ = tx.send(CallbackPayload::Error(format!("read callback: {err}")));
            return;
        }
    };
    let req = String::from_utf8_lossy(&buf[..n]);
    let first = req.lines().next().unwrap_or("");
    let path = first.split_whitespace().nth(1).unwrap_or("/");
    if !path.starts_with("/callback") {
        respond(
            &mut stream,
            "404 Not Found",
            &html_page("Not found", "Unexpected path."),
        );
        return;
    }
    let query = path.split_once('?').map(|(_, q)| q).unwrap_or("");
    let params = parse_query(query);
    if params.contains_key("error") || !params.contains_key("code") {
        let desc = params
            .get("error_description")
            .or_else(|| params.get("error"))
            .cloned()
            .unwrap_or_else(|| "Authorization failed.".into());
        respond(
            &mut stream,
            "400 Bad Request",
            &html_page("Authorization failed", &desc),
        );
    } else if params.get("state").map(String::as_str) != Some(expected_state) {
        respond(
            &mut stream,
            "400 Bad Request",
            &html_page("Authorization failed", "State mismatch."),
        );
    } else {
        respond(
            &mut stream,
            "200 OK",
            &html_page("Connected", "Authorization complete. Return to Lattice."),
        );
    }
    deliver_callback(expected_state, &params, tx);
}

fn build_authorize_url(config: &OAuthClientConfig, redirect_uri: &str, state: &str, challenge: &str) -> String {
    let mut url = format!(
        "{}?client_id={}&redirect_uri={}&response_type=code&state={}&code_challenge={}&code_challenge_method=S256",
        config.authorize_url,
        urlencoding_encode(&config.client_id),
        urlencoding_encode(redirect_uri),
        urlencoding_encode(state),
        urlencoding_encode(challenge),
    );
    if !config.scopes.is_empty() {
        url.push_str("&scope=");
        url.push_str(&urlencoding_encode(&config.scopes.join(" ")));
    }
    url
}

/// Start an OAuth session and return the authorize URL for the AuthPresenter.
pub fn oauth_begin(config: &OAuthClientConfig) -> Result<OAuthSessionStart> {
    if config.client_id.trim().is_empty() {
        return Err(Error::auth(format!(
            "{} OAuth client id is required",
            config.provider_id
        )));
    }
    let redirect_uri = config.redirect.redirect_uri();
    let state = random_url_safe(24);
    let code_verifier = random_url_safe(64);
    let challenge = pkce_challenge(&code_verifier);
    let authorize_url = build_authorize_url(config, &redirect_uri, &state, &challenge);

    let (tx, rx) = mpsc::channel();
    senders_by_state()
        .lock()
        .map_err(|_| Error::auth("oauth sender lock poisoned"))?
        .insert(state.clone(), tx.clone());

    if let OAuthRedirectMode::Loopback { port } = config.redirect {
        let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], port))).map_err(|err| {
            Error::auth(format!(
                "bind 127.0.0.1:{port} for {} OAuth: {err}. \
                 Register {redirect_uri} on the OAuth app and ensure the port is free.",
                config.provider_id
            ))
        })?;
        let expected_state = state.clone();
        let accept_tx = tx;
        thread::spawn(move || {
            listener.set_nonblocking(false).ok();
            match listener.accept() {
                Ok((stream, _)) => handle_loopback_client(stream, &expected_state, &accept_tx),
                Err(err) => {
                    let _ = accept_tx.send(CallbackPayload::Error(format!("accept: {err}")));
                }
            }
        });
    }

    let session_id = uuid::Uuid::now_v7().to_string();
    pending_map()
        .lock()
        .map_err(|_| Error::auth("oauth session lock poisoned"))?
        .insert(
            session_id.clone(),
            PendingOAuth {
                state,
                code_verifier,
                redirect_uri: redirect_uri.clone(),
                token_url: config.token_url.clone(),
                provider_id: config.provider_id.clone(),
                rx,
            },
        );

    Ok(OAuthSessionStart {
        session_id,
        provider_id: config.provider_id.clone(),
        authorize_url,
        redirect_uri,
        redirect_mode: config.redirect,
    })
}

/// Ingest a redirect URL from a deep link (`lattice://oauth/callback?...`) or
/// an equivalent query string. Completes the matching pending session by `state`.
pub fn oauth_ingest_callback_url(url: &str) -> Result<()> {
    let query = if let Some((_, query)) = url.split_once('?') {
        query
    } else if url.contains('=') && !url.contains("://") {
        url
    } else {
        return Err(Error::auth("OAuth callback URL is missing a query string"));
    };
    let params = parse_query(query);
    let state = params
        .get("state")
        .cloned()
        .ok_or_else(|| Error::auth("OAuth callback missing state"))?;

    let senders = senders_by_state()
        .lock()
        .map_err(|_| Error::auth("oauth sender lock poisoned"))?;
    let tx = senders
        .get(&state)
        .ok_or_else(|| Error::auth("no pending OAuth session matches callback state"))?;
    deliver_callback(&state, &params, tx);
    Ok(())
}

/// Wait for the browser/deep-link callback and exchange the code for tokens.
pub fn oauth_finish(
    client: &dyn OAuthHttpClient,
    session_id: &str,
    client_id: &str,
    client_secret: &str,
    timeout: Duration,
) -> Result<TokenMaterial> {
    if client_secret.trim().is_empty() {
        return Err(Error::auth(
            "OAuth client secret is required for authorization-code exchange",
        ));
    }
    let pending = pending_map()
        .lock()
        .map_err(|_| Error::auth("oauth session lock poisoned"))?
        .remove(session_id)
        .ok_or_else(|| Error::auth(format!("unknown OAuth session {session_id}")))?;

    let payload = match pending.rx.recv_timeout(timeout) {
        Ok(p) => p,
        Err(RecvTimeoutError::Timeout) => {
            let _ = senders_by_state()
                .lock()
                .map_err(|_| Error::auth("oauth sender lock poisoned"))?
                .remove(&pending.state);
            return Err(Error::auth(format!(
                "timed out waiting for {} authorization",
                pending.provider_id
            )));
        }
        Err(RecvTimeoutError::Disconnected) => {
            let _ = senders_by_state()
                .lock()
                .map_err(|_| Error::auth("oauth sender lock poisoned"))?
                .remove(&pending.state);
            return Err(Error::auth("OAuth listener closed unexpectedly"));
        }
    };
    let _ = senders_by_state()
        .lock()
        .map_err(|_| Error::auth("oauth sender lock poisoned"))?
        .remove(&pending.state);

    let code = match payload {
        CallbackPayload::Code { code, state } => {
            if state != pending.state {
                return Err(Error::auth("OAuth state mismatch"));
            }
            code
        }
        CallbackPayload::Error(message) => return Err(Error::auth(message)),
    };

    let body = client.post_form(
        &pending.token_url,
        &[
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("code", code.as_str()),
            ("grant_type", "authorization_code"),
            ("redirect_uri", pending.redirect_uri.as_str()),
            ("code_verifier", pending.code_verifier.as_str()),
        ],
    )?;
    #[derive(serde::Deserialize)]
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
    let parsed: TokenResponse =
        serde_json::from_str(&body).map_err(|err| Error::auth(format!("token exchange: {err}")))?;
    if let Some(token) = parsed.access_token {
        return Ok(TokenMaterial {
            access_token: token,
            refresh_token: parsed.refresh_token,
            expires_in: parsed.expires_in,
            token_type: parsed.token_type,
        });
    }
    Err(Error::auth(
        parsed
            .error_description
            .or(parsed.error)
            .unwrap_or_else(|| "token exchange failed".into()),
    ))
}

/// Convenience finish using the default HTTP client.
pub fn oauth_finish_http(
    session_id: &str,
    client_id: &str,
    client_secret: &str,
    timeout: Duration,
) -> Result<TokenMaterial> {
    oauth_finish(
        &HttpOAuthClient,
        session_id,
        client_id,
        client_secret,
        timeout,
    )
}

/// GitHub App browser OAuth (loopback). Prefer [`oauth_begin`] for new callers.
pub fn oauth_loopback_begin(client_id: &str) -> Result<OAuthSessionStart> {
    oauth_begin(&OAuthClientConfig {
        provider_id: "github".into(),
        authorize_url: GITHUB_AUTHORIZE_URL.into(),
        token_url: crate::auth::GITHUB_OAUTH_TOKEN_URL.into(),
        client_id: client_id.into(),
        scopes: Vec::new(),
        redirect: OAuthRedirectMode::Loopback {
            port: DEFAULT_OAUTH_LOOPBACK_PORT,
        },
    })
}

/// Back-compat wrapper around [`oauth_finish`].
pub fn oauth_loopback_finish(
    client: &dyn OAuthHttpClient,
    session_id: &str,
    client_id: &str,
    client_secret: &str,
    timeout: Duration,
) -> Result<TokenMaterial> {
    oauth_finish(client, session_id, client_id, client_secret, timeout)
}

/// Back-compat wrapper around [`oauth_finish_http`].
pub fn oauth_loopback_finish_http(
    session_id: &str,
    client_id: &str,
    client_secret: &str,
    timeout: Duration,
) -> Result<TokenMaterial> {
    oauth_finish_http(session_id, client_id, client_secret, timeout)
}

/// Deprecated alias for [`DEFAULT_OAUTH_LOOPBACK_PORT`].
pub const GITHUB_OAUTH_LOOPBACK_PORT: u16 = DEFAULT_OAUTH_LOOPBACK_PORT;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_challenge_is_url_safe() {
        let challenge = pkce_challenge("verifier");
        assert!(!challenge.contains('+'));
        assert!(!challenge.contains('/'));
        assert!(!challenge.contains('='));
    }

    #[test]
    fn parse_query_reads_code_and_state() {
        let q = parse_query("code=abc&state=xyz");
        assert_eq!(q.get("code").map(String::as_str), Some("abc"));
        assert_eq!(q.get("state").map(String::as_str), Some("xyz"));
    }

    #[test]
    fn custom_scheme_redirect_uri() {
        assert_eq!(
            OAuthRedirectMode::CustomScheme.redirect_uri(),
            LATTICE_OAUTH_CALLBACK_URI
        );
    }

    #[test]
    fn ingest_completes_custom_scheme_session() {
        let start = oauth_begin(&OAuthClientConfig {
            provider_id: "gitlab".into(),
            authorize_url: GITLAB_AUTHORIZE_URL.into(),
            token_url: GITLAB_OAUTH_TOKEN_URL.into(),
            client_id: "test-client".into(),
            scopes: vec!["read_api".into()],
            redirect: OAuthRedirectMode::CustomScheme,
        })
        .unwrap();
        assert!(start.authorize_url.contains("code_challenge"));
        assert_eq!(start.redirect_uri, LATTICE_OAUTH_CALLBACK_URI);

        let state = {
            let map = pending_map().lock().unwrap();
            map.get(&start.session_id).unwrap().state.clone()
        };
        oauth_ingest_callback_url(&format!(
            "{LATTICE_OAUTH_CALLBACK_URI}?code=test-code&state={state}"
        ))
        .unwrap();

        // Finish will attempt token exchange; use a stub by removing session after ingest
        // proved the channel path — pull payload via finish with a scripted client.
        struct Stub;
        impl OAuthHttpClient for Stub {
            fn post_form(&self, _url: &str, form: &[(&str, &str)]) -> Result<String> {
                assert!(form.iter().any(|(k, v)| *k == "code" && *v == "test-code"));
                Ok(r#"{"access_token":"tok","token_type":"bearer"}"#.into())
            }
        }
        let material = oauth_finish(
            &Stub,
            &start.session_id,
            "test-client",
            "test-secret",
            Duration::from_secs(2),
        )
        .unwrap();
        assert_eq!(material.access_token, "tok");
    }
}
