//! User-presence prompts via platform LocalAuthentication / Windows Hello.
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
                write!(
                    f,
                    "App lock authentication is not available on this platform"
                )
            }
        }
    }
}

impl std::error::Error for PresenceError {}

/// Prompt for device owner authentication (Touch ID / Windows Hello with PIN fallback).
pub fn request_user_presence(reason: PresenceReason) -> Result<(), PresenceError> {
    request_user_presence_with_reason(reason.as_localized(), None)
}

/// Prompt for presence, associating the Windows Hello UI with an owner HWND when provided.
///
/// On Win32 hosts, Hello must be requested via `IUserConsentVerifierInterop` with the
/// app window handle; otherwise the consent UI can freeze the webview.
pub fn request_user_presence_for_window(
    reason: PresenceReason,
    window_hwnd: Option<isize>,
) -> Result<(), PresenceError> {
    request_user_presence_with_reason(reason.as_localized(), window_hwnd)
}

/// Require presence for a privileged mutation (approve / apply).
///
/// - macOS / Windows: fail closed on cancel / failure / unavailable biometrics.
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

pub fn request_user_presence_with_reason(
    reason: &str,
    window_hwnd: Option<isize>,
) -> Result<(), PresenceError> {
    #[cfg(target_os = "macos")]
    {
        let _ = window_hwnd;
        macos::evaluate(reason)
    }
    #[cfg(target_os = "windows")]
    {
        windows_hello::evaluate(reason, window_hwnd)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = (reason, window_hwnd);
        Err(PresenceError::Unsupported)
    }
}

/// Whether this build can evaluate device-owner presence.
pub fn presence_available() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::can_evaluate()
    }
    #[cfg(target_os = "windows")]
    {
        windows_hello::can_evaluate()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        false
    }
}

/// Map WinRT `UserConsentVerificationResult` discriminant (stable ABI values).
///
/// Kept platform-agnostic so Darwin CI can cover Windows Hello error mapping
/// without a live verifier.
#[cfg(any(test, target_os = "windows"))]
pub(crate) fn map_user_consent_verification_result(code: i32) -> Result<(), PresenceError> {
    match code {
        0 => Ok(()),                                   // Verified
        6 => Err(PresenceError::Cancelled),            // Canceled
        1 | 2 | 3 => Err(PresenceError::NotAvailable), // DeviceNotPresent / NotConfigured / DisabledByPolicy
        _ => Err(PresenceError::Failed),               // DeviceBusy / RetriesExhausted / unknown
    }
}

/// Map WinRT `UserConsentVerifierAvailability` discriminant (stable ABI values).
#[cfg(any(test, target_os = "windows"))]
pub(crate) fn map_user_consent_availability(code: i32) -> bool {
    matches!(
        code,
        0 | 4 // Available | DeviceBusy
    )
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

#[cfg(target_os = "windows")]
mod windows_hello {
    use std::sync::mpsc;
    use std::thread;

    use windows::core::{factory, HSTRING};
    use windows::Security::Credentials::UI::{UserConsentVerificationResult, UserConsentVerifier};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::WinRT::IUserConsentVerifierInterop;
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, SetForegroundWindow};
    use windows_future::IAsyncOperation;

    use super::{
        map_user_consent_availability, map_user_consent_verification_result, PresenceError,
    };

    pub(super) fn can_evaluate() -> bool {
        match check_availability() {
            Ok(availability) => map_user_consent_availability(availability.0),
            Err(_) => false,
        }
    }

    pub(super) fn evaluate(reason: &str, window_hwnd: Option<isize>) -> Result<(), PresenceError> {
        if reason.trim().is_empty() {
            return Err(PresenceError::Failed);
        }

        match check_availability() {
            Ok(availability) if map_user_consent_availability(availability.0) => {}
            Ok(_) => return Err(PresenceError::NotAvailable),
            Err(_) => return Err(PresenceError::NotAvailable),
        }

        let reason = reason.to_string();
        // WinRT `.get()` pumps COM on the calling thread. Running it on a worker
        // keeps Tauri's async/UI path responsive while Hello is showing.
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let _ = tx.send(evaluate_blocking(&reason, window_hwnd));
        });
        rx.recv().unwrap_or(Err(PresenceError::Failed))
    }

    fn evaluate_blocking(reason: &str, window_hwnd: Option<isize>) -> Result<(), PresenceError> {
        let window = resolve_owner_hwnd(window_hwnd);
        focus_owner_window(window);

        let interop = factory::<UserConsentVerifier, IUserConsentVerifierInterop>()
            .map_err(|_| PresenceError::Failed)?;
        let operation: IAsyncOperation<UserConsentVerificationResult> = unsafe {
            interop
                .RequestVerificationForWindowAsync(window, &HSTRING::from(reason))
                .map_err(|_| PresenceError::Failed)?
        };
        let result = operation.get().map_err(|_| PresenceError::Failed)?;
        map_user_consent_verification_result(result.0)
    }

    fn resolve_owner_hwnd(window_hwnd: Option<isize>) -> HWND {
        if let Some(raw) = window_hwnd {
            if raw != 0 {
                return HWND(raw as *mut _);
            }
        }
        // Foreground as last resort for approval paths without an AppHandle.
        unsafe { GetForegroundWindow() }
    }

    fn focus_owner_window(window: HWND) {
        if window.0.is_null() {
            return;
        }
        unsafe {
            let _ = SetForegroundWindow(window);
        }
    }

    fn check_availability(
    ) -> windows::core::Result<windows::Security::Credentials::UI::UserConsentVerifierAvailability>
    {
        UserConsentVerifier::CheckAvailabilityAsync()?.get()
    }

    #[cfg(test)]
    mod win_abi_smoke {
        use windows::Security::Credentials::UI::{
            UserConsentVerificationResult, UserConsentVerifierAvailability,
        };

        use super::super::{map_user_consent_availability, map_user_consent_verification_result};

        #[test]
        fn verification_result_abi_matches_mapper() {
            assert_eq!(UserConsentVerificationResult::Verified.0, 0);
            assert_eq!(UserConsentVerificationResult::Canceled.0, 6);
            assert_eq!(UserConsentVerificationResult::DeviceNotPresent.0, 1);
            assert!(map_user_consent_verification_result(0).is_ok());
        }

        #[test]
        fn availability_abi_matches_mapper() {
            assert_eq!(UserConsentVerifierAvailability::Available.0, 0);
            assert_eq!(UserConsentVerifierAvailability::DeviceBusy.0, 4);
            assert!(map_user_consent_availability(0));
            assert!(map_user_consent_availability(4));
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

    #[test]
    fn unsupported_message_is_platform_neutral() {
        let message = PresenceError::Unsupported.to_string();
        assert!(!message.to_lowercase().contains("macos only"));
        assert!(message.contains("not available on this platform"));
    }

    #[test]
    fn windows_hello_verification_mapping() {
        assert_eq!(map_user_consent_verification_result(0), Ok(()));
        assert_eq!(
            map_user_consent_verification_result(6),
            Err(PresenceError::Cancelled)
        );
        assert_eq!(
            map_user_consent_verification_result(1),
            Err(PresenceError::NotAvailable)
        );
        assert_eq!(
            map_user_consent_verification_result(2),
            Err(PresenceError::NotAvailable)
        );
        assert_eq!(
            map_user_consent_verification_result(3),
            Err(PresenceError::NotAvailable)
        );
        assert_eq!(
            map_user_consent_verification_result(4),
            Err(PresenceError::Failed)
        );
        assert_eq!(
            map_user_consent_verification_result(5),
            Err(PresenceError::Failed)
        );
    }

    #[test]
    fn windows_hello_availability_mapping() {
        assert!(map_user_consent_availability(0));
        assert!(map_user_consent_availability(4));
        assert!(!map_user_consent_availability(1));
        assert!(!map_user_consent_availability(2));
        assert!(!map_user_consent_availability(3));
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    #[test]
    fn unsupported_platforms_report_unavailable() {
        assert!(!presence_available());
        assert_eq!(
            request_user_presence(PresenceReason::UnlockApp),
            Err(PresenceError::Unsupported)
        );
    }
}
