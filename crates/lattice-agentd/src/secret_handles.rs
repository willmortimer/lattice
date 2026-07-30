//! Opt-in KernelFS secret handle allowlist (deny-by-default).
//!
//! Hosts map manifest secret handle ids to filesystem paths via process env or
//! per-run tool options. See [`SECRET_HANDLES_ENV`].

use std::path::PathBuf;

use kernelfs::SecretHandleEntry;
use serde::Deserialize;
use serde_json::Value;

/// Process env: JSON array or `id=/path,id2=/path2` mapping for secret handles.
pub const SECRET_HANDLES_ENV: &str = "LATTICE_WASI_SECRET_HANDLES";

#[derive(Debug, Deserialize)]
struct SecretHandleSpec {
    id: String,
    #[serde(alias = "hostPath")]
    host_path: PathBuf,
}

/// Parse a secret-handle allowlist from JSON or `id=/path` pairs.
pub fn parse_secret_handle_allowlist(text: &str) -> Result<Vec<SecretHandleEntry>, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    if trimmed.starts_with('[') {
        return parse_secret_handle_json(trimmed);
    }
    parse_secret_handle_kv(trimmed)
}

fn parse_secret_handle_json(text: &str) -> Result<Vec<SecretHandleEntry>, String> {
    let specs: Vec<SecretHandleSpec> = serde_json::from_str(text)
        .map_err(|err| format!("invalid {SECRET_HANDLES_ENV} JSON: {err}"))?;
    specs
        .into_iter()
        .map(|spec| {
            let id = spec.id.trim().to_string();
            if id.is_empty() {
                return Err(format!("secret handle id must not be empty in {SECRET_HANDLES_ENV}"));
            }
            Ok(SecretHandleEntry {
                id,
                host_path: spec.host_path,
            })
        })
        .collect()
}

fn parse_secret_handle_kv(text: &str) -> Result<Vec<SecretHandleEntry>, String> {
    let mut out = Vec::new();
    for segment in text.split(',') {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }
        let (id, path) = segment
            .split_once('=')
            .ok_or_else(|| {
                format!(
                    "invalid secret handle entry {segment:?}; expected id=/path or JSON array"
                )
            })?;
        let id = id.trim();
        if id.is_empty() {
            return Err(format!("secret handle id must not be empty in {segment:?}"));
        }
        let path = path.trim();
        if path.is_empty() {
            return Err(format!("secret handle path must not be empty for id {id:?}"));
        }
        out.push(SecretHandleEntry {
            id: id.to_string(),
            host_path: PathBuf::from(path),
        });
    }
    Ok(out)
}

/// Load allowlist from [`SECRET_HANDLES_ENV`] when set (invalid values are ignored).
pub fn secret_handles_from_env() -> Vec<SecretHandleEntry> {
    match std::env::var(SECRET_HANDLES_ENV) {
        Ok(text) if !text.trim().is_empty() => parse_secret_handle_allowlist(&text).unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// Parse `secretHandlesJson` tool arg when present; otherwise env.
pub fn secret_handles_for_run(args: &Value) -> Result<Vec<SecretHandleEntry>, String> {
    if let Some(text) = args
        .get("secretHandlesJson")
        .and_then(|value| value.as_str())
        .filter(|s| !s.trim().is_empty())
    {
        return parse_secret_handle_allowlist(text);
    }
    Ok(secret_handles_from_env())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_json_allowlist() {
        let entries = parse_secret_handle_allowlist(
            r#"[{"id":"api-key","hostPath":"/etc/key"},{"id":"token","host_path":"token.txt"}]"#,
        )
        .expect("json");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].id, "api-key");
        assert_eq!(entries[0].host_path, PathBuf::from("/etc/key"));
        assert_eq!(entries[1].id, "token");
        assert_eq!(entries[1].host_path, PathBuf::from("token.txt"));
    }

    #[test]
    fn parses_kv_allowlist() {
        let entries =
            parse_secret_handle_allowlist("api-key=/var/key,token=./secrets/token").expect("kv");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].id, "api-key");
        assert_eq!(entries[0].host_path, PathBuf::from("/var/key"));
        assert_eq!(entries[1].host_path, PathBuf::from("./secrets/token"));
    }

    #[test]
    fn rejects_invalid_kv_segment() {
        let err = parse_secret_handle_allowlist("not-a-mapping").unwrap_err();
        assert!(err.contains("expected id=/path"));
    }
}
