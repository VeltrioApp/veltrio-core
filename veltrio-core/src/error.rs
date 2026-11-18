use thiserror::Error;

#[derive(Debug, Error, uniffi::Error)]
pub enum VeltrioError {
    #[error("network error: {message}")]
    Network { message: String },

    #[error("parse error: {message}")]
    Parse { message: String },

    #[error("invalid URL: {message}")]
    InvalidUrl { message: String },

    #[error("resource not found: {message}")]
    NotFound { message: String },

    #[error("addon does not support this resource: {message}")]
    Unsupported { message: String },

    #[error("internal error: {message}")]
    Internal { message: String },
}

impl From<reqwest::Error> for VeltrioError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_decode() {
            Self::Parse { message: err.to_string() }
        } else {
            Self::Network { message: err.to_string() }
        }
    }
}

impl From<url::ParseError> for VeltrioError {
    fn from(err: url::ParseError) -> Self {
        Self::InvalidUrl { message: err.to_string() }
    }
}

impl From<serde_json::Error> for VeltrioError {
    fn from(err: serde_json::Error) -> Self {
        Self::Parse { message: err.to_string() }
    }
}

pub type VeltrioResult<T> = Result<T, VeltrioError>;
