use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

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

/// Shared atomic cancel token for query / encode sessions.
#[derive(Debug, Default, Clone)]
pub struct AtomicCancel {
    flag: Arc<AtomicBool>,
}

impl AtomicCancel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_flag(flag: Arc<AtomicBool>) -> Self {
        Self { flag }
    }

    pub fn flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.flag)
    }

    pub fn cancel(&self) {
        self.flag.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }
}

impl CancelCheck for AtomicCancel {
    fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }
}

impl CancelCheck for Arc<AtomicBool> {
    fn is_cancelled(&self) -> bool {
        self.load(Ordering::SeqCst)
    }
}
