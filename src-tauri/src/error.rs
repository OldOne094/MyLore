//! Application error type shared across layers.
//!
//! Commands return `Result<T, AppError>`; the error is serialized to the
//! frontend as a plain string. Error messages are the public surface of the
//! app — never include secrets, keys, or user data in them.

use serde::Serializer;

/// Top-level error for the application.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("internal error: {0}")]
    Internal(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("configuration error: {0}")]
    Config(String),

    #[error("database error: {0}")]
    Database(Box<sqlx::Error>),
}

impl From<sqlx::Error> for AppError {
    fn from(source: sqlx::Error) -> Self {
        Self::Database(Box::new(source))
    }
}

impl AppError {
    /// Build an internal error without leaking internals to the UI.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }
}

/// Serialize as a plain string so Tauri can ship it to the frontend.
impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_messages_are_readable() {
        assert_eq!(
            AppError::internal("boom").to_string(),
            "internal error: boom"
        );
        assert_eq!(
            AppError::Config("missing key".into()).to_string(),
            "configuration error: missing key"
        );
    }

    #[test]
    fn serializes_to_plain_string() {
        let err = AppError::internal("boom");
        let json = serde_json::to_string(&err).unwrap();
        assert_eq!(json, "\"internal error: boom\"");
    }

    #[test]
    fn converts_from_io_and_serde() {
        let io_err = AppError::from(std::io::Error::new(std::io::ErrorKind::NotFound, "gone"));
        assert!(matches!(io_err, AppError::Io(_)));

        let serde_err: AppError = serde_json::from_str::<serde_json::Value>("{")
            .unwrap_err()
            .into();
        assert!(matches!(serde_err, AppError::Serde(_)));
    }

    #[test]
    fn converts_from_database_error() {
        let err = AppError::from(sqlx::Error::RowNotFound);
        assert!(matches!(err, AppError::Database(_)));
    }
}
