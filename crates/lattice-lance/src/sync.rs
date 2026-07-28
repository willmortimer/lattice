use crate::error::{LanceError, Result};

/// Run an async future on the current Tokio runtime from synchronous callers.
///
/// Callers that do not already hold a Tokio runtime should construct one at the
/// integration boundary (for example in `lattice-index` during T3).
pub fn block_on<F>(future: F) -> Result<F::Output>
where
    F: std::future::Future,
{
    Ok(tokio::runtime::Handle::try_current()
        .map_err(|_| {
            LanceError::Store {
                message: "no Tokio runtime available; call from an async context or install a runtime before using block_on".into(),
            }
        })?
        .block_on(future))
}
