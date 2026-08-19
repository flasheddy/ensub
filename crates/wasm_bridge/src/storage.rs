use std::collections::{BTreeMap, HashSet};
use std::error::Error;

use chrono::{DateTime, Utc};
use core_engine::{
    Capture, CaptureResult, ContextId, ContextRecord, IntervalDistribution, PodcastCapture,
    PodcastCaptureResult, PodcastContextRecord, PodcastStorageAdapter, ReviewCard, ReviewQueueItem,
    ReviewQueueStorageAdapter, ReviewRating, ReviewState, ReviewStatistics, ReviewUpdate,
    StorageAdapter, WordId, WordRecord, MIN_EASE_FACTOR,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

const SNAPSHOT_FORMAT: &str = "ensub-browser-storage";
pub const SNAPSHOT_SCHEMA_VERSION: u16 = 2;
pub const V01_BACKUP_STORAGE_KEY: &str = "ensub_v0.1_backup";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotAccess {
    ReadOnly,
    ReadWrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotMigrationStatus {
    Empty,
    Current,
    Migrated,
}

pub trait SnapshotBackend {
    type Error: Error + 'static;

    fn load(&self, key: &str) -> Result<Option<String>, Self::Error>;
    fn store(&mut self, key: &str, value: &str) -> Result<(), Self::Error>;
    fn remove(&mut self, key: &str) -> Result<(), Self::Error>;
}

#[cfg(target_arch = "wasm32")]
#[derive(Debug, Clone)]
pub struct LocalStorageBackend {
    storage: web_sys::Storage,
}

#[cfg(target_arch = "wasm32")]
impl LocalStorageBackend {
    pub fn open() -> Result<Self, LocalStorageBackendError> {
        let window = web_sys::window().ok_or_else(|| LocalStorageBackendError {
            message: "window is unavailable".to_string(),
        })?;
        let storage = window
            .local_storage()
            .map_err(LocalStorageBackendError::from_js)?
            .ok_or_else(|| LocalStorageBackendError {
                message: "localStorage is unavailable".to_string(),
            })?;
        Ok(Self { storage })
    }

    pub fn from_storage(storage: web_sys::Storage) -> Self {
        Self { storage }
    }
}

#[cfg(target_arch = "wasm32")]
impl SnapshotBackend for LocalStorageBackend {
    type Error = LocalStorageBackendError;

    fn load(&self, key: &str) -> Result<Option<String>, Self::Error> {
        self.storage
            .get_item(key)
            .map_err(LocalStorageBackendError::from_js)
    }

    fn store(&mut self, key: &str, value: &str) -> Result<(), Self::Error> {
        self.storage
            .set_item(key, value)
            .map_err(LocalStorageBackendError::from_js)
    }

    fn remove(&mut self, key: &str) -> Result<(), Self::Error> {
        self.storage
            .remove_item(key)
            .map_err(LocalStorageBackendError::from_js)
    }
}

#[cfg(target_arch = "wasm32")]
#[derive(Debug, Error)]
#[error("{message}")]
pub struct LocalStorageBackendError {
    message: String,
}

#[cfg(target_arch = "wasm32")]
impl LocalStorageBackendError {
    fn from_js(value: wasm_bindgen::JsValue) -> Self {
        let message = value
            .dyn_ref::<js_sys::Error>()
            .map(|error| format!("{}: {}", error.name(), error.message()))
            .unwrap_or_else(|| format!("{value:?}"));
        Self { message }
    }
}

pub struct SnapshotStorage<B> {
    backend: B,
    key: String,
    access: SnapshotAccess,
    initialized: bool,
}

impl<B> SnapshotStorage<B> {
    pub fn new(backend: B, key: impl Into<String>, access: SnapshotAccess) -> Self {
        Self {
            backend,
            key: key.into(),
            access,
            initialized: false,
        }
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }

    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    pub fn into_backend(self) -> B {
        self.backend
    }
}

impl<B: SnapshotBackend> SnapshotStorage<B> {
    pub fn initialize(&mut self) -> Result<SnapshotMigrationStatus, SnapshotError> {
        if self.initialized {
            return Ok(SnapshotMigrationStatus::Current);
        }
        let result = self.initialize_inner();
        if result.is_err() {
            self.access = SnapshotAccess::ReadOnly;
        }
        result
    }

    pub fn is_read_only(&self) -> bool {
        self.access == SnapshotAccess::ReadOnly
    }

    pub fn raw_snapshot(&self) -> Result<Option<String>, SnapshotError> {
        if let Some(raw) = self
            .backend
            .load(&self.key)
            .map_err(|error| SnapshotError::backend("load", error))?
        {
            return Ok(Some(raw));
        }
        self.backend
            .load(V01_BACKUP_STORAGE_KEY)
            .map_err(|error| SnapshotError::backend("load backup", error))
    }

    fn initialize_inner(&mut self) -> Result<SnapshotMigrationStatus, SnapshotError> {
        let Some(raw) = self
            .backend
            .load(&self.key)
            .map_err(|error| SnapshotError::backend("load", error))?
        else {
            self.initialized = true;
            return Ok(SnapshotMigrationStatus::Empty);
        };
        let header: SnapshotHeader = serde_json::from_str(&raw)
            .map_err(|error| SnapshotError::CorruptSnapshot(error.to_string()))?;
        if header.format != SNAPSHOT_FORMAT {
            return Err(SnapshotError::CorruptSnapshot(
                "snapshot format identifier is invalid".to_string(),
            ));
        }
        match header.schema_version {
            1 => {
                self.ensure_writable()?;
                let legacy = serde_json::from_str::<SnapshotV1>(&raw)
                    .map_err(|error| SnapshotError::CorruptSnapshot(error.to_string()))?;
                let mut migrated = SnapshotV2::from(legacy);
                validate_snapshot(&migrated)?;
                migrated.revision = migrated.revision.saturating_add(1);
                let encoded = serde_json::to_string(&migrated)
                    .map_err(|error| SnapshotError::Serialization(error.to_string()))?;
                match self
                    .backend
                    .load(V01_BACKUP_STORAGE_KEY)
                    .map_err(|error| SnapshotError::backend("load backup", error))?
                {
                    Some(backup) if backup != raw => return Err(SnapshotError::BackupConflict),
                    Some(_) => {}
                    None => self
                        .backend
                        .store(V01_BACKUP_STORAGE_KEY, &raw)
                        .map_err(|error| SnapshotError::backend("store backup", error))?,
                }
                self.backend
                    .store(&self.key, &encoded)
                    .map_err(|error| SnapshotError::backend("store migrated snapshot", error))?;
                self.initialized = true;
                Ok(SnapshotMigrationStatus::Migrated)
            }
            SNAPSHOT_SCHEMA_VERSION => {
                let snapshot = serde_json::from_str::<SnapshotV2>(&raw)
                    .map_err(|error| SnapshotError::CorruptSnapshot(error.to_string()))?;
                validate_snapshot(&snapshot)?;
                self.initialized = true;
                Ok(SnapshotMigrationStatus::Current)
            }
            actual => Err(SnapshotError::UnsupportedSchema {
                expected: SNAPSHOT_SCHEMA_VERSION,
                actual,
            }),
        }
    }

    pub fn reset(&mut self) -> Result<(), SnapshotError> {
        self.ensure_ready_for_write()?;
        self.backend
            .remove(&self.key)
            .map_err(|error| SnapshotError::backend("remove", error))?;
        self.backend
            .remove(V01_BACKUP_STORAGE_KEY)
            .map_err(|error| SnapshotError::backend("remove backup", error))
    }

    pub fn next_review_at_after(
        &self,
        after: DateTime<Utc>,
    ) -> Result<Option<DateTime<Utc>>, SnapshotError> {
        let snapshot = self.load_snapshot()?;
        snapshot
            .review_states
            .values()
            .map(|state| decode_timestamp(state.next_review_at))
            .collect::<Result<Vec<_>, _>>()
            .map(|dates| dates.into_iter().filter(|date| *date > after).min())
    }

    fn ensure_writable(&self) -> Result<(), SnapshotError> {
        if self.access == SnapshotAccess::ReadOnly {
            Err(SnapshotError::ReadOnly)
        } else {
            Ok(())
        }
    }

    fn ensure_ready_for_write(&mut self) -> Result<(), SnapshotError> {
        self.ensure_writable()?;
        if !self.initialized {
            self.initialize()?;
        }
        self.ensure_writable()
    }

    fn load_snapshot(&self) -> Result<SnapshotV2, SnapshotError> {
        let Some(raw) = self
            .backend
            .load(&self.key)
            .map_err(|error| SnapshotError::backend("load", error))?
        else {
            return Ok(SnapshotV2::empty());
        };
        let header: SnapshotHeader = serde_json::from_str(&raw)
            .map_err(|error| SnapshotError::CorruptSnapshot(error.to_string()))?;
        if header.format != SNAPSHOT_FORMAT {
            return Err(SnapshotError::CorruptSnapshot(
                "snapshot format identifier is invalid".to_string(),
            ));
        }
        let snapshot = match header.schema_version {
            1 => serde_json::from_str::<SnapshotV1>(&raw)
                .map(SnapshotV2::from)
                .map_err(|error| SnapshotError::CorruptSnapshot(error.to_string()))?,
            SNAPSHOT_SCHEMA_VERSION => serde_json::from_str::<SnapshotV2>(&raw)
                .map_err(|error| SnapshotError::CorruptSnapshot(error.to_string()))?,
            actual => {
                return Err(SnapshotError::UnsupportedSchema {
                    expected: SNAPSHOT_SCHEMA_VERSION,
                    actual,
                })
            }
        };
        validate_snapshot(&snapshot)?;
        Ok(snapshot)
    }

    fn store_snapshot(&mut self, mut snapshot: SnapshotV2) -> Result<(), SnapshotError> {
        self.ensure_writable()?;
        validate_snapshot(&snapshot)?;
        snapshot.revision = snapshot.revision.saturating_add(1);
        let encoded = serde_json::to_string(&snapshot)
            .map_err(|error| SnapshotError::Serialization(error.to_string()))?;
        self.backend
            .store(&self.key, &encoded)
            .map_err(|error| SnapshotError::backend("store", error))
    }

    fn mutate<T>(
        &mut self,
        operation: impl FnOnce(&mut SnapshotV2) -> Result<T, SnapshotError>,
    ) -> Result<T, SnapshotError> {
        self.ensure_ready_for_write()?;
        let mut snapshot = self.load_snapshot()?;
        let result = operation(&mut snapshot)?;
        self.store_snapshot(snapshot)?;
        Ok(result)
    }
}

impl<B: SnapshotBackend> StorageAdapter for SnapshotStorage<B> {
    type Error = SnapshotError;

    fn save_word(&mut self, word: &WordRecord) -> Result<(), Self::Error> {
        let stored = StoredWordV1::from(word);
        self.mutate(|snapshot| {
            ensure_lemma_available(snapshot, word.id.as_str(), &word.lemma)?;
            snapshot.words.insert(word.id.as_str().to_string(), stored);
            Ok(())
        })
    }

    fn save_context(&mut self, context: &ContextRecord) -> Result<(), Self::Error> {
        let stored = StoredContextV1::from(context);
        self.mutate(|snapshot| {
            ensure_word_exists(snapshot, context.word_id.as_str())?;
            snapshot
                .contexts
                .insert(context.id.as_str().to_string(), stored);
            Ok(())
        })
    }

    fn save_review_state(&mut self, state: &ReviewState) -> Result<(), Self::Error> {
        validate_review_state(state)?;
        let stored = StoredReviewStateV1::from(state);
        self.mutate(|snapshot| {
            ensure_word_exists(snapshot, state.word_id.as_str())?;
            snapshot
                .review_states
                .insert(state.word_id.as_str().to_string(), stored);
            Ok(())
        })
    }

    fn save_capture(&mut self, capture: &Capture) -> Result<CaptureResult, Self::Error> {
        let mut results = self.save_captures(std::slice::from_ref(capture))?;
        results.pop().ok_or(SnapshotError::InvalidCapture)
    }

    fn save_captures(&mut self, captures: &[Capture]) -> Result<Vec<CaptureResult>, Self::Error> {
        if captures.is_empty() {
            return Ok(Vec::new());
        }
        for capture in captures {
            validate_capture(capture)?;
        }
        self.mutate(|snapshot| {
            let mut results = Vec::with_capacity(captures.len());
            for capture in captures {
                ensure_lemma_available(snapshot, capture.word.id.as_str(), &capture.word.lemma)?;
                let word_created = match snapshot.words.get_mut(capture.word.id.as_str()) {
                    Some(existing) => {
                        existing.lemma = capture.word.lemma.clone();
                        existing.phonetic = capture.word.phonetic.clone();
                        existing.definition = capture.word.definition.clone();
                        false
                    }
                    None => {
                        snapshot.words.insert(
                            capture.word.id.as_str().to_string(),
                            StoredWordV1::from(&capture.word),
                        );
                        true
                    }
                };
                let mut contexts_created = 0_u64;
                for context in &capture.contexts {
                    if !snapshot.contexts.contains_key(context.id.as_str()) {
                        snapshot.contexts.insert(
                            context.id.as_str().to_string(),
                            StoredContextV1::from(context),
                        );
                        contexts_created = contexts_created.saturating_add(1);
                    }
                }
                snapshot
                    .review_states
                    .entry(capture.word.id.as_str().to_string())
                    .or_insert_with(|| StoredReviewStateV1::from(&capture.initial_review_state));
                results.push(CaptureResult {
                    word_created,
                    contexts_created,
                });
            }
            Ok(results)
        })
    }

    fn compare_and_swap_review_state(
        &mut self,
        expected: &ReviewState,
        replacement: &ReviewState,
    ) -> Result<ReviewUpdate, Self::Error> {
        self.ensure_ready_for_write()?;
        if expected.word_id != replacement.word_id {
            return Err(SnapshotError::InvalidReviewReplacement);
        }
        validate_review_state(replacement)?;
        let mut snapshot = self.load_snapshot()?;
        let stored_expected = StoredReviewStateV1::from(expected);
        if snapshot.review_states.get(expected.word_id.as_str()) != Some(&stored_expected) {
            return Ok(ReviewUpdate::Conflict);
        }
        snapshot.review_states.insert(
            replacement.word_id.as_str().to_string(),
            StoredReviewStateV1::from(replacement),
        );
        self.store_snapshot(snapshot)?;
        Ok(ReviewUpdate::Updated)
    }

    fn review_state(&self, word_id: &WordId) -> Result<Option<ReviewState>, Self::Error> {
        self.load_snapshot()?
            .review_states
            .get(word_id.as_str())
            .map(|stored| stored.to_domain(word_id.clone()))
            .transpose()
    }

    fn due_reviews(&self, as_of: DateTime<Utc>) -> Result<Vec<ReviewCard>, Self::Error> {
        let snapshot = self.load_snapshot()?;
        let mut cards = Vec::new();
        for (word_id, stored_state) in &snapshot.review_states {
            let state = stored_state.to_domain(WordId::new(word_id))?;
            if state.next_review_at > as_of {
                continue;
            }
            let stored_word = snapshot
                .words
                .get(word_id)
                .ok_or_else(|| SnapshotError::MissingWord(word_id.clone()))?;
            let word = stored_word.to_domain(WordId::new(word_id))?;
            let mut contexts = snapshot
                .contexts
                .values()
                .filter(|context| context.word_id == *word_id)
                .map(|context| context.to_domain())
                .collect::<Result<Vec<_>, _>>()?;
            contexts.sort_by(|left, right| {
                right
                    .captured_at
                    .cmp(&left.captured_at)
                    .then_with(|| left.id.as_str().cmp(right.id.as_str()))
            });
            cards.push(ReviewCard {
                word,
                contexts,
                state,
            });
        }
        cards.sort_by(|left, right| {
            left.state
                .next_review_at
                .cmp(&right.state.next_review_at)
                .then_with(|| left.word.id.as_str().cmp(right.word.id.as_str()))
        });
        Ok(cards)
    }

    fn due_count(&self, as_of: DateTime<Utc>) -> Result<u64, Self::Error> {
        let snapshot = self.load_snapshot()?;
        let mut count = 0_u64;
        for state in snapshot.review_states.values() {
            if decode_timestamp(state.next_review_at)? <= as_of {
                count = count.saturating_add(1);
            }
        }
        Ok(count)
    }

    fn review_statistics(&self, as_of: DateTime<Utc>) -> Result<ReviewStatistics, Self::Error> {
        let snapshot = self.load_snapshot()?;
        let mut statistics = ReviewStatistics::default();
        for stored in snapshot.review_states.values() {
            statistics.total_cards = statistics.total_cards.saturating_add(1);
            if decode_timestamp(stored.next_review_at)? <= as_of {
                statistics.due_cards = statistics.due_cards.saturating_add(1);
            }
            increment_interval(&mut statistics.intervals, stored.interval_days);
        }
        Ok(statistics)
    }
}

impl<B: SnapshotBackend> PodcastStorageAdapter for SnapshotStorage<B> {
    fn save_podcast_capture(
        &mut self,
        capture: &PodcastCapture,
    ) -> Result<PodcastCaptureResult, Self::Error> {
        let generic_context = validate_podcast_capture(capture)?;
        let context_id = capture.podcast_context.context_id.as_str().to_string();
        self.mutate(|snapshot| {
            ensure_lemma_available(
                snapshot,
                capture.capture.word.id.as_str(),
                &capture.capture.word.lemma,
            )?;
            if let Some(existing) = snapshot.podcast_contexts.get_mut(&context_id) {
                let mut comparable = capture.podcast_context.clone();
                comparable.context.captured_at = existing.context.captured_at;
                comparable.context.playback_position_ms = existing.context.playback_position_ms;
                match (
                    existing.context.episode.publisher_guid.as_ref(),
                    comparable.context.episode.publisher_guid.as_ref(),
                ) {
                    (None, Some(guid)) => {
                        existing.context.episode.publisher_guid = Some(guid.clone());
                    }
                    (Some(guid), None) => {
                        comparable.context.episode.publisher_guid = Some(guid.clone());
                    }
                    _ => {}
                }
                if existing != &comparable {
                    return Err(SnapshotError::PodcastContextConflict(context_id));
                }
            }
            if let Some(existing) = snapshot.contexts.get(&context_id) {
                let mut comparable = StoredContextV1::from(generic_context);
                comparable.captured_at = existing.captured_at;
                if existing != &comparable {
                    return Err(SnapshotError::PodcastContextConflict(context_id));
                }
            }

            let word_created = match snapshot.words.get_mut(capture.capture.word.id.as_str()) {
                Some(existing) => {
                    existing.lemma = capture.capture.word.lemma.clone();
                    existing.phonetic = capture.capture.word.phonetic.clone();
                    existing.definition = capture.capture.word.definition.clone();
                    false
                }
                None => {
                    snapshot.words.insert(
                        capture.capture.word.id.as_str().to_string(),
                        StoredWordV1::from(&capture.capture.word),
                    );
                    true
                }
            };
            let context_created = !snapshot.contexts.contains_key(&context_id);
            if context_created {
                snapshot
                    .contexts
                    .insert(context_id.clone(), StoredContextV1::from(generic_context));
            }
            let podcast_context_created = !snapshot.podcast_contexts.contains_key(&context_id);
            if podcast_context_created {
                snapshot
                    .podcast_contexts
                    .insert(context_id, capture.podcast_context.clone());
            }
            snapshot
                .review_states
                .entry(capture.capture.word.id.as_str().to_string())
                .or_insert_with(|| {
                    StoredReviewStateV1::from(&capture.capture.initial_review_state)
                });
            Ok(PodcastCaptureResult {
                word_created,
                contexts_created: u64::from(context_created),
                podcast_context_created,
            })
        })
    }

    fn podcast_contexts(&self, word_id: &WordId) -> Result<Vec<PodcastContextRecord>, Self::Error> {
        let snapshot = self.load_snapshot()?;
        let mut contexts = snapshot
            .podcast_contexts
            .values()
            .filter(|record| record.word_id == *word_id)
            .cloned()
            .collect::<Vec<_>>();
        contexts.sort_by(|left, right| {
            right
                .context
                .captured_at
                .cmp(&left.context.captured_at)
                .then_with(|| left.context_id.as_str().cmp(right.context_id.as_str()))
        });
        Ok(contexts)
    }
}

impl<B: SnapshotBackend> ReviewQueueStorageAdapter for SnapshotStorage<B> {
    fn due_review_queue(
        &self,
        as_of: DateTime<Utc>,
        limit: u32,
    ) -> Result<Vec<ReviewQueueItem>, Self::Error> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let snapshot = self.load_snapshot()?;
        let mut items = Vec::new();
        for (word_id, stored_state) in &snapshot.review_states {
            if decode_timestamp(stored_state.next_review_at)? > as_of {
                continue;
            }
            if let Some(item) = review_queue_item(&snapshot, word_id)? {
                items.push(item);
            }
        }
        items.sort_by(|left, right| {
            left.card
                .state
                .next_review_at
                .cmp(&right.card.state.next_review_at)
                .then_with(|| left.card.word.id.as_str().cmp(right.card.word.id.as_str()))
        });
        items.truncate(limit as usize);
        Ok(items)
    }

    fn review_item(&self, word_id: &WordId) -> Result<Option<ReviewQueueItem>, Self::Error> {
        let snapshot = self.load_snapshot()?;
        review_queue_item(&snapshot, word_id.as_str())
    }
}

fn review_queue_item(
    snapshot: &SnapshotV2,
    word_id: &str,
) -> Result<Option<ReviewQueueItem>, SnapshotError> {
    let Some(stored_state) = snapshot.review_states.get(word_id) else {
        return Ok(None);
    };
    let stored_word = snapshot
        .words
        .get(word_id)
        .ok_or_else(|| SnapshotError::MissingWord(word_id.to_string()))?;
    let mut contexts = snapshot
        .contexts
        .values()
        .filter(|context| context.word_id == word_id)
        .map(StoredContextV1::to_domain)
        .collect::<Result<Vec<_>, _>>()?;
    contexts.sort_by(|left, right| {
        right
            .captured_at
            .cmp(&left.captured_at)
            .then_with(|| left.id.as_str().cmp(right.id.as_str()))
    });
    let mut podcast_contexts = snapshot
        .podcast_contexts
        .values()
        .filter(|record| record.word_id.as_str() == word_id)
        .cloned()
        .collect::<Vec<_>>();
    podcast_contexts.sort_by(|left, right| {
        right
            .context
            .captured_at
            .cmp(&left.context.captured_at)
            .then_with(|| left.context_id.as_str().cmp(right.context_id.as_str()))
    });
    Ok(Some(ReviewQueueItem {
        card: ReviewCard {
            word: stored_word.to_domain(WordId::new(word_id))?,
            contexts,
            state: stored_state.to_domain(WordId::new(word_id))?,
        },
        podcast_contexts,
    }))
}

#[derive(Debug, Error)]
pub enum SnapshotError {
    #[error("browser storage is read-only in this tab")]
    ReadOnly,

    #[error("browser storage {operation} failed: {message}")]
    Backend {
        operation: &'static str,
        message: String,
    },

    #[error("the existing v0.1 backup does not match the active snapshot")]
    BackupConflict,

    #[error("browser snapshot is corrupt: {0}")]
    CorruptSnapshot(String),

    #[error("browser snapshot schema {actual} is unsupported; expected {expected}")]
    UnsupportedSchema { expected: u16, actual: u16 },

    #[error("browser snapshot serialization failed: {0}")]
    Serialization(String),

    #[error("capture records do not refer to the same word")]
    InvalidCapture,

    #[error("podcast capture records are inconsistent")]
    InvalidPodcastCapture,

    #[error("podcast context {0} conflicts with an existing encounter")]
    PodcastContextConflict(String),

    #[error("review replacement refers to a different word")]
    InvalidReviewReplacement,

    #[error("browser snapshot refers to missing word {0}")]
    MissingWord(String),

    #[error("browser snapshot refers to missing context {0}")]
    MissingContext(String),

    #[error("browser snapshot contains duplicate lemma {0}")]
    DuplicateLemma(String),

    #[error("review state has invalid ease factor {0}")]
    InvalidEaseFactor(f64),

    #[error("browser snapshot contains invalid timestamp {0}")]
    InvalidTimestamp(i64),

    #[error("browser snapshot contains invalid rating {0}")]
    InvalidRating(u8),
}

impl SnapshotError {
    fn backend(operation: &'static str, error: impl Error) -> Self {
        Self::Backend {
            operation,
            message: error.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SnapshotHeader {
    format: String,
    schema_version: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SnapshotV1 {
    format: String,
    schema_version: u16,
    revision: u64,
    words: BTreeMap<String, StoredWordV1>,
    contexts: BTreeMap<String, StoredContextV1>,
    review_states: BTreeMap<String, StoredReviewStateV1>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SnapshotV2 {
    format: String,
    schema_version: u16,
    revision: u64,
    words: BTreeMap<String, StoredWordV1>,
    contexts: BTreeMap<String, StoredContextV1>,
    review_states: BTreeMap<String, StoredReviewStateV1>,
    podcast_contexts: BTreeMap<String, PodcastContextRecord>,
}

impl From<SnapshotV1> for SnapshotV2 {
    fn from(value: SnapshotV1) -> Self {
        Self {
            format: value.format,
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            revision: value.revision,
            words: value.words,
            contexts: value.contexts,
            review_states: value.review_states,
            podcast_contexts: BTreeMap::new(),
        }
    }
}

impl SnapshotV2 {
    fn empty() -> Self {
        Self {
            format: SNAPSHOT_FORMAT.to_string(),
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            revision: 0,
            words: BTreeMap::new(),
            contexts: BTreeMap::new(),
            review_states: BTreeMap::new(),
            podcast_contexts: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredWordV1 {
    id: String,
    term: String,
    lemma: String,
    phonetic: String,
    definition: String,
    created_at: i64,
}

impl From<&WordRecord> for StoredWordV1 {
    fn from(word: &WordRecord) -> Self {
        Self {
            id: word.id.as_str().to_string(),
            term: word.term.clone(),
            lemma: word.lemma.clone(),
            phonetic: word.phonetic.clone(),
            definition: word.definition.clone(),
            created_at: word.created_at.timestamp_millis(),
        }
    }
}

impl StoredWordV1 {
    fn to_domain(&self, id: WordId) -> Result<WordRecord, SnapshotError> {
        Ok(WordRecord {
            id,
            term: self.term.clone(),
            lemma: self.lemma.clone(),
            phonetic: self.phonetic.clone(),
            definition: self.definition.clone(),
            created_at: decode_timestamp(self.created_at)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredContextV1 {
    id: String,
    word_id: String,
    sentence: String,
    source: String,
    captured_at: i64,
}

impl From<&ContextRecord> for StoredContextV1 {
    fn from(context: &ContextRecord) -> Self {
        Self {
            id: context.id.as_str().to_string(),
            word_id: context.word_id.as_str().to_string(),
            sentence: context.sentence.clone(),
            source: context.source.clone(),
            captured_at: context.captured_at.timestamp_millis(),
        }
    }
}

impl StoredContextV1 {
    fn to_domain(&self) -> Result<ContextRecord, SnapshotError> {
        Ok(ContextRecord {
            id: ContextId::new(&self.id),
            word_id: WordId::new(&self.word_id),
            sentence: self.sentence.clone(),
            source: self.source.clone(),
            captured_at: decode_timestamp(self.captured_at)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredReviewStateV1 {
    word_id: String,
    ease_factor: f64,
    repetitions: u32,
    interval_days: u32,
    next_review_at: i64,
    last_rating: Option<u8>,
}

impl From<&ReviewState> for StoredReviewStateV1 {
    fn from(state: &ReviewState) -> Self {
        Self {
            word_id: state.word_id.as_str().to_string(),
            ease_factor: state.ease_factor,
            repetitions: state.repetitions,
            interval_days: state.interval_days,
            next_review_at: state.next_review_at.timestamp_millis(),
            last_rating: state.last_rating.map(ReviewRating::value),
        }
    }
}

impl StoredReviewStateV1 {
    fn to_domain(&self, word_id: WordId) -> Result<ReviewState, SnapshotError> {
        let state = ReviewState {
            word_id,
            ease_factor: self.ease_factor,
            repetitions: self.repetitions,
            interval_days: self.interval_days,
            next_review_at: decode_timestamp(self.next_review_at)?,
            last_rating: self
                .last_rating
                .map(|rating| {
                    ReviewRating::try_from(rating).map_err(|_| SnapshotError::InvalidRating(rating))
                })
                .transpose()?,
        };
        validate_review_state(&state)?;
        Ok(state)
    }
}

fn validate_capture(capture: &Capture) -> Result<(), SnapshotError> {
    if capture.initial_review_state.word_id != capture.word.id
        || capture
            .contexts
            .iter()
            .any(|context| context.word_id != capture.word.id)
    {
        return Err(SnapshotError::InvalidCapture);
    }
    validate_review_state(&capture.initial_review_state)
}

fn validate_podcast_capture(capture: &PodcastCapture) -> Result<&ContextRecord, SnapshotError> {
    PodcastCapture::try_new(capture.capture.clone(), capture.podcast_context.clone())
        .map_err(|_| SnapshotError::InvalidPodcastCapture)?;
    capture
        .capture
        .contexts
        .first()
        .ok_or(SnapshotError::InvalidPodcastCapture)
}

fn validate_review_state(state: &ReviewState) -> Result<(), SnapshotError> {
    if state.ease_factor.is_finite() && state.ease_factor >= MIN_EASE_FACTOR {
        Ok(())
    } else {
        Err(SnapshotError::InvalidEaseFactor(state.ease_factor))
    }
}

fn validate_snapshot(snapshot: &SnapshotV2) -> Result<(), SnapshotError> {
    let mut lemmas = HashSet::new();
    for (id, word) in &snapshot.words {
        if id != &word.id {
            return Err(SnapshotError::CorruptSnapshot(format!(
                "word map key {id} does not match record {}",
                word.id
            )));
        }
        let lemma = word.lemma.to_lowercase();
        if !lemmas.insert(lemma.clone()) {
            return Err(SnapshotError::DuplicateLemma(lemma));
        }
        decode_timestamp(word.created_at)?;
    }
    for (id, context) in &snapshot.contexts {
        if id != &context.id {
            return Err(SnapshotError::CorruptSnapshot(format!(
                "context map key {id} does not match record {}",
                context.id
            )));
        }
        ensure_word_exists(snapshot, &context.word_id)?;
        decode_timestamp(context.captured_at)?;
    }
    for (id, state) in &snapshot.review_states {
        if id != &state.word_id {
            return Err(SnapshotError::CorruptSnapshot(format!(
                "review map key {id} does not match record {}",
                state.word_id
            )));
        }
        ensure_word_exists(snapshot, &state.word_id)?;
        state.to_domain(WordId::new(id))?;
    }
    for (id, record) in &snapshot.podcast_contexts {
        if id != record.context_id.as_str() {
            return Err(SnapshotError::CorruptSnapshot(format!(
                "podcast context map key {id} does not match record {}",
                record.context_id.as_str()
            )));
        }
        ensure_word_exists(snapshot, record.word_id.as_str())?;
        let context = snapshot
            .contexts
            .get(id)
            .ok_or_else(|| SnapshotError::MissingContext(id.clone()))?;
        if context.word_id != record.word_id.as_str()
            || context.sentence != record.context.sentence
            || context.captured_at != record.context.captured_at.timestamp_millis()
        {
            return Err(SnapshotError::InvalidPodcastCapture);
        }
    }
    Ok(())
}

fn ensure_word_exists(snapshot: &SnapshotV2, word_id: &str) -> Result<(), SnapshotError> {
    if snapshot.words.contains_key(word_id) {
        Ok(())
    } else {
        Err(SnapshotError::MissingWord(word_id.to_string()))
    }
}

fn ensure_lemma_available(
    snapshot: &SnapshotV2,
    word_id: &str,
    lemma: &str,
) -> Result<(), SnapshotError> {
    let normalized = lemma.to_lowercase();
    if snapshot
        .words
        .iter()
        .any(|(id, word)| id != word_id && word.lemma.to_lowercase() == normalized)
    {
        Err(SnapshotError::DuplicateLemma(normalized))
    } else {
        Ok(())
    }
}

fn decode_timestamp(value: i64) -> Result<DateTime<Utc>, SnapshotError> {
    DateTime::from_timestamp_millis(value).ok_or(SnapshotError::InvalidTimestamp(value))
}

fn increment_interval(distribution: &mut IntervalDistribution, interval_days: u32) {
    match interval_days {
        0 => distribution.new = distribution.new.saturating_add(1),
        1..=6 => distribution.days_1_to_6 = distribution.days_1_to_6.saturating_add(1),
        7..=30 => distribution.days_7_to_30 = distribution.days_7_to_30.saturating_add(1),
        31..=90 => distribution.days_31_to_90 = distribution.days_31_to_90.saturating_add(1),
        _ => distribution.days_91_plus = distribution.days_91_plus.saturating_add(1),
    }
}
