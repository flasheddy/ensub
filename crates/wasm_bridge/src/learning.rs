use chrono::{DateTime, Utc};
use core_engine::{
    schedule_review, PodcastContextDraft, PodcastStorageAdapter, ReviewQueueStorageAdapter,
    ReviewRating, ReviewUpdate, StorageAdapter, WordId,
};
use language_engine::{
    podcast_capture_from_entry, prepare_disambiguation, validate_disambiguation_response,
    BrowserLexicon, BrowserLexiconError, Definition, DisambiguationError, LexiconEntry,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::review::{prompt_from_item, review_token};
use crate::{
    DueCountDto, DueCountInputDto, DueReviewsDto, DueReviewsInputDto,
    PrepareDisambiguationInputDto, PreparedDisambiguationDto, RateReviewInputDto,
    RevealReviewInputDto, ReviewAnswerDto, ReviewTransitionDto, SnapshotAccess, SnapshotBackend,
    SnapshotError, SnapshotMigrationStatus, SnapshotStorage,
    ValidateDisambiguationResponseInputDto,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LookupDefinitionDto {
    pub part_of_speech: String,
    pub text: String,
}

impl From<Definition> for LookupDefinitionDto {
    fn from(value: Definition) -> Self {
        Self {
            part_of_speech: value.part_of_speech,
            text: value.text,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LookupEntryDto {
    pub lemma: String,
    pub phonetic: String,
    pub definitions: Vec<LookupDefinitionDto>,
}

impl From<LexiconEntry> for LookupEntryDto {
    fn from(value: LexiconEntry) -> Self {
        Self {
            lemma: value.lemma,
            phonetic: value.phonetic,
            definitions: value.definitions.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum TokenLookupDto {
    Found {
        surface: String,
        entry: LookupEntryDto,
    },
    Ambiguous {
        surface: String,
        entries: Vec<LookupEntryDto>,
    },
    Unknown {
        surface: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapturePodcastInput {
    pub draft: PodcastContextDraft,
    pub selected_lemma: Option<String>,
    pub captured_at_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapturePodcastStatus {
    CreatedCard,
    AddedEncounter,
    AlreadyCaptured,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapturePodcastOutput {
    pub status: CapturePodcastStatus,
    pub word_id: String,
    pub context_id: String,
}

#[derive(Debug, Error)]
pub enum PlayerLearningError {
    #[error("offline lexicon is invalid: {0}")]
    Lexicon(#[from] BrowserLexiconError),
    #[error("selected token is not present in the offline lexicon")]
    UnknownToken,
    #[error("lookup is ambiguous; select one of the returned lemmas")]
    AmbiguousToken,
    #[error("selected lemma is not one of the offline lookup results")]
    InvalidLemma,
    #[error("capture timestamp is invalid")]
    InvalidTimestamp,
    #[error("review queue limit {0} is outside the inclusive range 1 through 50")]
    InvalidReviewLimit(u32),
    #[error("rating {0} is outside the inclusive range 0 through 5")]
    InvalidRating(u8),
    #[error("review card is unavailable")]
    ReviewNotFound,
    #[error("review state changed; refresh the due queue")]
    ReviewConflict,
    #[error("review token serialization failed: {0}")]
    ReviewToken(String),
    #[error("podcast capture is invalid: {0}")]
    InvalidCapture(String),
    #[error(transparent)]
    Disambiguation(#[from] DisambiguationError),
    #[error(transparent)]
    Core(#[from] core_engine::CoreError),
    #[error(transparent)]
    Storage(#[from] SnapshotError),
}

pub struct PlayerLearning<B> {
    lexicon: BrowserLexicon,
    storage: SnapshotStorage<B>,
}

impl<B: SnapshotBackend> PlayerLearning<B> {
    pub fn open(
        backend: B,
        storage_key: impl Into<String>,
        access: SnapshotAccess,
        lexicon_bytes: &[u8],
    ) -> Result<Self, PlayerLearningError> {
        Ok(Self {
            lexicon: BrowserLexicon::decode(lexicon_bytes)?,
            storage: SnapshotStorage::new(backend, storage_key, access),
        })
    }

    pub fn lookup_token(&self, surface: &str) -> TokenLookupDto {
        let entries = self.lexicon.lookup_entries(surface);
        match entries.as_slice() {
            [] => TokenLookupDto::Unknown {
                surface: surface.to_string(),
            },
            [entry] => TokenLookupDto::Found {
                surface: surface.to_string(),
                entry: entry.clone().into(),
            },
            _ => TokenLookupDto::Ambiguous {
                surface: surface.to_string(),
                entries: entries.into_iter().map(Into::into).collect(),
            },
        }
    }

    pub fn initialize_storage(&mut self) -> Result<SnapshotMigrationStatus, PlayerLearningError> {
        self.storage.initialize().map_err(Into::into)
    }

    pub fn is_read_only(&self) -> bool {
        self.storage.is_read_only()
    }

    pub fn raw_snapshot(&self) -> Result<Option<String>, PlayerLearningError> {
        self.storage.raw_snapshot().map_err(Into::into)
    }

    pub fn reset(&mut self) -> Result<(), PlayerLearningError> {
        self.storage.reset().map_err(Into::into)
    }

    pub fn capture_podcast(
        &mut self,
        input: CapturePodcastInput,
    ) -> Result<CapturePodcastOutput, PlayerLearningError> {
        let entries = self
            .lexicon
            .lookup_entries(input.draft.selected_token.surface());
        let entry = match (entries.len(), input.selected_lemma.as_deref()) {
            (0, _) => return Err(PlayerLearningError::UnknownToken),
            (1, None) => entries.into_iter().next(),
            (1, Some(lemma)) => entries.into_iter().find(|entry| entry.lemma == lemma),
            (_, None) => return Err(PlayerLearningError::AmbiguousToken),
            (_, Some(lemma)) => entries.into_iter().find(|entry| entry.lemma == lemma),
        }
        .ok_or(PlayerLearningError::InvalidLemma)?;
        let captured_at = DateTime::<Utc>::from_timestamp_millis(input.captured_at_ms)
            .ok_or(PlayerLearningError::InvalidTimestamp)?;
        let capture = podcast_capture_from_entry(input.draft, entry, captured_at)
            .map_err(|error| PlayerLearningError::InvalidCapture(error.to_string()))?;
        let result = self.storage.save_podcast_capture(&capture)?;
        let status = if result.word_created {
            CapturePodcastStatus::CreatedCard
        } else if result.podcast_context_created {
            CapturePodcastStatus::AddedEncounter
        } else {
            CapturePodcastStatus::AlreadyCaptured
        };
        Ok(CapturePodcastOutput {
            status,
            word_id: capture.capture.word.id.as_str().to_string(),
            context_id: capture.podcast_context.context_id.as_str().to_string(),
        })
    }

    pub fn due_count(&self, input: DueCountInputDto) -> Result<DueCountDto, PlayerLearningError> {
        let as_of = learning_timestamp(input.as_of_ms)?;
        Ok(DueCountDto {
            due_count: self.storage.due_count(as_of)?,
        })
    }

    pub fn due_reviews(
        &self,
        input: DueReviewsInputDto,
    ) -> Result<DueReviewsDto, PlayerLearningError> {
        if !(1..=50).contains(&input.limit) {
            return Err(PlayerLearningError::InvalidReviewLimit(input.limit));
        }
        let as_of = learning_timestamp(input.as_of_ms)?;
        let cards = self
            .storage
            .due_review_queue(as_of, input.limit)?
            .into_iter()
            .map(prompt_from_item)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| PlayerLearningError::ReviewToken(error.to_string()))?;
        Ok(DueReviewsDto {
            as_of_ms: input.as_of_ms,
            cards,
        })
    }

    pub fn reveal_review(
        &self,
        input: RevealReviewInputDto,
    ) -> Result<ReviewAnswerDto, PlayerLearningError> {
        let item = self
            .storage
            .review_item(&WordId::new(&input.word_id))?
            .ok_or(PlayerLearningError::ReviewNotFound)?;
        let current_token = review_token(&item.card.state)
            .map_err(|error| PlayerLearningError::ReviewToken(error.to_string()))?;
        if current_token != input.review_token {
            return Err(PlayerLearningError::ReviewConflict);
        }
        Ok(ReviewAnswerDto {
            word_id: item.card.word.id.into_inner(),
            review_token: current_token,
            term: item.card.word.term,
            lemma: item.card.word.lemma,
            phonetic: item.card.word.phonetic,
            definition: item.card.word.definition,
        })
    }

    pub fn review(
        &mut self,
        input: RateReviewInputDto,
    ) -> Result<ReviewTransitionDto, PlayerLearningError> {
        let rating = ReviewRating::try_from(input.rating)
            .map_err(|_| PlayerLearningError::InvalidRating(input.rating))?;
        let reviewed_at = learning_timestamp(input.reviewed_at_ms)?;
        let word_id = WordId::new(&input.word_id);
        let current = self
            .storage
            .review_state(&word_id)?
            .ok_or(PlayerLearningError::ReviewNotFound)?;
        let current_token = review_token(&current)
            .map_err(|error| PlayerLearningError::ReviewToken(error.to_string()))?;
        if current_token != input.review_token {
            return Err(PlayerLearningError::ReviewConflict);
        }
        let replacement = schedule_review(&current, rating, reviewed_at)?;
        if self
            .storage
            .commit_review(&current, &replacement, reviewed_at)?
            == ReviewUpdate::Conflict
        {
            return Err(PlayerLearningError::ReviewConflict);
        }
        Ok(ReviewTransitionDto {
            word_id: input.word_id,
            rating: input.rating,
            reviewed_at_ms: input.reviewed_at_ms,
            ease_factor: replacement.ease_factor,
            repetitions: replacement.repetitions,
            interval_days: replacement.interval_days,
            next_review_at_ms: replacement.next_review_at.timestamp_millis(),
        })
    }

    pub fn prepare_disambiguation(
        &self,
        input: PrepareDisambiguationInputDto,
    ) -> Result<PreparedDisambiguationDto, PlayerLearningError> {
        let entries = self
            .lexicon
            .lookup_entries(input.draft.selected_token.surface());
        prepare_disambiguation(
            input.draft.selected_token.surface(),
            &input.draft.sentence,
            &entries,
            &input.draft.episode.title,
        )
        .map(Into::into)
        .map_err(Into::into)
    }

    pub fn validate_disambiguation_response(
        &self,
        input: ValidateDisambiguationResponseInputDto,
    ) -> Result<language_engine::DisambiguationResponse, PlayerLearningError> {
        validate_disambiguation_response(&input.request, &input.response_json).map_err(Into::into)
    }
}

fn learning_timestamp(milliseconds: i64) -> Result<DateTime<Utc>, PlayerLearningError> {
    DateTime::from_timestamp_millis(milliseconds).ok_or(PlayerLearningError::InvalidTimestamp)
}
