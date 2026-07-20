/// Default row cap for a single analytical IPC batch (ADR 0021 bounded transfer).
pub const DEFAULT_MAX_ROWS: usize = 10_000;

/// Default encoded IPC byte cap (8 MiB). Oversized batches shrink row count until they fit.
pub const DEFAULT_MAX_BYTES: usize = 8 * 1024 * 1024;
