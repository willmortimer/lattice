//! Approval signing for privileged desktop applies (DevCell-shaped).
//!
//! Flow: LocalAuthentication → sign canonical challenge → append JSONL audit.
//! Default backend is software P-256 (works in CI / unsigned builds). On macOS,
//! [`macos_se`] tries a Secure Enclave key first and falls back to software.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::presence::{require_approval_presence, PresenceReason};

const ALGORITHM: &str = "ES256";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalEvidence {
    pub proposal_id: String,
    pub device_id: String,
    pub key_id: String,
    pub algorithm: String,
    /// `software` or `secure-enclave`
    pub backend: String,
    pub signed_payload_sha256: String,
    pub signature_der_hex: String,
    pub authenticated_at_unix: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApprovalChallenge {
    kind: String,
    proposal_id: String,
    workspace_root: String,
    selected_command_indices: Vec<usize>,
    nonce: String,
    issued_at_unix: u64,
}

pub trait ApprovalSigner: Send + Sync {
    fn device_id(&self) -> &str;
    fn key_id(&self) -> &str;
    fn backend(&self) -> &'static str;
    fn sign(&self, payload: &[u8]) -> Result<Vec<u8>, String>;
}

/// Approve a proposal: presence → sign → audit evidence under `.lattice/approvals/`.
pub fn approve_proposal_with_evidence(
    root: &Path,
    proposal_id: &str,
    selected_command_indices: &[usize],
    signer: &dyn ApprovalSigner,
) -> Result<ApprovalEvidence, String> {
    require_approval_presence(PresenceReason::ApproveProposal)?;

    let issued_at = unix_now();
    let challenge = ApprovalChallenge {
        kind: "lattice.proposal.apply".into(),
        proposal_id: proposal_id.to_string(),
        workspace_root: root.display().to_string(),
        selected_command_indices: selected_command_indices.to_vec(),
        nonce: format!("{issued_at}-{}", selected_command_indices.len()),
        issued_at_unix: issued_at,
    };
    let payload = serde_json::to_vec(&challenge).map_err(|err| err.to_string())?;
    let signature = signer.sign(&payload)?;
    let evidence = ApprovalEvidence {
        proposal_id: proposal_id.to_string(),
        device_id: signer.device_id().to_string(),
        key_id: signer.key_id().to_string(),
        algorithm: ALGORITHM.into(),
        backend: signer.backend().into(),
        signed_payload_sha256: hex::encode(Sha256::digest(&payload)),
        signature_der_hex: hex::encode(signature),
        authenticated_at_unix: issued_at,
    };
    append_audit_evidence(root, &evidence)?;
    Ok(evidence)
}

fn append_audit_evidence(root: &Path, evidence: &ApprovalEvidence) -> Result<(), String> {
    let dir = root.join(".lattice").join("approvals");
    fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    let path = dir.join(format!("{}.jsonl", evidence.proposal_id));
    let line = serde_json::to_string(evidence).map_err(|err| err.to_string())?;
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|err| err.to_string())?;
    writeln!(file, "{line}").map_err(|err| err.to_string())
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn key_id_from_spki(spki_der: &[u8]) -> String {
    let digest = Sha256::digest(spki_der);
    format!("key_{}", hex::encode(&digest[..8]))
}

fn lattice_state_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Lattice")
}

fn load_or_create_device_id() -> Result<String, String> {
    let path = lattice_state_dir().join("device-id");
    if let Ok(existing) = fs::read_to_string(&path) {
        let trimmed = existing.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }
    let id = format!("device_{}", uuid::Uuid::new_v4().simple());
    fs::create_dir_all(lattice_state_dir()).map_err(|err| err.to_string())?;
    fs::write(&path, &id).map_err(|err| err.to_string())?;
    Ok(id)
}

/// Process-wide signer (SE when available, else software).
pub fn shared_approval_signer() -> &'static dyn ApprovalSigner {
    static SIGNER: OnceLock<Box<dyn ApprovalSigner>> = OnceLock::new();
    SIGNER
        .get_or_init(|| match create_platform_signer() {
            Ok(signer) => signer,
            Err(err) => {
                eprintln!("lattice: approval signer init failed ({err}); ephemeral software key");
                Box::new(
                    SoftwareApprovalSigner::ephemeral()
                        .expect("ephemeral software approval signer"),
                )
            }
        })
        .as_ref()
}

fn create_platform_signer() -> Result<Box<dyn ApprovalSigner>, String> {
    #[cfg(target_os = "macos")]
    {
        if let Ok(signer) = macos_se::try_load_or_create() {
            return Ok(signer);
        }
    }
    Ok(Box::new(SoftwareApprovalSigner::load_or_create()?))
}

pub struct SoftwareApprovalSigner {
    device_id: String,
    key_id: String,
    signing_key: p256::ecdsa::SigningKey,
}

impl SoftwareApprovalSigner {
    pub fn load_or_create() -> Result<Self, String> {
        use p256::ecdsa::SigningKey;
        use p256::pkcs8::{DecodePrivateKey, EncodePrivateKey, EncodePublicKey};

        let device_id = load_or_create_device_id()?;
        let path = lattice_state_dir().join("approval-software.p8");
        fs::create_dir_all(lattice_state_dir()).map_err(|err| err.to_string())?;
        let signing_key = if path.is_file() {
            let pem = fs::read_to_string(&path).map_err(|err| err.to_string())?;
            SigningKey::from_pkcs8_pem(&pem).map_err(|err| err.to_string())?
        } else {
            let key = SigningKey::random(&mut rand_core::OsRng);
            let pem = key
                .to_pkcs8_pem(p256::pkcs8::LineEnding::LF)
                .map_err(|err| err.to_string())?;
            fs::write(&path, pem.as_str()).map_err(|err| err.to_string())?;
            key
        };
        Self::from_key(device_id, signing_key)
    }

    pub fn ephemeral() -> Result<Self, String> {
        use p256::ecdsa::SigningKey;
        Self::from_key(
            format!("device_{}", uuid::Uuid::new_v4().simple()),
            SigningKey::random(&mut rand_core::OsRng),
        )
    }

    fn from_key(
        device_id: String,
        signing_key: p256::ecdsa::SigningKey,
    ) -> Result<Self, String> {
        use p256::pkcs8::EncodePublicKey;
        let spki = signing_key
            .verifying_key()
            .to_public_key_der()
            .map_err(|err| err.to_string())?;
        Ok(Self {
            device_id,
            key_id: key_id_from_spki(spki.as_bytes()),
            signing_key,
        })
    }
}

impl ApprovalSigner for SoftwareApprovalSigner {
    fn device_id(&self) -> &str {
        &self.device_id
    }

    fn key_id(&self) -> &str {
        &self.key_id
    }

    fn backend(&self) -> &'static str {
        "software"
    }

    fn sign(&self, payload: &[u8]) -> Result<Vec<u8>, String> {
        use p256::ecdsa::{signature::Signer, Signature};
        let sig: Signature = self.signing_key.sign(payload);
        Ok(sig.to_der().as_bytes().to_vec())
    }
}

#[cfg(target_os = "macos")]
mod macos_se {
    use super::*;

    struct SeBridgeSigner {
        inner: lattice_approval_macos::SeApprovalSigner,
    }

    impl ApprovalSigner for SeBridgeSigner {
        fn device_id(&self) -> &str {
            self.inner.device_id()
        }

        fn key_id(&self) -> &str {
            self.inner.key_id()
        }

        fn backend(&self) -> &'static str {
            self.inner.backend()
        }

        fn sign(&self, payload: &[u8]) -> Result<Vec<u8>, String> {
            self.inner.sign(payload)
        }
    }

    /// Prefer CryptoKit Secure Enclave via `libLatticeApprovalBridge.dylib`.
    pub fn try_load_or_create() -> Result<Box<dyn ApprovalSigner>, String> {
        let inner = lattice_approval_macos::SeApprovalSigner::load_or_create()?;
        Ok(Box::new(SeBridgeSigner { inner }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn software_signer_writes_audit_evidence() {
        std::env::set_var("LATTICE_SKIP_PRESENCE", "1");
        let dir = tempdir().unwrap();
        let signer = SoftwareApprovalSigner::ephemeral().unwrap();
        let evidence =
            approve_proposal_with_evidence(dir.path(), "prop-1", &[0, 2], &signer).unwrap();
        assert_eq!(evidence.algorithm, "ES256");
        assert_eq!(evidence.backend, "software");
        assert!(!evidence.signature_der_hex.is_empty());
        let audit = fs::read_to_string(dir.path().join(".lattice/approvals/prop-1.jsonl")).unwrap();
        assert!(audit.contains("prop-1"));
        std::env::remove_var("LATTICE_SKIP_PRESENCE");
    }
}
