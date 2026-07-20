/// Cooperative cancellation hook for long encodes (stub-friendly).
///
/// Callers can later wire a token that flips when the UI cancels; the default
/// [`NeverCancel`] never interrupts.
pub trait CancelCheck {
    fn is_cancelled(&self) -> bool;
}

/// Default cancel check used when the frontend has not wired cancellation yet.
#[derive(Debug, Default, Clone, Copy)]
pub struct NeverCancel;

impl CancelCheck for NeverCancel {
    fn is_cancelled(&self) -> bool {
        false
    }
}
