//! Browser bindings and persistence adapter for Ensub.

#![forbid(unsafe_code)]

mod ingestion;
mod player;
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
pub use player::{
    EpisodeOpenDto, PlayerWorkspace, PlayerWorkspaceDto, PlayerWorkspaceError, TranscriptStateDto,
    TranscriptSyncDto, MAX_PLAYER_CACHE_BYTES, MAX_PLAYER_CUES, MAX_PLAYER_FEED_BYTES,
    MAX_PLAYER_TRANSCRIPT_BYTES, PLAYER_CACHE_FORMAT, PLAYER_CACHE_SCHEMA_VERSION,
};
pub use storage::{
    SnapshotAccess, SnapshotBackend, SnapshotError, SnapshotStorage, SNAPSHOT_SCHEMA_VERSION,
};

#[cfg(target_arch = "wasm32")]
pub use browser::{parse_podcast_feed, parse_transcript, EnsubPlayerWorkspace, EnsubSandbox};

#[cfg(target_arch = "wasm32")]
pub use storage::{LocalStorageBackend, LocalStorageBackendError};
