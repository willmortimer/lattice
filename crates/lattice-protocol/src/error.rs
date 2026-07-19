use std::io;

/// Errors from length-delimited framing or Protobuf decode.
#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    /// The length prefix exceeds the configured maximum frame size.
    #[error(
        "frame exceeds maximum length of {max_frame_length} bytes (declared {declared_length})"
    )]
    FrameTooLarge {
        max_frame_length: usize,
        declared_length: usize,
    },

    /// The codec rejected input for a reason other than size.
    #[error("framing error: {0}")]
    Framing(#[source] io::Error),

    /// Bytes inside a complete frame are not a valid Envelope.
    #[error("invalid envelope protobuf: {0}")]
    Decode(#[from] prost::DecodeError),
}

impl From<io::Error> for ProtocolError {
    fn from(err: io::Error) -> Self {
        // LengthDelimitedCodec reports oversized frames as InvalidData with
        // Display "frame size too big". Prefer [`ProtocolError::FrameTooLarge`]
        // from the framing pre-check when the declared length is known.
        if err.kind() == io::ErrorKind::InvalidData {
            let message = err.to_string();
            if message.contains("frame size too big") || message.contains("max frame length") {
                return Self::FrameTooLarge {
                    max_frame_length: crate::framing::MAX_FRAME_LENGTH,
                    declared_length: 0,
                };
            }
        }
        Self::Framing(err)
    }
}
