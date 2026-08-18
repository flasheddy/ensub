//! Portable English tokenization and offline lexicon contracts for Ensub.

#![forbid(unsafe_code)]

mod browser_lexicon;
mod capture;
mod disambiguation;
#[cfg(feature = "document")]
mod document;
mod lexicon;
mod morphology;
mod parser;
mod podcast_feed;
mod transcript;
mod transcript_context;

pub use browser_lexicon::{
    BrowserLexicon, BrowserLexiconAsset, BrowserLexiconError, BrowserLexiconForm,
    BROWSER_LEXICON_SCHEMA_VERSION,
};
pub use capture::{
    capture_from_candidate, capture_from_entry, podcast_capture_from_entry, word_id_for_lemma,
};
pub use disambiguation::{
    prepare_disambiguation, validate_disambiguation_response, CandidateSense,
    DisambiguationConfidence, DisambiguationError, DisambiguationRequest, DisambiguationResponse,
    PreparedDisambiguation, DISAMBIGUATION_SYSTEM_PROMPT, MAX_DISAMBIGUATION_EXPLANATION_CHARS,
    MAX_DISAMBIGUATION_RESPONSE_BYTES,
};
#[cfg(feature = "document")]
pub use document::{
    Block, BlockKind, Document, DocumentFormat, DocumentToken, InlineStyle, StyledRange,
};
pub use lexicon::{Definition, Lexicon, LexiconEntry};
pub use morphology::lemma_candidates;
pub use parser::{
    extract_candidates, segment_text, Candidate, ParseOptions, ParseReport, SentenceSpan,
    TextSegmentation, WordSpan,
};
pub use podcast_feed::{
    parse_podcast_feed, PodcastFeedIssue, PodcastFeedIssueDisposition, PodcastFeedParseError,
    PodcastFeedParseReport,
};
pub use transcript::{parse_transcript, TimestampEndpoint, TranscriptParseError};
pub use transcript_context::{
    reconstruct_transcript_context, ReconstructedTranscriptContext, TranscriptContextError,
};
