//! Wasmtime engine/store limits for bounded WASI execution.

use wasmtime::{Config, Engine, Store};

/// Default fuel budget for a single guest invocation (rough instruction count).
pub const DEFAULT_FUEL_LIMIT: u64 = 50_000_000;

/// Default epoch ticks before the guest is interrupted (host calls [`Engine::increment_epoch`]).
pub const DEFAULT_EPOCH_DEADLINE_TICKS: u64 = 1;

/// Fuel and epoch interruption settings applied when building a Wasmtime [`Engine`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WasmtimeLimits {
    /// When `Some`, enables fuel metering with this initial budget per store.
    pub fuel: Option<u64>,
    /// When `Some`, enables epoch interruption and sets the store deadline in epoch ticks.
    pub epoch_deadline_ticks: Option<u64>,
}

impl Default for WasmtimeLimits {
    fn default() -> Self {
        Self {
            fuel: Some(DEFAULT_FUEL_LIMIT),
            epoch_deadline_ticks: Some(DEFAULT_EPOCH_DEADLINE_TICKS),
        }
    }
}

impl WasmtimeLimits {
    /// Limits with fuel metering only (no epoch interruption).
    pub fn fuel_only(fuel: u64) -> Self {
        Self {
            fuel: Some(fuel),
            epoch_deadline_ticks: None,
        }
    }

    /// Limits with epoch interruption only (no fuel metering).
    pub fn epoch_only(epoch_deadline_ticks: u64) -> Self {
        Self {
            fuel: None,
            epoch_deadline_ticks: Some(epoch_deadline_ticks),
        }
    }

    /// No fuel or epoch limits (not recommended for untrusted guests).
    pub fn unlimited() -> Self {
        Self {
            fuel: None,
            epoch_deadline_ticks: None,
        }
    }
}

/// Apply [`WasmtimeLimits`] to a Wasmtime [`Config`] before [`Engine::new`].
pub fn configure_engine(config: &mut Config, limits: &WasmtimeLimits) {
    config.consume_fuel(limits.fuel.is_some());
    config.epoch_interruption(limits.epoch_deadline_ticks.is_some());
}

/// Build an [`Engine`] with [`configure_engine`] applied.
pub fn engine_with_limits(limits: &WasmtimeLimits) -> Result<Engine, wasmtime::Error> {
    let mut config = Config::new();
    configure_engine(&mut config, limits);
    Engine::new(&config)
}

/// Apply per-store fuel and epoch deadline after [`Store::new`].
pub fn configure_store<T>(store: &mut Store<T>, limits: &WasmtimeLimits) -> Result<(), wasmtime::Error> {
    if let Some(fuel) = limits.fuel {
        store.set_fuel(fuel)?;
    }
    if let Some(ticks) = limits.epoch_deadline_ticks {
        store.epoch_deadline_trap();
        store.set_epoch_deadline(ticks);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configure_engine_enables_fuel_and_epoch() {
        let limits = WasmtimeLimits::default();
        let mut config = Config::new();
        configure_engine(&mut config, &limits);
        let engine = Engine::new(&config).expect("engine");
        let mut store: Store<()> = Store::new(&engine, ());
        configure_store(&mut store, &limits).expect("store limits");
        assert_eq!(store.get_fuel().expect("fuel enabled"), DEFAULT_FUEL_LIMIT);
    }

    #[test]
    fn fuel_only_skips_epoch_interruption() {
        let limits = WasmtimeLimits::fuel_only(1234);
        let engine = engine_with_limits(&limits).expect("engine");
        let mut store: Store<()> = Store::new(&engine, ());
        configure_store(&mut store, &limits).expect("store");
        assert_eq!(store.get_fuel().expect("fuel"), 1234);
    }

    #[test]
    fn epoch_only_skips_fuel() {
        let limits = WasmtimeLimits::epoch_only(3);
        let engine = engine_with_limits(&limits).expect("engine");
        let mut store: Store<()> = Store::new(&engine, ());
        configure_store(&mut store, &limits).expect("store");
        assert!(store.get_fuel().is_err());
    }
}
