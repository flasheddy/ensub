//! Portable English tokenization and offline lexicon contracts for Ensub.

#![forbid(unsafe_code)]

mod browser_lexicon;
mod capture;
#[cfg(feature = "document")]
mod document;
mod lexicon;
mod morphology;
mod parser;

pub use browser_lexicon::{
    BrowserLexicon, BrowserLexiconAsset, BrowserLexiconError, BrowserLexiconForm,
    BROWSER_LEXICON_SCHEMA_VERSION,
};
pub use capture::{capture_from_candidate, capture_from_entry, word_id_for_lemma};
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
