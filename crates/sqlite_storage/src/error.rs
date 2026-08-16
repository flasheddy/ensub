use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SqliteError {
    #[error("SQLite operation failed: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("filesystem operation failed for {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("the platform {0} directory is unavailable")]
    PlatformDirectoryUnavailable(&'static str),

    #[error("database schema version {0} is newer than this Ensub build supports")]
    UnsupportedSchema(i64),

    #[error("lexicon schema version is missing or unsupported: {0}")]
    UnsupportedLexiconSchema(String),

    #[error("lexicon install path has no valid UTF-8 file name: {0}")]
    InvalidLexiconInstallPath(PathBuf),

    #[error("lexicon checksum mismatch for {subject}: expected {expected}, got {actual}")]
    LexiconChecksum {
        subject: String,
        expected: String,
        actual: String,
    },

    #[error("stored UTC millisecond timestamp is invalid: {0}")]
    InvalidTimestamp(i64),

    #[error("stored {field} value is outside the supported range: {value}")]
    InvalidInteger { field: &'static str, value: i64 },

    #[error("stored ease factor is invalid: {0}")]
    InvalidEaseFactor(f64),

    #[error("capture contains records linked to different word identifiers")]
    InvalidCapture,

    #[error("review replacement targets a different word")]
    InvalidReviewReplacement,

    #[error("committed review replacement has no last rating")]
    MissingCommittedReviewRating,

    #[error(transparent)]
    Core(#[from] core_engine::CoreError),
}

impl SqliteError {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}
