//! Where captured pixels should be routed after capture.

/// Post-capture routing target (handled above the backend in desktop flows).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureDestination {
    Clipboard,
    CaptureInbox,
    CurrentNote,
    CurrentCanvas,
    NamedCollection(String),
}
