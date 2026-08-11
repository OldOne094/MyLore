//! Domain-layer error type (MISSION-022).
//!
//! Pure value/entity invariants produce `DomainError`, not `AppError`, keeping
//! the domain layer free of I/O concerns. The application layer maps it to
//! `AppError` when crossing out of the domain.

/// A domain-invariant violation.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum DomainError {
    #[error("validation error: {0}")]
    Validation(String),
}

impl DomainError {
    /// Build a validation error for a violated invariant.
    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation(message.into())
    }
}
