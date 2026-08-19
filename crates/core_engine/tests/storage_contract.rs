use chrono::{DateTime, TimeZone, Utc};
use core_engine::{
    initial_review_state, Capture, CaptureResult, ContextId, ContextRecord, IntervalDistribution,
    ReviewCard, ReviewQueueItem, ReviewQueueStorageAdapter, ReviewState, ReviewStatistics,
    ReviewUpdate, StorageAdapter, WordId, WordRecord,
};
use std::convert::Infallible;

#[derive(Default)]
struct RecordingStorage {
    captures: Vec<Capture>,
    compared_states: Vec<(ReviewState, ReviewState)>,
}

impl StorageAdapter for RecordingStorage {
    type Error = Infallible;

    fn save_word(&mut self, _word: &WordRecord) -> Result<(), Self::Error> {
        Ok(())
    }

    fn save_context(&mut self, _context: &ContextRecord) -> Result<(), Self::Error> {
        Ok(())
    }

    fn save_review_state(&mut self, _state: &ReviewState) -> Result<(), Self::Error> {
        Ok(())
    }

    fn save_capture(&mut self, capture: &Capture) -> Result<CaptureResult, Self::Error> {
        self.captures.push(capture.clone());
        Ok(CaptureResult {
            word_created: true,
            contexts_created: capture.contexts.len() as u64,
        })
    }

    fn save_captures(&mut self, captures: &[Capture]) -> Result<Vec<CaptureResult>, Self::Error> {
        captures
            .iter()
            .map(|capture| self.save_capture(capture))
            .collect()
    }

    fn compare_and_swap_review_state(
        &mut self,
        expected: &ReviewState,
        replacement: &ReviewState,
    ) -> Result<ReviewUpdate, Self::Error> {
        self.compared_states
            .push((expected.clone(), replacement.clone()));
        Ok(ReviewUpdate::Updated)
    }

    fn review_state(&self, _word_id: &WordId) -> Result<Option<ReviewState>, Self::Error> {
        Ok(None)
    }

    fn due_reviews(&self, _as_of: DateTime<Utc>) -> Result<Vec<ReviewCard>, Self::Error> {
        Ok(Vec::new())
    }

    fn due_count(&self, _as_of: DateTime<Utc>) -> Result<u64, Self::Error> {
        Ok(0)
    }

    fn review_statistics(&self, _as_of: DateTime<Utc>) -> Result<ReviewStatistics, Self::Error> {
        Ok(ReviewStatistics::default())
    }
}

impl ReviewQueueStorageAdapter for RecordingStorage {
    fn due_review_queue(
        &self,
        _as_of: DateTime<Utc>,
        _limit: u32,
    ) -> Result<Vec<ReviewQueueItem>, Self::Error> {
        Ok(Vec::new())
    }

    fn review_item(&self, _word_id: &WordId) -> Result<Option<ReviewQueueItem>, Self::Error> {
        Ok(None)
    }
}

fn timestamp() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 8, 14, 10, 0, 0)
        .single()
        .expect("test timestamp must be valid")
}

fn capture() -> Capture {
    let word_id = WordId::new("word-go");
    let word = WordRecord {
        id: word_id.clone(),
        term: "went".to_string(),
        lemma: "go".to_string(),
        phonetic: "goʊ".to_string(),
        definition: "move from one place to another".to_string(),
        created_at: timestamp(),
    };
    let context = ContextRecord {
        id: ContextId::new("context-go"),
        word_id: word_id.clone(),
        sentence: "She went home.".to_string(),
        source: "cli:test".to_string(),
        captured_at: timestamp(),
    };

    Capture {
        word,
        contexts: vec![context],
        initial_review_state: initial_review_state(word_id, timestamp()),
    }
}

#[test]
fn capture_is_an_owned_serializable_storage_aggregate() {
    let capture = capture();
    let encoded = serde_json::to_string(&capture).expect("capture must serialize");
    let decoded: Capture = serde_json::from_str(&encoded).expect("capture must deserialize");

    assert_eq!(decoded, capture);
}

#[test]
fn statistics_defaults_cover_every_interval_bucket() {
    let statistics = ReviewStatistics::default();

    assert_eq!(statistics.total_cards, 0);
    assert_eq!(statistics.due_cards, 0);
    assert_eq!(statistics.intervals, IntervalDistribution::default());
    assert_eq!(statistics.intervals.new, 0);
    assert_eq!(statistics.intervals.days_1_to_6, 0);
    assert_eq!(statistics.intervals.days_7_to_30, 0);
    assert_eq!(statistics.intervals.days_31_to_90, 0);
    assert_eq!(statistics.intervals.days_91_plus, 0);
}

#[test]
fn storage_adapter_exposes_atomic_capture_and_reporting_contracts() {
    let capture = capture();
    let mut storage = RecordingStorage::default();

    let result = storage
        .save_captures(std::slice::from_ref(&capture))
        .expect("recording storage must save");

    assert_eq!(
        result,
        vec![CaptureResult {
            word_created: true,
            contexts_created: 1,
        }]
    );
    assert_eq!(storage.captures, vec![capture]);
}

#[test]
fn storage_adapter_exposes_review_state_lookup() {
    let storage = RecordingStorage::default();

    let state = storage
        .review_state(&WordId::new("word-go"))
        .expect("recording storage lookup must succeed");

    assert_eq!(state, None);
}

#[test]
fn commit_review_defaults_to_compare_and_swap_for_existing_adapters() {
    let mut storage = RecordingStorage::default();
    let expected = capture().initial_review_state;
    let rating = core_engine::ReviewRating::try_from(4).expect("test rating must be valid");
    let replacement = core_engine::schedule_review(&expected, rating, timestamp())
        .expect("test review must schedule");

    let update = storage
        .commit_review(&expected, &replacement, timestamp())
        .expect("recording storage commit must succeed");

    assert_eq!(update, ReviewUpdate::Updated);
    assert_eq!(storage.compared_states, vec![(expected, replacement)]);
}

#[test]
fn review_queue_projection_is_owned_serializable_and_optional() {
    let capture = capture();
    let item = ReviewQueueItem {
        card: ReviewCard {
            word: capture.word,
            contexts: capture.contexts,
            state: capture.initial_review_state,
        },
        podcast_contexts: Vec::new(),
    };
    let encoded = serde_json::to_string(&item).expect("review queue item must serialize");
    let decoded: ReviewQueueItem =
        serde_json::from_str(&encoded).expect("review queue item must deserialize");
    let storage = RecordingStorage::default();

    assert_eq!(decoded, item);
    assert_eq!(
        storage
            .due_review_queue(timestamp(), 0)
            .expect("optional queue query must succeed"),
        Vec::new()
    );
    assert_eq!(
        storage
            .review_item(&WordId::new("word-go"))
            .expect("optional item query must succeed"),
        None
    );
}
