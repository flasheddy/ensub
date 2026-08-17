//! Shared domain types and deterministic scheduling policy for Ensub.
//!
//! Platform crates provide storage and presentation. This crate deliberately
//! contains no database, UI, CLI, WASM, or system-clock dependencies.

#![forbid(unsafe_code)]

mod domain;
mod error;
mod media;
mod srs;
mod storage;

pub use domain::{
    Capture, CaptureResult, ContextId, ContextRecord, DailyReviewActivity, IntervalDistribution,
    LibraryOrder, LibraryPage, LibraryQuery, ReviewActivity, ReviewCard, ReviewHistoryEntry,
    ReviewHistoryPage, ReviewHistoryQuery, ReviewRating, ReviewState, ReviewStatistics,
    ReviewUpdate, WordId, WordRecord,
};
pub use error::{CoreError, MediaDomainError};
pub use media::{
    calculate_padded_audio_slice, AudioSlice, CueRange, EpisodeIdentity, PodcastContext,
    PodcastContextQuality, PodcastEpisode, PodcastEpisodeProvenance, PodcastFeed,
    PodcastFeedProvenance, TranscriptCue, TranscriptDocument, TranscriptFormat,
    TranscriptProvenance, TranscriptResource, TranscriptToken, AUDIO_SLICE_PADDING_MS,
};
pub use srs::{
    calculate_next_ease_factor, calculate_next_interval_days, calculate_next_repetitions,
    initial_review_state, schedule_review, DEFAULT_EASE_FACTOR, MIN_EASE_FACTOR,
};
pub use storage::{LibraryStorageAdapter, StorageAdapter};
