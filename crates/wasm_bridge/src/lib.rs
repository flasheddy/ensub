//! Browser bindings and persistence adapter for Ensub.

#![forbid(unsafe_code)]

mod ingestion;
mod sandbox;
mod storage;
mod text;

#[cfg(target_arch = "wasm32")]
mod browser;

pub use sandbox::{
    CandidateDto, CaptureOutput, CaptureParsedInput, ContextDto, DueReviewsInput, DueReviewsOutput,
    IntervalDistributionDto, ParseInput, ParseOutput, ReviewCardDto, ReviewInput, ReviewOutput,
    Sandbox, SandboxError, StatsInput, StatsOutput,
};

pub use ingestion::{
    parse_podcast_feed_dto, parse_transcript_dto, EpisodeIdentityDto, IngestionError,
    PodcastEpisodeDto, PodcastFeedDto, PodcastFeedIssueDto, PodcastFeedParseOutputDto,
    TranscriptCueDto, TranscriptDocumentDto, TranscriptResourceDto, TranscriptTokenDto,
};
pub use storage::{
    SnapshotAccess, SnapshotBackend, SnapshotError, SnapshotStorage, SNAPSHOT_SCHEMA_VERSION,
};

#[cfg(target_arch = "wasm32")]
pub use browser::{parse_podcast_feed, parse_transcript, EnsubSandbox};

#[cfg(target_arch = "wasm32")]
pub use storage::{LocalStorageBackend, LocalStorageBackendError};
