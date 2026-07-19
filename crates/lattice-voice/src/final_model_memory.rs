//! Final-model residency hooks (lazy load / idle unload).
//!
//! Stubs for `latticed` memory policy. Production streaming Unified stays warm
//! separately; this tracks an **optional** second final model (TDT v2 / Unified
//! offline encoder) that must not stay resident by default.

use std::time::{Duration, Instant};

/// Residency of the optional independent final model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinalModelResidency {
    /// On disk only; not mapped.
    Cold,
    /// Load in progress (lazy).
    Loading,
    /// Ready for offline re-decode.
    Ready,
    /// Unload in progress.
    Unloading,
}

/// Action requested by the policy when a final decode is needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinalModelLoadAction {
    /// Caller should start a lazy load of the final model.
    StartLazyLoad,
    /// Model is already ready.
    AlreadyReady,
    /// Load already in flight.
    LoadInProgress,
}

/// Idle unload + lazy-load policy for the optional final model.
///
/// Default idle window is conservative; `latticed` may override once supervised.
#[derive(Debug, Clone)]
pub struct FinalModelMemoryPolicy {
    idle_unload_after: Duration,
    residency: FinalModelResidency,
    last_used: Option<Instant>,
}

impl Default for FinalModelMemoryPolicy {
    fn default() -> Self {
        Self::new(Duration::from_secs(60))
    }
}

impl FinalModelMemoryPolicy {
    #[must_use]
    pub fn new(idle_unload_after: Duration) -> Self {
        Self {
            idle_unload_after,
            residency: FinalModelResidency::Cold,
            last_used: None,
        }
    }

    #[must_use]
    pub fn residency(&self) -> FinalModelResidency {
        self.residency
    }

    #[must_use]
    pub fn idle_unload_after(&self) -> Duration {
        self.idle_unload_after
    }

    /// Request the final model for a decode. Loads lazily from Cold.
    pub fn request_load(&mut self) -> FinalModelLoadAction {
        match self.residency {
            FinalModelResidency::Ready => FinalModelLoadAction::AlreadyReady,
            FinalModelResidency::Loading => FinalModelLoadAction::LoadInProgress,
            FinalModelResidency::Cold | FinalModelResidency::Unloading => {
                self.residency = FinalModelResidency::Loading;
                FinalModelLoadAction::StartLazyLoad
            }
        }
    }

    /// Mark a successful load (stub completion hook).
    pub fn mark_ready(&mut self, now: Instant) {
        self.residency = FinalModelResidency::Ready;
        self.last_used = Some(now);
    }

    /// Record that the final model was used for a decode.
    pub fn mark_used(&mut self, now: Instant) {
        self.last_used = Some(now);
        if self.residency == FinalModelResidency::Ready {
            return;
        }
        // Using while still Loading is allowed once the caller finished load.
        if self.residency == FinalModelResidency::Loading {
            self.residency = FinalModelResidency::Ready;
        }
    }

    /// If idle past the threshold, request unload. Returns true when unload starts.
    pub fn maybe_unload_idle(&mut self, now: Instant) -> bool {
        if self.residency != FinalModelResidency::Ready {
            return false;
        }
        let Some(last_used) = self.last_used else {
            return false;
        };
        if now.duration_since(last_used) < self.idle_unload_after {
            return false;
        }
        self.residency = FinalModelResidency::Unloading;
        true
    }

    /// Complete an unload (stub).
    pub fn mark_cold(&mut self) {
        self.residency = FinalModelResidency::Cold;
        self.last_used = None;
    }

    /// Force unload under memory pressure (always allowed from Ready/Loading).
    pub fn force_unload(&mut self) -> bool {
        match self.residency {
            FinalModelResidency::Cold | FinalModelResidency::Unloading => false,
            FinalModelResidency::Ready | FinalModelResidency::Loading => {
                self.residency = FinalModelResidency::Unloading;
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lazy_load_then_idle_unload() {
        let mut policy = FinalModelMemoryPolicy::new(Duration::from_secs(30));
        assert_eq!(policy.residency(), FinalModelResidency::Cold);
        assert_eq!(policy.request_load(), FinalModelLoadAction::StartLazyLoad);
        assert_eq!(policy.residency(), FinalModelResidency::Loading);

        let t0 = Instant::now();
        policy.mark_ready(t0);
        assert_eq!(policy.request_load(), FinalModelLoadAction::AlreadyReady);
        policy.mark_used(t0);

        assert!(!policy.maybe_unload_idle(t0 + Duration::from_secs(10)));
        assert!(policy.maybe_unload_idle(t0 + Duration::from_secs(31)));
        assert_eq!(policy.residency(), FinalModelResidency::Unloading);
        policy.mark_cold();
        assert_eq!(policy.residency(), FinalModelResidency::Cold);
    }

    #[test]
    fn force_unload_under_pressure() {
        let mut policy = FinalModelMemoryPolicy::default();
        let _ = policy.request_load();
        policy.mark_ready(Instant::now());
        assert!(policy.force_unload());
        assert_eq!(policy.residency(), FinalModelResidency::Unloading);
    }
}
