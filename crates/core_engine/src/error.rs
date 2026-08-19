use thiserror::Error;

use crate::media::TranscriptFormat;

/// Errors produced by core domain validation and scheduling.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CoreError {
    #[error("review rating must be between 0 and 5, got {0}")]
    InvalidReviewRating(u8),

    #[error("next review timestamp exceeds the supported UTC range")]
    ReviewDateOverflow,
}

/// Errors produced by portable podcast and transcript domain validation.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum MediaDomainError {
    #[error("token span must satisfy start_byte < end_byte, got {start_byte}..{end_byte}")]
    InvalidTokenBounds { start_byte: u32, end_byte: u32 },

    #[error("cue {cue_id} text exceeds the supported token-offset range")]
    CueTextTooLong { cue_id: String },

    #[error(
        "token span {start_byte}..{end_byte} is outside cue {cue_id} text length {text_len_bytes}"
    )]
    TokenSpanOutOfBounds {
        cue_id: String,
        start_byte: u32,
        end_byte: u32,
        text_len_bytes: u32,
    },

    #[error("token offset {offset} is not a UTF-8 character boundary in cue {cue_id}")]
    TokenSpanNotOnCharBoundary { cue_id: String, offset: u32 },

    #[error("token surface does not match cue {cue_id} span {start_byte}..{end_byte}")]
    TokenSurfaceMismatch {
        cue_id: String,
        start_byte: u32,
        end_byte: u32,
    },

    #[error("cue {cue_id} must satisfy start_ms < end_ms, got {start_ms}..{end_ms}")]
    InvalidCueBounds {
        cue_id: String,
        start_ms: u64,
        end_ms: u64,
    },

    #[error("a transcript document requires a supported transcript resource")]
    UnsupportedTranscriptResource,

    #[error("cue {cue_id} format {actual:?} does not match transcript format {expected:?}")]
    CueFormatMismatch {
        cue_id: String,
        expected: TranscriptFormat,
        actual: TranscriptFormat,
    },

    #[error("cue identity {cue_id} appears more than once in the transcript")]
    DuplicateCueId { cue_id: String },

    #[error("cue {cue_id} has source order {actual}, expected {expected}")]
    InvalidCueSourceOrder {
        cue_id: String,
        expected: u32,
        actual: u32,
    },

    #[error(
        "cue {cue_id} starts at {start_ms} ms before cue {previous_cue_id} at {previous_start_ms} ms"
    )]
    CueStartOutOfOrder {
        previous_cue_id: String,
        previous_start_ms: u64,
        cue_id: String,
        start_ms: u64,
    },

    #[error("cue count exceeds the supported active-index capacity")]
    CueIndexCapacityOverflow,

    #[error("a cue range requires at least one cue")]
    EmptyCueRange,

    #[error("cue range must satisfy start_ms < end_ms, got {start_ms}..{end_ms}")]
    InvalidCueRangeBounds { start_ms: u64, end_ms: u64 },

    #[error("adding {padding_ms} ms to cue end {end_ms} ms overflows")]
    PaddedEndOverflow { end_ms: u64, padding_ms: u64 },

    #[error(
        "audio slice must satisfy start_ms < end_ms, got {start_ms}..{end_ms} with duration {duration_ms:?}"
    )]
    InvalidAudioSliceBounds {
        start_ms: u64,
        end_ms: u64,
        duration_ms: Option<u64>,
    },

    #[error("podcast context sentence must not be empty")]
    EmptyPodcastContextSentence,

    #[error("podcast context selected cue identity must not be empty")]
    EmptyPodcastSelectedCueId,

    #[error("podcast context normalized lemma must not be empty")]
    EmptyPodcastNormalizedLemma,

    #[error("podcast context audio source does not match the episode enclosure")]
    PodcastAudioSourceMismatch,

    #[error("podcast context audio slice does not enclose its cue range")]
    PodcastAudioSliceDoesNotCoverCueRange,

    #[error("podcast capture requires exactly one generic context, got {actual}")]
    InvalidPodcastCaptureContextCount { actual: usize },

    #[error("podcast capture word identities do not match")]
    PodcastCaptureWordMismatch,

    #[error("podcast capture context identities do not match")]
    PodcastCaptureContextIdMismatch,

    #[error("podcast context sentence does not match its generic context")]
    PodcastContextSentenceMismatch,

    #[error("podcast context capture time does not match its generic context")]
    PodcastContextTimestampMismatch,

    #[error("podcast context lemma does not match its word record")]
    PodcastContextLemmaMismatch,
}
