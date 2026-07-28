//! User-presence prompts via platform LocalAuthentication.
//!
//! This is a session gate (and future privileged-action approval hook), not
//! encryption. See ADR 0049 and ADR 0038.

use std::fmt;

/// Why presence was requested (shown in the system prompt where supported).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresenceReason {
    UnlockApp,
    EnableAppLock,
    /// Approve / apply an agent or external transaction proposal.
    ApproveProposal,
    /// Apply link-repair mutations after a rename.
    ApplyLinkRepair,
}

impl PresenceReason {
    pub fn as_localized(self) -> &'static str {
        match self {
            Self::UnlockApp => "unlock Lattice",
            Self::EnableAppLock => "enable app lock",
            Self::ApproveProposal => "approve changes in Lattice",
            Self::ApplyLinkRepair => "apply link repairs in Lattice",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresenceError {
    Cancelled,
    Failed,
    NotAvailable,
    Unsupported,
}

impl PresenceError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Cancelled => "presence-cancelled",
            Self::Failed => "presence-failed",
            Self::NotAvailable => "presence-not-available",
            Self::Unsupported => "presence-unsupported",
        }
    }
}

impl fmt::Display for PresenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => write!(f, "Authentication was cancelled"),
            Self::Failed => write!(f, "Authentication failed"),
            Self::NotAvailable => write!(f, "Biometric or device authentication is not available"),
            Self::Unsupported => {
                write!(f, "App lock authentication is only available on macOS")
            }
        }
    }
}

impl std::error::Error for PresenceError {}

/// Prompt for device owner authentication (Touch ID with password fallback on macOS).
pub fn request_user_presence(reason: PresenceReason) -> Result<(), PresenceError> {
    request_user_presence_with_reason(reason.as_localized())
}

/// Require presence for a privileged mutation (approve / apply).
///
/// - macOS: fail closed on cancel / failure / unavailable biometrics.
/// - Other platforms: allow until a presence backend exists ([`PresenceError::Unsupported`]).
/// - Automated runs: skip when `LATTICE_SKIP_PRESENCE=1` or the `e2e-testing` feature is on.
pub fn require_approval_presence(reason: PresenceReason) -> Result<(), String> {
    if std::env::var_os("LATTICE_SKIP_PRESENCE").is_some() {
        return Ok(());
    }
    #[cfg(feature = "e2e-testing")]
    {
        let _ = reason;
        return Ok(());
    }
    match request_user_presence(reason) {
        Ok(()) => Ok(()),
        Err(PresenceError::Unsupported) => Ok(()),
        Err(err) => Err(format!("{}: {err}", err.code())),
    }
}

pub fn request_user_presence_with_reason(reason: &str) -> Result<(), PresenceError> {
    #[cfg(target_os = "macos")]
    {
        macos::evaluate(reason)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = reason;
        Err(PresenceError::Unsupported)
    }
}

/// Whether this build can evaluate device-owner presence.
pub fn presence_available() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::can_evaluate()
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use std::sync::mpsc;

    use block2::RcBlock;
    use objc2::runtime::Bool;
    use objc2_foundation::{NSError, NSString};
    use objc2_local_authentication::{LAContext, LAError, LAPolicy};

    use super::PresenceError;

    fn policy() -> LAPolicy {
        // Touch ID / Face ID when available, with device password fallback.
        LAPolicy::DeviceOwnerAuthentication
    }

    pub(super) fn can_evaluate() -> bool {
        let context = unsafe { LAContext::new() };
        unsafe { context.canEvaluatePolicy_error(policy()).is_ok() }
    }

    pub(super) fn evaluate(reason: &str) -> Result<(), PresenceError> {
        if reason.trim().is_empty() {
            return Err(PresenceError::Failed);
        }

        let context = unsafe { LAContext::new() };
        let policy = policy();
        if unsafe { context.canEvaluatePolicy_error(policy) }.is_err() {
            return Err(PresenceError::NotAvailable);
        }

        let (tx, rx) = mpsc::channel::<Result<(), PresenceError>>();
        let localized = NSString::from_str(reason);
        // Keep the context alive until the reply runs.
        let context_retained = context.clone();
        let reply = RcBlock::new(move |success: Bool, error: *mut NSError| {
            let _keep = &context_retained;
            let result = if bool::from(success) {
                Ok(())
            } else {
                Err(map_ns_error(error))
            };
            let _ = tx.send(result);
        });

        unsafe {
            context.evaluatePolicy_localizedReason_reply(policy, &localized, &reply);
        }

        rx.recv().unwrap_or(Err(PresenceError::Failed))
    }

    fn map_ns_error(error: *mut NSError) -> PresenceError {
        if error.is_null() {
            return PresenceError::Failed;
        }
        let error = unsafe { &*error };
        let code = error.code();
        if code == LAError::UserCancel.0
            || code == LAError::AppCancel.0
            || code == LAError::SystemCancel.0
            || code == LAError::UserFallback.0
        {
            PresenceError::Cancelled
        } else if code == LAError::BiometryNotAvailable.0
            || code == LAError::BiometryNotEnrolled.0
            || code == LAError::PasscodeNotSet.0
            || code == LAError::BiometryLockout.0
        {
            PresenceError::NotAvailable
        } else {
            PresenceError::Failed
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presence_reason_copy_is_nonempty() {
        assert!(!PresenceReason::UnlockApp.as_localized().is_empty());
        assert!(!PresenceReason::EnableAppLock.as_localized().is_empty());
        assert!(!PresenceReason::ApproveProposal.as_localized().is_empty());
        assert!(!PresenceReason::ApplyLinkRepair.as_localized().is_empty());
    }

    #[test]
    fn error_codes_are_stable() {
        assert_eq!(PresenceError::Cancelled.code(), "presence-cancelled");
        assert_eq!(PresenceError::Unsupported.code(), "presence-unsupported");
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn non_macos_presence_is_unsupported() {
        assert!(!presence_available());
        assert_eq!(
            request_user_presence(PresenceReason::UnlockApp),
            Err(PresenceError::Unsupported)
        );
    }
}
