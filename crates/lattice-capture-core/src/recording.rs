//! Recording session handle (stub surface for future SCK stream capture).

use crate::CaptureError;

/// Active screen recording; platform backends may implement later.
pub trait RecordingSession: Send {
    /// Stop capture and finalize any encoded output.
    fn stop(&mut self) -> Result<(), CaptureError>;
}
