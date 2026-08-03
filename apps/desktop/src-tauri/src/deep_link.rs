//! Deep-link URL parsing for custom scheme and Universal Links.
//!
//! Connector OAuth stays on `lattice://oauth/…` (not cloud). Cloud account
//! browser SIWA returns on `lattice://oauth/cloud/callback`. Workspace open
//! links use `lattice://open?root=…&path=…`. Settings use `lattice://settings/…`.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenSettingsPayload {
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenResourcePayload {
    pub root: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudAuthCallbackPayload {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeepLinkAction {
    /// Connector OAuth (`lattice://oauth/callback…`), not cloud account.
    OAuthCallback(String),
    /// Browser SIWA handoff (`lattice://oauth/cloud/callback…`).
    CloudAuthCallback(CloudAuthCallbackPayload),
    OpenResource(OpenResourcePayload),
    OpenSettings(OpenSettingsPayload),
}

/// Classify a URL delivered by `tauri-plugin-deep-link` (custom scheme or https).
pub fn classify_deep_link(url: &str) -> Option<DeepLinkAction> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(payload) = parse_cloud_auth_callback(trimmed) {
        return Some(DeepLinkAction::CloudAuthCallback(payload));
    }

    if trimmed.starts_with("lattice://oauth/") {
        return Some(DeepLinkAction::OAuthCallback(trimmed.to_string()));
    }

    if let Some(path) = parse_lattice_settings(trimmed) {
        return Some(DeepLinkAction::OpenSettings(OpenSettingsPayload { path }));
    }

    parse_open_resource(trimmed).map(DeepLinkAction::OpenResource)
}

fn parse_cloud_auth_callback(url: &str) -> Option<CloudAuthCallbackPayload> {
    let lower = url.to_ascii_lowercase();
    if !lower.starts_with("lattice://oauth/cloud/callback") {
        return None;
    }
    let parsed = url::Url::parse(url).ok()?;
    Some(CloudAuthCallbackPayload {
        code: query_value(&parsed, "code"),
        state: query_value(&parsed, "state"),
        error: query_value(&parsed, "error"),
    })
}

fn parse_lattice_settings(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    if parsed.scheme() != "lattice" {
        return None;
    }
    let host = parsed.host_str().unwrap_or("");
    if host == "settings" {
        let path = path_from_segments(&parsed);
        return Some(path);
    }
    let path = parsed.path().trim_start_matches('/');
    if path == "settings" || path.starts_with("settings/") {
        return Some(path.trim_start_matches("settings/").trim_matches('/').to_string());
    }
    None
}

fn parse_open_resource(url: &str) -> Option<OpenResourcePayload> {
    let parsed = url::Url::parse(url).ok()?;
    match parsed.scheme() {
        "lattice" => parse_lattice_open(&parsed),
        "https" => parse_https_open(&parsed),
        _ => None,
    }
}

fn parse_lattice_open(parsed: &url::Url) -> Option<OpenResourcePayload> {
    let host = parsed.host_str().unwrap_or("");
    // `lattice://open?…` → host "open"; `lattice:///open?…` → path "/open".
    if host == "open" || ((host.is_empty()) && matches!(parsed.path(), "/open" | "open")) {
        return query_open_payload(parsed);
    }
    if host == "resource" {
        let path = path_from_segments(parsed);
        if path.is_empty() {
            return None;
        }
        let root = query_value(parsed, "root")?;
        return Some(OpenResourcePayload { root, path });
    }
    None
}

fn parse_https_open(parsed: &url::Url) -> Option<OpenResourcePayload> {
    let host = parsed.host_str()?;
    if !matches!(
        host,
        "lattice-notes.com" | "www.lattice-notes.com" | "app.lattice-notes.com"
    ) {
        return None;
    }
    if parsed.path() == "/open" || parsed.path().starts_with("/open/") {
        return query_open_payload(parsed);
    }
    None
}

fn query_open_payload(parsed: &url::Url) -> Option<OpenResourcePayload> {
    let root = query_value(parsed, "root")?;
    let path = query_value(parsed, "path")?;
    Some(OpenResourcePayload { root, path })
}

fn query_value(parsed: &url::Url, key: &str) -> Option<String> {
    for (k, v) in parsed.query_pairs() {
        if k == key {
            let value = v.trim();
            if value.is_empty() {
                return None;
            }
            return Some(value.to_string());
        }
    }
    None
}

fn path_from_segments(parsed: &url::Url) -> String {
    // Prefer path segments so `lattice://resource/Notes/Hello.md` works.
    // Fall back to decoding the raw path for a single percent-encoded segment.
    if let Some(segments) = parsed.path_segments() {
        let joined = segments.collect::<Vec<_>>().join("/");
        if !joined.is_empty() && !joined.contains('%') {
            return joined;
        }
    }
    percent_decode(parsed.path().trim_start_matches('/'))
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (hex_nibble(bytes[i + 1]), hex_nibble(bytes[i + 2])) {
                out.push((hi << 4) | lo);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_oauth_callback() {
        let url = "lattice://oauth/callback?code=abc&state=1";
        assert_eq!(
            classify_deep_link(url),
            Some(DeepLinkAction::OAuthCallback(url.into()))
        );
    }

    #[test]
    fn classifies_cloud_auth_callback() {
        let url = "lattice://oauth/cloud/callback?code=ldh_abc&state=desk1";
        assert_eq!(
            classify_deep_link(url),
            Some(DeepLinkAction::CloudAuthCallback(CloudAuthCallbackPayload {
                code: Some("ldh_abc".into()),
                state: Some("desk1".into()),
                error: None,
            }))
        );
        let err = "lattice://oauth/cloud/callback?state=desk1&error=nope";
        assert_eq!(
            classify_deep_link(err),
            Some(DeepLinkAction::CloudAuthCallback(CloudAuthCallbackPayload {
                code: None,
                state: Some("desk1".into()),
                error: Some("nope".into()),
            }))
        );
    }

    #[test]
    fn classifies_lattice_open_query() {
        let url = "lattice://open?root=/Users/me/ws&path=Notes/Hello.md";
        assert_eq!(
            classify_deep_link(url),
            Some(DeepLinkAction::OpenResource(OpenResourcePayload {
                root: "/Users/me/ws".into(),
                path: "Notes/Hello.md".into(),
            }))
        );
    }

    #[test]
    fn classifies_lattice_resource_host() {
        let nested = "lattice://resource/Notes/Hello.md?root=/tmp/ws";
        assert_eq!(
            classify_deep_link(nested),
            Some(DeepLinkAction::OpenResource(OpenResourcePayload {
                root: "/tmp/ws".into(),
                path: "Notes/Hello.md".into(),
            }))
        );

        let encoded = "lattice://resource/Notes%2FHello.md?root=/tmp/ws";
        assert_eq!(
            classify_deep_link(encoded),
            Some(DeepLinkAction::OpenResource(OpenResourcePayload {
                root: "/tmp/ws".into(),
                path: "Notes/Hello.md".into(),
            }))
        );
    }

    #[test]
    fn classifies_https_open() {
        let url = "https://app.lattice-notes.com/open?root=/tmp/ws&path=a.md";
        assert_eq!(
            classify_deep_link(url),
            Some(DeepLinkAction::OpenResource(OpenResourcePayload {
                root: "/tmp/ws".into(),
                path: "a.md".into(),
            }))
        );
    }

    #[test]
    fn classifies_settings_paths() {
        assert_eq!(
            classify_deep_link("lattice://settings/ai/provider"),
            Some(DeepLinkAction::OpenSettings(OpenSettingsPayload {
                path: "ai/provider".into(),
            }))
        );
        assert_eq!(
            classify_deep_link("lattice://settings/remote-access"),
            Some(DeepLinkAction::OpenSettings(OpenSettingsPayload {
                path: "remote-access".into(),
            }))
        );
    }

    #[test]
    fn rejects_unknown() {
        assert_eq!(
            classify_deep_link("https://example.com/open?root=a&path=b"),
            None
        );
        assert_eq!(classify_deep_link("lattice://oauth-not"), None);
        assert_eq!(classify_deep_link(""), None);
    }
}
