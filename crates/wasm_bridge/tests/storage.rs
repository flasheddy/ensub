use std::cell::Cell;
use std::collections::BTreeMap;
use std::io;

use chrono::{DateTime, Duration, Utc};
use core_engine::{
    calculate_padded_audio_slice, initial_review_state, Capture, ContextId, ContextRecord,
    CueRange, PodcastContextDraft, PodcastContextQuality, PodcastEpisodeProvenance,
    PodcastFeedProvenance, PodcastStorageAdapter, ReviewQueueStorageAdapter, ReviewRating,
    ReviewUpdate, StorageAdapter, TranscriptFormat, TranscriptProvenance, TranscriptToken, WordId,
    WordRecord,
};
use ensub_wasm::{
    SnapshotAccess, SnapshotBackend, SnapshotError, SnapshotMigrationStatus, SnapshotStorage,
    V01_BACKUP_STORAGE_KEY,
};
use language_engine::{podcast_capture_from_entry, Definition, LexiconEntry};

#[derive(Default)]
struct MemoryBackend {
    values: BTreeMap<String, String>,
    fail_next_store: bool,
    fail_store_key: Option<String>,
    load_count: Cell<u64>,
}

impl MemoryBackend {
    fn value(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    fn load_count(&self) -> u64 {
        self.load_count.get()
    }
}

impl SnapshotBackend for MemoryBackend {
    type Error = io::Error;

    fn load(&self, key: &str) -> Result<Option<String>, Self::Error> {
        self.load_count.set(self.load_count.get().saturating_add(1));
        Ok(self.values.get(key).cloned())
    }

    fn store(&mut self, key: &str, value: &str) -> Result<(), Self::Error> {
        if self.fail_next_store || self.fail_store_key.as_deref() == Some(key) {
            self.fail_next_store = false;
            return Err(io::Error::new(io::ErrorKind::StorageFull, "quota"));
        }
        self.values.insert(key.to_string(), value.to_string());
        Ok(())
    }

    fn remove(&mut self, key: &str) -> Result<(), Self::Error> {
        self.values.remove(key);
        Ok(())
    }
}

fn timestamp(hour: u32) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(&format!("2026-08-15T{hour:02}:00:00Z"))
        .expect("fixture timestamp must parse")
        .with_timezone(&Utc)
}

fn capture(id: &str, lemma: &str, next_review_at: DateTime<Utc>) -> Capture {
    let word_id = WordId::new(id);
    Capture {
        word: WordRecord {
            id: word_id.clone(),
            term: lemma.to_string(),
            lemma: lemma.to_string(),
            phonetic: format!("/{lemma}/"),
            definition: format!("noun: {lemma} definition"),
            created_at: timestamp(8),
        },
        contexts: vec![ContextRecord {
            id: ContextId::new(format!("context-{id}")),
            word_id: word_id.clone(),
            sentence: format!("A sentence containing {lemma}."),
            source: "sandbox".to_string(),
            captured_at: timestamp(9),
        }],
        initial_review_state: initial_review_state(word_id, next_review_at),
    }
}

fn storage(backend: MemoryBackend) -> SnapshotStorage<MemoryBackend> {
    SnapshotStorage::new(backend, "ensub.test", SnapshotAccess::ReadWrite)
}

fn podcast_capture(cue_id: &str) -> core_engine::PodcastCapture {
    let range = CueRange::try_new(cue_id.to_string(), cue_id.to_string(), 1_000, 2_000)
        .expect("fixture range must be valid");
    let draft = PodcastContextDraft {
        sentence: "We went home.".to_string(),
        quality: PodcastContextQuality::CompleteSentence,
        feed: PodcastFeedProvenance {
            source_url: "https://example.test/feed.xml".to_string(),
            title: "Test Feed".to_string(),
        },
        episode: PodcastEpisodeProvenance {
            internal_id: "episode-1".to_string(),
            publisher_guid: None,
            title: "Episode".to_string(),
            published_at: None,
            enclosure_url: "https://example.test/audio.mp3".to_string(),
        },
        transcript: TranscriptProvenance {
            url: "https://example.test/transcript.vtt".to_string(),
            format: TranscriptFormat::WebVtt,
            language: Some("en".to_string()),
        },
        selected_cue_id: cue_id.to_string(),
        selected_token: TranscriptToken::try_new("went".to_string(), 3, 7)
            .expect("fixture token must be valid"),
        cue_range: range.clone(),
        playback_position_ms: 1_500,
        audio_slice: calculate_padded_audio_slice(
            "https://example.test/audio.mp3".to_string(),
            &range,
            None,
        )
        .expect("fixture slice must be valid"),
    };
    podcast_capture_from_entry(
        draft,
        LexiconEntry {
            lemma: "go".to_string(),
            phonetic: "go".to_string(),
            definitions: vec![Definition {
                part_of_speech: "verb".to_string(),
                text: "move".to_string(),
            }],
        },
        timestamp(10),
    )
    .expect("fixture capture must build")
}

#[test]
fn capture_round_trip_uses_versioned_snapshot_and_survives_reopen() {
    let mut first_session = storage(MemoryBackend::default());
    let saved = first_session
        .save_capture(&capture("word-a", "immersion", timestamp(10)))
        .expect("capture must save");
    let backend = first_session.into_backend();
    let raw: serde_json::Value = serde_json::from_str(
        backend
            .value("ensub.test")
            .expect("snapshot must be written"),
    )
    .expect("snapshot must be JSON");
    let reopened = storage(backend);
    let due = reopened
        .due_reviews(timestamp(10))
        .expect("reopened storage must query");

    assert!(saved.word_created);
    assert_eq!(saved.contexts_created, 1);
    assert_eq!(raw["format"], "ensub-browser-storage");
    assert_eq!(raw["schemaVersion"], 2);
    assert_eq!(raw["revision"], 1);
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].word.lemma, "immersion");
}

#[test]
fn repeated_capture_preserves_term_creation_and_review_but_refreshes_lexicon_fields() {
    let mut storage = storage(MemoryBackend::default());
    let first = capture("word-a", "immersion", timestamp(10));
    storage
        .save_capture(&first)
        .expect("first capture must save");
    let mut reviewed = first.initial_review_state.clone();
    reviewed.interval_days = 6;
    reviewed.repetitions = 2;
    reviewed.next_review_at = timestamp(16);
    storage
        .save_review_state(&reviewed)
        .expect("review state must save");
    let mut repeated = first.clone();
    repeated.word.term = "Immersions".to_string();
    repeated.word.created_at = timestamp(12);
    repeated.word.definition = "noun: refreshed definition".to_string();
    repeated.contexts[0].id = ContextId::new("context-new");
    repeated.contexts[0].captured_at = timestamp(12);

    let result = storage
        .save_capture(&repeated)
        .expect("repeated capture must save");
    let cards = storage
        .due_reviews(timestamp(20))
        .expect("cards must query");

    assert!(!result.word_created);
    assert_eq!(result.contexts_created, 1);
    assert_eq!(cards[0].word.term, "immersion");
    assert_eq!(cards[0].word.created_at, timestamp(8));
    assert_eq!(cards[0].word.definition, "noun: refreshed definition");
    assert_eq!(cards[0].state, reviewed);
    assert_eq!(cards[0].contexts.len(), 2);
    assert_eq!(cards[0].contexts[0].captured_at, timestamp(12));
}

#[test]
fn due_order_statistics_and_next_scheduled_time_match_core_contract() {
    let mut storage = storage(MemoryBackend::default());
    let mut first = capture("word-a", "alpha", timestamp(10));
    first.initial_review_state.interval_days = 0;
    let mut second = capture("word-b", "beta", timestamp(9));
    second.initial_review_state.interval_days = 12;
    let mut future = capture("word-c", "gamma", timestamp(13));
    future.initial_review_state.interval_days = 100;
    storage
        .save_captures(&[first, second, future])
        .expect("batch must save");

    let due = storage
        .due_reviews(timestamp(10))
        .expect("due reviews must query");
    let stats = storage
        .review_statistics(timestamp(10))
        .expect("statistics must query");

    assert_eq!(due[0].word.lemma, "beta");
    assert_eq!(due[1].word.lemma, "alpha");
    assert_eq!(stats.total_cards, 3);
    assert_eq!(stats.due_cards, 2);
    assert_eq!(stats.intervals.new, 1);
    assert_eq!(stats.intervals.days_7_to_30, 1);
    assert_eq!(stats.intervals.days_91_plus, 1);
    assert_eq!(
        storage
            .next_review_at_after(timestamp(10))
            .expect("next review must query"),
        Some(timestamp(13))
    );
}

#[test]
fn compare_and_swap_updates_exact_state_and_rejects_stale_state() {
    let mut storage = storage(MemoryBackend::default());
    let capture = capture("word-a", "alpha", timestamp(10));
    storage.save_capture(&capture).expect("capture must save");
    let expected = capture.initial_review_state;
    let mut replacement = expected.clone();
    replacement.interval_days = 1;
    replacement.repetitions = 1;
    replacement.next_review_at = timestamp(10) + Duration::days(1);
    replacement.last_rating = Some(ReviewRating::try_from(4).expect("rating must validate"));

    assert_eq!(
        storage
            .compare_and_swap_review_state(&expected, &replacement)
            .expect("current state must update"),
        ReviewUpdate::Updated
    );
    assert_eq!(
        storage
            .compare_and_swap_review_state(&expected, &replacement)
            .expect("stale state must return a conflict"),
        ReviewUpdate::Conflict
    );
    assert_eq!(
        storage
            .review_state(&WordId::new("word-a"))
            .expect("review state must query"),
        Some(replacement)
    );
}

#[test]
fn invalid_batch_and_failed_backend_write_leave_snapshot_unchanged() {
    let mut storage = storage(MemoryBackend::default());
    storage
        .save_capture(&capture("word-a", "alpha", timestamp(10)))
        .expect("first capture must save");
    let before = storage
        .backend()
        .value("ensub.test")
        .expect("snapshot must exist")
        .to_string();
    let mut invalid = capture("word-b", "beta", timestamp(10));
    invalid.contexts[0].word_id = WordId::new("wrong-word");

    assert!(matches!(
        storage.save_captures(&[capture("word-c", "gamma", timestamp(10)), invalid]),
        Err(SnapshotError::InvalidCapture)
    ));
    assert_eq!(storage.backend().value("ensub.test"), Some(before.as_str()));

    storage.backend_mut().fail_next_store = true;
    assert!(matches!(
        storage.save_capture(&capture("word-d", "delta", timestamp(10))),
        Err(SnapshotError::Backend { .. })
    ));
    assert_eq!(storage.backend().value("ensub.test"), Some(before.as_str()));
}

#[test]
fn corrupt_and_newer_snapshots_enter_read_only_recovery_and_are_preserved() {
    let mut corrupt = MemoryBackend::default();
    corrupt
        .values
        .insert("ensub.test".to_string(), "{not-json".to_string());
    let mut corrupt_storage = storage(corrupt);
    assert!(matches!(
        corrupt_storage.initialize(),
        Err(SnapshotError::CorruptSnapshot(_))
    ));
    assert!(corrupt_storage.is_read_only());
    assert!(matches!(
        corrupt_storage.reset(),
        Err(SnapshotError::ReadOnly)
    ));
    assert_eq!(
        corrupt_storage.backend().value("ensub.test"),
        Some("{not-json")
    );

    let raw_newer = r#"{"format":"ensub-browser-storage","schemaVersion":3,"revision":0,"words":{},"contexts":{},"reviewStates":{},"podcastContexts":{}}"#;
    let mut newer = MemoryBackend::default();
    newer
        .values
        .insert("ensub.test".to_string(), raw_newer.to_string());
    let mut newer_storage = storage(newer);
    assert!(matches!(
        newer_storage.initialize(),
        Err(SnapshotError::UnsupportedSchema { actual: 3, .. })
    ));
    assert!(newer_storage.is_read_only());
    assert!(matches!(
        newer_storage.reset(),
        Err(SnapshotError::ReadOnly)
    ));
    assert_eq!(newer_storage.backend().value("ensub.test"), Some(raw_newer));
}

#[test]
fn initialization_backs_up_and_migrates_a_v1_snapshot() {
    let raw_v1 = include_str!("fixtures/browser-snapshot-v1.json").to_string();
    let mut backend = MemoryBackend::default();
    backend
        .values
        .insert("ensub.test".to_string(), raw_v1.clone());
    let mut storage = storage(backend);

    assert_eq!(
        storage.initialize().expect("v1 migration must succeed"),
        SnapshotMigrationStatus::Migrated
    );
    assert_eq!(
        storage.backend().value(V01_BACKUP_STORAGE_KEY),
        Some(raw_v1.as_str())
    );
    let upgraded: serde_json::Value = serde_json::from_str(
        storage
            .backend()
            .value("ensub.test")
            .expect("v2 must exist"),
    )
    .expect("v2 must be JSON");
    assert_eq!(upgraded["schemaVersion"], 2);
    assert_eq!(upgraded["revision"], 8);
    assert_eq!(upgraded["podcastContexts"], serde_json::json!({}));
    let old = storage
        .review_state(&WordId::new("word-v01"))
        .expect("migrated review must query")
        .expect("migrated review must remain");
    assert_eq!(old.repetitions, 0);
    assert_eq!(old.interval_days, 0);
    let cards = storage
        .due_reviews(timestamp(10))
        .expect("migrated card must query");
    assert!(cards.iter().any(|card| card.word.lemma == "immersion"));
}

#[test]
fn initialization_does_not_rewrite_empty_or_current_storage() {
    let mut empty = storage(MemoryBackend::default());
    assert_eq!(
        empty.initialize().expect("empty storage must initialize"),
        SnapshotMigrationStatus::Empty
    );
    assert!(empty.backend().values.is_empty());

    let raw_v2 = r#"{"format":"ensub-browser-storage","schemaVersion":2,"revision":4,"words":{},"contexts":{},"reviewStates":{},"podcastContexts":{}}"#;
    let mut backend = MemoryBackend::default();
    backend
        .values
        .insert("ensub.test".to_string(), raw_v2.to_string());
    let mut current = storage(backend);
    assert_eq!(
        current.initialize().expect("v2 storage must initialize"),
        SnapshotMigrationStatus::Current
    );
    assert_eq!(current.backend().value("ensub.test"), Some(raw_v2));
    assert_eq!(current.backend().value(V01_BACKUP_STORAGE_KEY), None);
}

#[test]
fn identical_backup_allows_migration_retry_without_overwriting_backup() {
    let raw_v1 = include_str!("fixtures/browser-snapshot-v1.json").to_string();
    let mut backend = MemoryBackend::default();
    backend
        .values
        .insert("ensub.test".to_string(), raw_v1.clone());
    backend
        .values
        .insert(V01_BACKUP_STORAGE_KEY.to_string(), raw_v1.clone());
    backend.fail_store_key = Some(V01_BACKUP_STORAGE_KEY.to_string());
    let mut storage = storage(backend);

    assert_eq!(
        storage
            .initialize()
            .expect("identical backup must permit retry"),
        SnapshotMigrationStatus::Migrated
    );
    assert_eq!(
        storage.backend().value(V01_BACKUP_STORAGE_KEY),
        Some(raw_v1.as_str())
    );
}

#[test]
fn conflicting_backup_aborts_and_latches_read_only_recovery() {
    let raw_v1 = include_str!("fixtures/browser-snapshot-v1.json").to_string();
    let mut backend = MemoryBackend::default();
    backend
        .values
        .insert("ensub.test".to_string(), raw_v1.clone());
    backend.values.insert(
        V01_BACKUP_STORAGE_KEY.to_string(),
        "different snapshot".to_string(),
    );
    let mut storage = storage(backend);

    assert!(matches!(
        storage.initialize(),
        Err(SnapshotError::BackupConflict)
    ));
    assert!(storage.is_read_only());
    assert_eq!(
        storage.raw_snapshot().expect("raw export must load"),
        Some(raw_v1)
    );
    assert!(matches!(
        storage.save_capture(&capture("word-a", "alpha", timestamp(10))),
        Err(SnapshotError::ReadOnly)
    ));
}

#[test]
fn corrupt_v1_aborts_read_only_without_creating_a_backup() {
    let raw = r#"{"format":"ensub-browser-storage","schemaVersion":1}"#;
    let mut backend = MemoryBackend::default();
    backend
        .values
        .insert("ensub.test".to_string(), raw.to_string());
    let mut storage = storage(backend);

    assert!(matches!(
        storage.initialize(),
        Err(SnapshotError::CorruptSnapshot(_))
    ));
    assert!(storage.is_read_only());
    assert_eq!(storage.backend().value(V01_BACKUP_STORAGE_KEY), None);
    assert_eq!(
        storage.raw_snapshot().expect("raw export must load"),
        Some(raw.to_string())
    );
}

#[test]
fn migration_write_failures_preserve_recovery_data_and_latch_read_only() {
    let raw_v1 = include_str!("fixtures/browser-snapshot-v1.json").to_string();
    let mut backup_failure = MemoryBackend::default();
    backup_failure
        .values
        .insert("ensub.test".to_string(), raw_v1.clone());
    backup_failure.fail_store_key = Some(V01_BACKUP_STORAGE_KEY.to_string());
    let mut backup_storage = storage(backup_failure);
    assert!(matches!(
        backup_storage.initialize(),
        Err(SnapshotError::Backend { .. })
    ));
    assert!(backup_storage.is_read_only());
    assert_eq!(
        backup_storage.backend().value("ensub.test"),
        Some(raw_v1.as_str())
    );
    assert_eq!(backup_storage.backend().value(V01_BACKUP_STORAGE_KEY), None);

    let mut active_failure = MemoryBackend::default();
    active_failure
        .values
        .insert("ensub.test".to_string(), raw_v1.clone());
    active_failure.fail_store_key = Some("ensub.test".to_string());
    let mut storage = storage(active_failure);
    assert!(matches!(
        storage.initialize(),
        Err(SnapshotError::Backend { .. })
    ));
    assert!(storage.is_read_only());
    assert_eq!(storage.backend().value("ensub.test"), Some(raw_v1.as_str()));
    assert_eq!(
        storage.backend().value(V01_BACKUP_STORAGE_KEY),
        Some(raw_v1.as_str())
    );
}

#[test]
fn raw_export_falls_back_to_backup_and_healthy_reset_removes_both_keys() {
    let raw_v1 = include_str!("fixtures/browser-snapshot-v1.json").to_string();
    let mut backup_only = MemoryBackend::default();
    backup_only
        .values
        .insert(V01_BACKUP_STORAGE_KEY.to_string(), raw_v1.clone());
    let backup_storage = storage(backup_only);
    assert_eq!(
        backup_storage
            .raw_snapshot()
            .expect("backup export must load"),
        Some(raw_v1.clone())
    );

    let mut backend = MemoryBackend::default();
    backend
        .values
        .insert("ensub.test".to_string(), raw_v1.clone());
    let mut storage = storage(backend);
    storage.initialize().expect("migration must succeed");
    storage.reset().expect("healthy reset must succeed");

    assert_eq!(storage.backend().value("ensub.test"), None);
    assert_eq!(storage.backend().value(V01_BACKUP_STORAGE_KEY), None);
}

#[test]
fn failed_v1_migration_preserves_exact_original_bytes() {
    let raw_v1 = "{\n  \"format\": \"ensub-browser-storage\", \"schemaVersion\": 1, \"revision\": 0, \"words\": {}, \"contexts\": {}, \"reviewStates\": {}\n}";
    let mut backend = MemoryBackend::default();
    backend
        .values
        .insert("ensub.test".to_string(), raw_v1.to_string());
    backend.fail_next_store = true;
    let mut storage = storage(backend);

    assert!(matches!(
        storage.save_capture(&capture("word-a", "alpha", timestamp(10))),
        Err(SnapshotError::Backend { .. })
    ));
    assert_eq!(storage.backend().value("ensub.test"), Some(raw_v1));
}

#[test]
fn podcast_capture_is_atomic_idempotent_and_retains_multiple_encounters() {
    let mut storage = storage(MemoryBackend::default());
    let first = podcast_capture("cue-1");
    let saved = storage
        .save_podcast_capture(&first)
        .expect("first encounter must save");
    let retry = storage
        .save_podcast_capture(&first)
        .expect("retry must be idempotent");
    let second = storage
        .save_podcast_capture(&podcast_capture("cue-2"))
        .expect("second encounter must save");
    let contexts = storage
        .podcast_contexts(&first.capture.word.id)
        .expect("contexts must query");

    assert!(saved.word_created);
    assert!(saved.podcast_context_created);
    assert!(!retry.word_created);
    assert!(!retry.podcast_context_created);
    assert!(!second.word_created);
    assert!(second.podcast_context_created);
    assert_eq!(contexts.len(), 2);
}

#[test]
fn malformed_podcast_capture_is_rejected_without_writing_or_panicking() {
    let mut storage = storage(MemoryBackend::default());
    let mut missing_context = podcast_capture("cue-1");
    missing_context.capture.contexts.clear();

    assert!(matches!(
        storage.save_podcast_capture(&missing_context),
        Err(SnapshotError::InvalidPodcastCapture)
    ));
    assert_eq!(storage.backend().value("ensub.test"), None);

    let mut mismatched = podcast_capture("cue-1");
    mismatched.capture.contexts[0].sentence = "Different sentence.".to_string();
    assert!(matches!(
        storage.save_podcast_capture(&mismatched),
        Err(SnapshotError::InvalidPodcastCapture)
    ));
    assert_eq!(storage.backend().value("ensub.test"), None);
}

#[test]
fn podcast_capture_accepts_monotonic_publisher_guid_enrichment() {
    let mut storage = storage(MemoryBackend::default());
    let first = podcast_capture("cue-1");
    storage
        .save_podcast_capture(&first)
        .expect("capture without GUID must save");
    let mut enriched = first.clone();
    enriched.podcast_context.context.episode.publisher_guid = Some("publisher-guid".to_string());

    let repeated = storage
        .save_podcast_capture(&enriched)
        .expect("GUID enrichment must remain idempotent");
    let contexts = storage
        .podcast_contexts(&first.capture.word.id)
        .expect("enriched context must load");

    assert!(!repeated.podcast_context_created);
    assert_eq!(
        contexts[0].context.episode.publisher_guid.as_deref(),
        Some("publisher-guid")
    );

    let mut conflicting = enriched;
    conflicting.podcast_context.context.episode.publisher_guid = Some("different-guid".to_string());
    assert!(matches!(
        storage.save_podcast_capture(&conflicting),
        Err(SnapshotError::PodcastContextConflict(_))
    ));
}

#[test]
fn review_queue_enriches_cards_from_one_snapshot_load() {
    let mut storage = storage(MemoryBackend::default());
    storage
        .save_podcast_capture(&podcast_capture("cue-1"))
        .expect("first podcast encounter must save");
    storage
        .save_podcast_capture(&podcast_capture("cue-2"))
        .expect("second podcast encounter must save");
    storage
        .save_capture(&capture("word-text", "text", timestamp(10)))
        .expect("text-only capture must save");
    let loads_before = storage.backend().load_count();

    let queue = storage
        .due_review_queue(timestamp(10), 50)
        .expect("review queue must load");

    assert_eq!(storage.backend().load_count(), loads_before + 1);
    assert_eq!(queue.len(), 2);
    let podcast = queue
        .iter()
        .find(|item| item.card.word.lemma == "go")
        .expect("podcast card must be present");
    assert_eq!(podcast.card.contexts.len(), 2);
    assert_eq!(podcast.podcast_contexts.len(), 2);
    assert!(podcast
        .podcast_contexts
        .windows(2)
        .all(|pair| pair[0].context_id.as_str() < pair[1].context_id.as_str()));
    assert_eq!(
        podcast.podcast_contexts[0].context.audio_slice.start_ms(),
        500
    );
    assert_eq!(
        podcast.podcast_contexts[0].context.audio_slice.end_ms(),
        2_500
    );
    let text_only = queue
        .iter()
        .find(|item| item.card.word.lemma == "text")
        .expect("text-only card must be present");
    assert!(text_only.podcast_contexts.is_empty());
}

#[test]
fn review_queue_honors_due_order_limit_and_item_lookup() {
    let mut storage = storage(MemoryBackend::default());
    storage
        .save_captures(&[
            capture("word-b", "beta", timestamp(9)),
            capture("word-a", "alpha", timestamp(10)),
            capture("word-c", "gamma", timestamp(13)),
        ])
        .expect("captures must save");

    let queue = storage
        .due_review_queue(timestamp(10), 1)
        .expect("bounded queue must load");
    let item = storage
        .review_item(&WordId::new("word-a"))
        .expect("item lookup must load")
        .expect("item must exist");
    let missing = storage
        .review_item(&WordId::new("missing"))
        .expect("missing lookup must succeed");

    assert_eq!(queue.len(), 1);
    assert_eq!(queue[0].card.word.lemma, "beta");
    assert_eq!(item.card.word.lemma, "alpha");
    assert!(item.podcast_contexts.is_empty());
    assert_eq!(missing, None);
    assert!(storage
        .due_review_queue(timestamp(10), 0)
        .expect("zero limit must succeed")
        .is_empty());
}

#[test]
fn read_only_storage_allows_queries_and_rejects_mutations() {
    let mut writable = storage(MemoryBackend::default());
    writable
        .save_capture(&capture("word-a", "alpha", timestamp(10)))
        .expect("capture must save");
    let backend = writable.into_backend();
    let mut read_only = SnapshotStorage::new(backend, "ensub.test", SnapshotAccess::ReadOnly);

    assert_eq!(
        read_only.due_count(timestamp(10)).expect("query must work"),
        1
    );
    assert!(matches!(
        read_only.save_capture(&capture("word-b", "beta", timestamp(10))),
        Err(SnapshotError::ReadOnly)
    ));
    assert!(matches!(read_only.reset(), Err(SnapshotError::ReadOnly)));
}
