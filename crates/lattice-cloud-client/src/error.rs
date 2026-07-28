use thiserror::Error;

#[derive(Debug, Error)]
pub enum CloudError {
    #[error("cloud request failed: {0}")]
    Http(String),
    #[error("cloud API error ({status}): {message}")]
    Api { status: u16, message: String },
    #[error("invalid cloud response: {0}")]
    InvalidResponse(String),
    #[error("credentials store error: {0}")]
    Credentials(String),
    #[error("not signed in")]
    NotSignedIn,
}

impl CloudError {
    pub fn api_status(&self) -> Option<u16> {
        match self {
            Self::Api { status, .. } => Some(*status),
            _ => None,
        }
    }
}

pub type Result<T> = std::result::Result<T, CloudError>;
