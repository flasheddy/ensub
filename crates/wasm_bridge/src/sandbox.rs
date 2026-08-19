use std::collections::HashSet;

use chrono::{DateTime, Utc};
use core_engine::{schedule_review, ReviewRating, ReviewUpdate, StorageAdapter, WordId};
use language_engine::{
    capture_from_candidate, extract_candidates, BrowserLexicon, BrowserLexiconError, Candidate,
    Definition, ParseOptions,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::review::review_token;
use crate::text::utf16_offset as utf16_offset_raw;
use crate::{
    SnapshotAccess, SnapshotBackend, SnapshotError, SnapshotMigrationStatus, SnapshotStorage,
};

const DEFAULT_MAX_CANDIDATES: usize = 100;

pub struct Sandbox<B> {
    lexicon: BrowserLexicon,
    storage: SnapshotStorage<B>,
}

impl<B: SnapshotBackend> Sandbox<B> {
    pub fn open(
        backend: B,
        storage_key: impl Into<String>,
        access: SnapshotAccess,
        lexicon_bytes: &[u8],
    ) -> Result<Self, SandboxError> {
        Ok(Self {
            lexicon: BrowserLexicon::decode(lexicon_bytes)?,
            storage: SnapshotStorage::new(backend, storage_key, access),
        })
    }

    pub fn parse(&self, input: &ParseInput) -> Result<ParseOutput, SandboxError> {
        let report =
            self.parse_report(&input.text, input.include_stopwords, input.max_candidates)?;
        let candidates = report
            .candidates
            .iter()
            .map(|candidate| candidate_dto(&input.text, candidate))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ParseOutput {
            candidates,
            lookup_misses: report.lookup_misses,
            filtered_stopwords: report.filtered_stopwords,
            truncated_candidates: report.truncated_candidates,
        })
    }

    pub fn initialize_storage(&mut self) -> Result<SnapshotMigrationStatus, SandboxError> {
        self.storage.initialize().map_err(Into::into)
    }

    pub fn is_read_only(&self) -> bool {
        self.storage.is_read_only()
    }

    pub fn raw_snapshot(&self) -> Result<Option<String>, SandboxError> {
        self.storage.raw_snapshot().map_err(Into::into)
    }

    pub fn capture_parsed(
        &mut self,
        input: &CaptureParsedInput,
    ) -> Result<CaptureOutput, SandboxError> {
        let captured_at = timestamp(input.captured_at_ms)?;
        let report =
            self.parse_report(&input.text, input.include_stopwords, input.max_candidates)?;
        let mut selected = HashSet::with_capacity(input.candidate_ids.len());
        for id in &input.candidate_ids {
            if !selected.insert(id.clone()) {
                return Err(SandboxError::DuplicateCandidate(id.clone()));
            }
        }
        let mut captures = Vec::with_capacity(selected.len());
        for candidate in &report.candidates {
            let id = candidate_id(candidate);
            if selected.remove(&id) {
                captures.push(capture_from_candidate(
                    candidate,
                    input.source.trim(),
                    captured_at,
                ));
            }
        }
        if let Some(unknown) = selected.into_iter().next() {
            return Err(SandboxError::UnknownCandidate(unknown));
        }
        let results = self.storage.save_captures(&captures)?;
        Ok(CaptureOutput {
            captured_cards: results.len(),
            created_cards: results.iter().filter(|result| result.word_created).count(),
            created_contexts: results.iter().fold(0_u64, |total, result| {
                total.saturating_add(result.contexts_created)
            }),
        })
    }

    pub fn due_reviews(&self, input: &DueReviewsInput) -> Result<DueReviewsOutput, SandboxError> {
        let mut cards = self.storage.due_reviews(timestamp(input.as_of_ms)?)?;
        cards.truncate(input.limit);
        Ok(DueReviewsOutput {
            cards: cards
                .into_iter()
                .map(|card| {
                    Ok(ReviewCardDto {
                        word_id: card.word.id.into_inner(),
                        term: card.word.term,
                        lemma: card.word.lemma,
                        phonetic: card.word.phonetic,
                        definition: card.word.definition,
                        contexts: card
                            .contexts
                            .into_iter()
                            .map(|context| ContextDto {
                                sentence: context.sentence,
                                source: context.source,
                                captured_at_ms: context.captured_at.timestamp_millis(),
                            })
                            .collect(),
                        ease_factor: card.state.ease_factor,
                        repetitions: card.state.repetitions,
                        interval_days: card.state.interval_days,
                        next_review_at_ms: card.state.next_review_at.timestamp_millis(),
                        review_token: review_token(&card.state)
                            .map_err(|error| SandboxError::Serialization(error.to_string()))?,
                    })
                })
                .collect::<Result<Vec<_>, SandboxError>>()?,
        })
    }

    pub fn review(&mut self, input: &ReviewInput) -> Result<ReviewOutput, SandboxError> {
        let word_id = WordId::new(input.word_id.clone());
        let current = self
            .storage
            .review_state(&word_id)?
            .ok_or(SandboxError::ReviewConflict)?;
        if review_token(&current).map_err(|error| SandboxError::Serialization(error.to_string()))?
            != input.review_token
        {
            return Err(SandboxError::ReviewConflict);
        }
        let rating = ReviewRating::try_from(input.rating)
            .map_err(|_| SandboxError::InvalidRating(input.rating))?;
        let reviewed_at = timestamp(input.reviewed_at_ms)?;
        let replacement = schedule_review(&current, rating, reviewed_at)?;
        match self
            .storage
            .commit_review(&current, &replacement, reviewed_at)?
        {
            ReviewUpdate::Updated => Ok(ReviewOutput {
                ease_factor: replacement.ease_factor,
                repetitions: replacement.repetitions,
                interval_days: replacement.interval_days,
                next_review_at_ms: replacement.next_review_at.timestamp_millis(),
            }),
            ReviewUpdate::Conflict => Err(SandboxError::ReviewConflict),
        }
    }

    pub fn stats(&self, input: &StatsInput) -> Result<StatsOutput, SandboxError> {
        let as_of = timestamp(input.as_of_ms)?;
        let statistics = self.storage.review_statistics(as_of)?;
        Ok(StatsOutput {
            total_cards: statistics.total_cards,
            due_cards: statistics.due_cards,
            intervals: IntervalDistributionDto {
                new: statistics.intervals.new,
                days_1_to_6: statistics.intervals.days_1_to_6,
                days_7_to_30: statistics.intervals.days_7_to_30,
                days_31_to_90: statistics.intervals.days_31_to_90,
                days_91_plus: statistics.intervals.days_91_plus,
            },
            next_review_at_ms: self
                .storage
                .next_review_at_after(as_of)?
                .map(|date| date.timestamp_millis()),
        })
    }

    pub fn reset(&mut self) -> Result<(), SandboxError> {
        self.storage.reset()?;
        Ok(())
    }

    fn parse_report(
        &self,
        text: &str,
        include_stopwords: bool,
        max_candidates: usize,
    ) -> Result<language_engine::ParseReport, SandboxError> {
        if text.trim().is_empty() {
            return Err(SandboxError::EmptyText);
        }
        let max_candidates = if max_candidates == 0 {
            DEFAULT_MAX_CANDIDATES
        } else {
            max_candidates
        };
        match extract_candidates(
            text,
            &self.lexicon,
            ParseOptions {
                include_stopwords,
                max_candidates,
            },
        ) {
            Ok(report) => Ok(report),
            Err(error) => match error {},
        }
    }
}

fn timestamp(milliseconds: i64) -> Result<DateTime<Utc>, SandboxError> {
    DateTime::from_timestamp_millis(milliseconds)
        .ok_or(SandboxError::InvalidTimestamp(milliseconds))
}

fn candidate_dto(text: &str, candidate: &Candidate) -> Result<CandidateDto, SandboxError> {
    Ok(CandidateDto {
        id: candidate_id(candidate),
        surface: candidate.surface.clone(),
        sentence: candidate.sentence.clone(),
        sentence_start: utf16_offset(text, candidate.sentence_start)?,
        sentence_end: utf16_offset(text, candidate.sentence_end)?,
        token_start: utf16_offset(text, candidate.token_start)?,
        token_end: utf16_offset(text, candidate.token_end)?,
        lemma: candidate.entry.lemma.clone(),
        phonetic: candidate.entry.phonetic.clone(),
        definitions: candidate.entry.definitions.clone(),
    })
}

fn utf16_offset(text: &str, byte_offset: usize) -> Result<usize, SandboxError> {
    utf16_offset_raw(text, byte_offset).ok_or(SandboxError::InvalidTextBoundary(byte_offset))
}

fn candidate_id(candidate: &Candidate) -> String {
    let identity = format!(
        "{}:{}:{}:{}",
        candidate.token_start, candidate.token_end, candidate.entry.lemma, candidate.surface
    );
    format!("c-{:016x}", fnv1a(identity.as_bytes()))
}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325_u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseInput {
    pub text: String,
    #[serde(default)]
    pub include_stopwords: bool,
    #[serde(default = "default_max_candidates")]
    pub max_candidates: usize,
}

fn default_max_candidates() -> usize {
    DEFAULT_MAX_CANDIDATES
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateDto {
    pub id: String,
    pub surface: String,
    pub sentence: String,
    pub sentence_start: usize,
    pub sentence_end: usize,
    pub token_start: usize,
    pub token_end: usize,
    pub lemma: String,
    pub phonetic: String,
    pub definitions: Vec<Definition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseOutput {
    pub candidates: Vec<CandidateDto>,
    pub lookup_misses: usize,
    pub filtered_stopwords: usize,
    pub truncated_candidates: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureParsedInput {
    pub text: String,
    pub candidate_ids: Vec<String>,
    pub source: String,
    pub captured_at_ms: i64,
    #[serde(default)]
    pub include_stopwords: bool,
    #[serde(default = "default_max_candidates")]
    pub max_candidates: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureOutput {
    pub captured_cards: usize,
    pub created_cards: usize,
    pub created_contexts: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DueReviewsInput {
    pub as_of_ms: i64,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DueReviewsOutput {
    pub cards: Vec<ReviewCardDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewCardDto {
    pub word_id: String,
    pub term: String,
    pub lemma: String,
    pub phonetic: String,
    pub definition: String,
    pub contexts: Vec<ContextDto>,
    pub ease_factor: f64,
    pub repetitions: u32,
    pub interval_days: u32,
    pub next_review_at_ms: i64,
    pub review_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextDto {
    pub sentence: String,
    pub source: String,
    pub captured_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewInput {
    pub word_id: String,
    pub review_token: String,
    pub rating: u8,
    pub reviewed_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewOutput {
    pub ease_factor: f64,
    pub repetitions: u32,
    pub interval_days: u32,
    pub next_review_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsInput {
    pub as_of_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntervalDistributionDto {
    pub new: u64,
    pub days_1_to_6: u64,
    pub days_7_to_30: u64,
    pub days_31_to_90: u64,
    pub days_91_plus: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsOutput {
    pub total_cards: u64,
    pub due_cards: u64,
    pub intervals: IntervalDistributionDto,
    pub next_review_at_ms: Option<i64>,
}

#[derive(Debug, Error)]
pub enum SandboxError {
    #[error("text cannot be empty")]
    EmptyText,
    #[error("timestamp {0} is outside the supported range")]
    InvalidTimestamp(i64),
    #[error("parser produced invalid UTF-8 boundary {0}")]
    InvalidTextBoundary(usize),
    #[error("candidate {0} is not present in the parsed text")]
    UnknownCandidate(String),
    #[error("candidate {0} was selected more than once")]
    DuplicateCandidate(String),
    #[error("rating {0} is outside the inclusive range 0 through 5")]
    InvalidRating(u8),
    #[error("review state changed; refresh the due queue")]
    ReviewConflict,
    #[error("sandbox serialization failed: {0}")]
    Serialization(String),
    #[error(transparent)]
    Lexicon(#[from] BrowserLexiconError),
    #[error(transparent)]
    Storage(#[from] SnapshotError),
    #[error(transparent)]
    Core(#[from] core_engine::CoreError),
}
