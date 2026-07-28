//! OpenAI Responses API client stub (ADR 0051 / 0066).
//!
//! Live network streaming lands in a follow-on slice. This module compiles and
//! exposes a clear not-implemented surface so `provider: openai` fails loudly
//! without touching the network.

use thiserror::Error;

/// Errors from the (future) OpenAI Responses client.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ResponsesError {
    #[error(
        "openai Responses client is not implemented in this lattice-agentd build (use provider fake)"
    )]
    NotImplemented,
}

/// Placeholder for starting an OpenAI Responses stream.
///
/// Always returns [`ResponsesError::NotImplemented`]. No network I/O.
pub fn start_responses_stream(
    _model: &str,
    _prompt: &str,
) -> Result<ResponsesStreamHandle, ResponsesError> {
    Err(ResponsesError::NotImplemented)
}

/// Opaque handle reserved for a future streaming Responses session.
#[derive(Debug)]
pub struct ResponsesStreamHandle {
    _private: (),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openai_stub_is_not_implemented() {
        let err = start_responses_stream("gpt-test", "hello").unwrap_err();
        assert_eq!(err, ResponsesError::NotImplemented);
        assert!(err.to_string().contains("not implemented"));
    }
}
