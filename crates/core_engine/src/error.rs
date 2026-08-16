use thiserror::Error;

/// Errors produced by core domain validation and scheduling.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CoreError {
    #[error("review rating must be between 0 and 5, got {0}")]
    InvalidReviewRating(u8),

    #[error("next review timestamp exceeds the supported UTC range")]
    ReviewDateOverflow,
}
