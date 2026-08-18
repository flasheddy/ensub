//! Browser bindings and persistence adapter for Ensub.

#![forbid(unsafe_code)]

mod disambiguation;
mod ingestion;
mod learning;
mod player;
mod review;
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

pub use disambiguation::{
    CandidateSenseDto, DisambiguationRequestDto, DisambiguationResponseDto,
    PrepareDisambiguationInputDto, PreparedDisambiguationDto,
    ValidateDisambiguationResponseInputDto,
};

pub use ingestion::{
    parse_podcast_feed_dto, parse_transcript_dto, EpisodeIdentityDto, IngestionError,
    PodcastEpisodeDto, PodcastFeedDto, PodcastFeedIssueDto, PodcastFeedParseOutputDto,
    TranscriptCueDto, TranscriptDocumentDto, TranscriptResourceDto, TranscriptTokenDto,
};
pub use learning::{
    CapturePodcastInput, CapturePodcastOutput, CapturePodcastStatus, LookupDefinitionDto,
    LookupEntryDto, PlayerLearning, PlayerLearningError, TokenLookupDto,
};
pub use player::{
    CueNavigationDto, EpisodeOpenDto, PlayerWorkspace, PlayerWorkspaceDto, PlayerWorkspaceError,
    PreparePodcastCaptureInput, PreparedPodcastCaptureDto, TranscriptStateDto, TranscriptSyncDto,
    MAX_PLAYER_CACHE_BYTES, MAX_PLAYER_CUES, MAX_PLAYER_FEED_BYTES, MAX_PLAYER_FIXTURE_BYTES,
    MAX_PLAYER_TRANSCRIPT_BYTES, PLAYER_CACHE_FORMAT, PLAYER_CACHE_SCHEMA_VERSION,
};
pub use review::{
    AudioSlicePlaybackDto, DueCountDto, DueCountInputDto, DueReviewsDto, DueReviewsInputDto,
    PodcastContextQualityDto, RateReviewInputDto, RevealReviewInputDto, ReviewAnswerDto,
    ReviewContextDto, ReviewPromptDto, ReviewTransitionDto,
};
pub use storage::{
    SnapshotAccess, SnapshotBackend, SnapshotError, SnapshotMigrationStatus, SnapshotStorage,
    SNAPSHOT_SCHEMA_VERSION, V01_BACKUP_STORAGE_KEY,
};

#[cfg(target_arch = "wasm32")]
pub use browser::{
    parse_podcast_feed, parse_transcript, EnsubPlayerLearning, EnsubPlayerWorkspace, EnsubSandbox,
};

#[cfg(target_arch = "wasm32")]
pub use storage::{LocalStorageBackend, LocalStorageBackendError};
