use chrono::{DateTime, Utc};
use core_engine::{PodcastContextDraft, PodcastStorageAdapter};
use language_engine::{
    podcast_capture_from_entry, BrowserLexicon, BrowserLexiconError, Definition, LexiconEntry,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{SnapshotAccess, SnapshotBackend, SnapshotError, SnapshotStorage};

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
    #[error("podcast capture is invalid: {0}")]
    InvalidCapture(String),
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
}
