use core_engine::{
    CueRange, MediaDomainError, PodcastContextQuality, TranscriptDocument, TranscriptToken,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::segment_text;

const MAX_ADJACENT_CUES: usize = 3;
const MAX_WORDS_EACH_DIRECTION: usize = 60;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReconstructedTranscriptContext {
    pub sentence: String,
    pub quality: PodcastContextQuality,
    pub selected_cue_id: String,
    pub selected_token: TranscriptToken,
    pub cue_range: CueRange,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TranscriptContextError {
    #[error("transcript cue {cue_id} was not found")]
    CueNotFound { cue_id: String },
    #[error("transcript cue {cue_id} has no token at index {token_index}")]
    TokenNotFound { cue_id: String, token_index: usize },
    #[error("the bounded transcript context is empty")]
    EmptyContext,
    #[error("transcript context domain validation failed: {0}")]
    Domain(#[from] MediaDomainError),
}

#[derive(Debug)]
struct CombinedCue {
    document_index: usize,
    start: usize,
    end: usize,
}

#[derive(Debug)]
struct CombinedToken {
    cue_id: String,
    cue_token_index: usize,
    start: usize,
    end: usize,
}

pub fn reconstruct_transcript_context(
    document: &TranscriptDocument,
    cue_id: &str,
    token_index: usize,
) -> Result<ReconstructedTranscriptContext, TranscriptContextError> {
    let selected_cue_index = document
        .cues()
        .iter()
        .position(|cue| cue.id() == cue_id)
        .ok_or_else(|| TranscriptContextError::CueNotFound {
            cue_id: cue_id.to_string(),
        })?;
    let selected_token = document.cues()[selected_cue_index]
        .tokens()
        .get(token_index)
        .cloned()
        .ok_or_else(|| TranscriptContextError::TokenNotFound {
            cue_id: cue_id.to_string(),
            token_index,
        })?;

    let band_start = selected_cue_index.saturating_sub(MAX_ADJACENT_CUES);
    let band_end = document
        .cues()
        .len()
        .min(selected_cue_index.saturating_add(MAX_ADJACENT_CUES + 1));
    let mut text = String::new();
    let mut combined_cues = Vec::with_capacity(band_end.saturating_sub(band_start));
    let mut combined_tokens = Vec::new();

    for document_index in band_start..band_end {
        let cue = &document.cues()[document_index];
        if !text.is_empty() {
            text.push(' ');
        }
        let cue_start = text.len();
        text.push_str(cue.text());
        let cue_end = text.len();
        for (cue_token_index, token) in cue.tokens().iter().enumerate() {
            let start = cue_start
                + usize::try_from(token.start_byte()).map_err(|_| {
                    TranscriptContextError::Domain(MediaDomainError::CueTextTooLong {
                        cue_id: cue.id().to_string(),
                    })
                })?;
            let end = cue_start
                + usize::try_from(token.end_byte()).map_err(|_| {
                    TranscriptContextError::Domain(MediaDomainError::CueTextTooLong {
                        cue_id: cue.id().to_string(),
                    })
                })?;
            combined_tokens.push(CombinedToken {
                cue_id: cue.id().to_string(),
                cue_token_index,
                start,
                end,
            });
        }
        combined_cues.push(CombinedCue {
            document_index,
            start: cue_start,
            end: cue_end,
        });
    }

    let selected_combined_index = combined_tokens
        .iter()
        .position(|token| token.cue_id == cue_id && token.cue_token_index == token_index)
        .ok_or_else(|| TranscriptContextError::TokenNotFound {
            cue_id: cue_id.to_string(),
            token_index,
        })?;
    let first_token_index = selected_combined_index.saturating_sub(MAX_WORDS_EACH_DIRECTION);
    let token_end_index = combined_tokens
        .len()
        .min(selected_combined_index.saturating_add(MAX_WORDS_EACH_DIRECTION + 1));
    let bounded_start = if first_token_index == 0 {
        0
    } else {
        combined_tokens[first_token_index - 1].end
    };
    let bounded_end = if token_end_index == combined_tokens.len() {
        text.len()
    } else {
        combined_tokens[token_end_index].start
    };
    let bounded = text
        .get(bounded_start..bounded_end)
        .ok_or(TranscriptContextError::EmptyContext)?;
    if bounded.trim().is_empty() {
        return Err(TranscriptContextError::EmptyContext);
    }

    let selected_start = combined_tokens[selected_combined_index]
        .start
        .saturating_sub(bounded_start);
    let segmentation = segment_text(bounded);
    let sentence = segmentation
        .sentences
        .iter()
        .find(|sentence| sentence.start <= selected_start && selected_start < sentence.end);
    let left_was_truncated = band_start > 0 || first_token_index > 0;
    let complete = sentence.is_some_and(|sentence| {
        let has_left_boundary = !left_was_truncated
            || bounded
                .get(..sentence.start)
                .is_some_and(ends_with_terminal_punctuation);
        let has_terminal = bounded
            .get(sentence.start..sentence.end)
            .is_some_and(ends_with_terminal_punctuation);
        has_left_boundary && has_terminal
    });
    let (relative_start, relative_end, quality) = if complete {
        let sentence = sentence.ok_or(TranscriptContextError::EmptyContext)?;
        (
            sentence.start,
            sentence.end,
            PodcastContextQuality::CompleteSentence,
        )
    } else {
        (0, bounded.len(), PodcastContextQuality::FallbackCueWindow)
    };
    let raw_start = bounded_start + relative_start;
    let raw_end = bounded_start + relative_end;
    let selected_text = text
        .get(raw_start..raw_end)
        .ok_or(TranscriptContextError::EmptyContext)?;
    let leading = selected_text
        .len()
        .saturating_sub(selected_text.trim_start().len());
    let trailing = selected_text
        .len()
        .saturating_sub(selected_text.trim_end().len());
    let context_start = raw_start + leading;
    let context_end = raw_end.saturating_sub(trailing);
    let sentence = text
        .get(context_start..context_end)
        .ok_or(TranscriptContextError::EmptyContext)?
        .to_string();

    let first_cue = combined_cues
        .iter()
        .position(|cue| cue.end > context_start && cue.start < context_end)
        .ok_or(TranscriptContextError::EmptyContext)?;
    let last_cue = combined_cues
        .iter()
        .rposition(|cue| cue.end > context_start && cue.start < context_end)
        .ok_or(TranscriptContextError::EmptyContext)?;
    let first_document_index = combined_cues[first_cue].document_index;
    let last_document_index = combined_cues[last_cue].document_index;
    let cue_range =
        CueRange::try_from_cues(&document.cues()[first_document_index..=last_document_index])?;

    Ok(ReconstructedTranscriptContext {
        sentence,
        quality,
        selected_cue_id: cue_id.to_string(),
        selected_token,
        cue_range,
    })
}

fn ends_with_terminal_punctuation(text: &str) -> bool {
    text.trim_end()
        .trim_end_matches(['"', '\'', '\u{2019}', '\u{201d}', ')', ']', '}'])
        .ends_with([
            '.', '!', '?', '\u{2026}', '\u{3002}', '\u{ff01}', '\u{ff1f}',
        ])
}
