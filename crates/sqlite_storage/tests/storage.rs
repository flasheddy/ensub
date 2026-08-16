use std::path::Path;
use std::sync::{Arc, Barrier};

use chrono::{DateTime, Duration, TimeZone, Utc};
use core_engine::{
    initial_review_state, schedule_review, Capture, ContextId, ContextRecord, LibraryOrder,
    LibraryQuery, LibraryStorageAdapter, ReviewHistoryQuery, ReviewRating, ReviewUpdate,
    StorageAdapter, WordId, WordRecord,
};
use ensub_sqlite::SqliteStorage;
use rusqlite::Connection;
use tempfile::TempDir;

fn timestamp(hour: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 8, 14, hour, 0, 0)
        .single()
        .expect("test timestamp must be valid")
}

fn capture(id: &str, lemma: &str, interval_days: u32, next_review_at: DateTime<Utc>) -> Capture {
    let word_id = WordId::new(id);
    let mut state = initial_review_state(word_id.clone(), next_review_at);
    state.interval_days = interval_days;
    Capture {
        word: WordRecord {
            id: word_id.clone(),
            term: lemma.to_string(),
            lemma: lemma.to_string(),
            phonetic: format!("/{lemma}/"),
            definition: format!("definition of {lemma}"),
            created_at: timestamp(8),
        },
        contexts: vec![ContextRecord {
            id: ContextId::new(format!("context-{id}")),
            word_id: word_id.clone(),
            sentence: format!("A sentence containing {lemma}."),
            source: "cli:test".to_string(),
            captured_at: timestamp(9),
        }],
        initial_review_state: state,
    }
}

fn open(temp: &TempDir) -> (SqliteStorage, &Path) {
    let path = temp.path().join("ensub.sqlite3");
    let storage = SqliteStorage::open(&path).expect("temporary database must open");
    (storage, Box::leak(path.into_boxed_path()))
}

#[test]
fn open_creates_versioned_wal_database() {
    let temp = TempDir::new().expect("temporary directory must create");
    let (_storage, path) = open(&temp);
    let connection = Connection::open(path).expect("database must reopen");

    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("user version must query");
    let journal_mode: String = connection
        .pragma_query_value(None, "journal_mode", |row| row.get(0))
        .expect("journal mode must query");
    let table_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN ('words', 'contexts', 'review_state', 'review_events')",
            [],
            |row| row.get(0),
        )
        .expect("table count must query");

    assert_eq!(version, 2);
    assert_eq!(journal_mode.to_lowercase(), "wal");
    assert_eq!(table_count, 4);
}

#[test]
fn concurrent_first_open_serializes_schema_migration() {
    let temp = TempDir::new().expect("temporary directory must create");
    let path = Arc::new(temp.path().join("concurrent.sqlite3"));
    let barrier = Arc::new(Barrier::new(3));
    let mut threads = Vec::new();
    for _ in 0..2 {
        let thread_path = Arc::clone(&path);
        let thread_barrier = Arc::clone(&barrier);
        threads.push(std::thread::spawn(move || {
            thread_barrier.wait();
            SqliteStorage::open(thread_path.as_path())
        }));
    }
    barrier.wait();

    for thread in threads {
        thread
            .join()
            .expect("opening thread must not panic")
            .expect("concurrent open must succeed");
    }
    let connection = Connection::open(path.as_path()).expect("database must reopen");
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("schema version must query");
    assert_eq!(version, 2);
}

#[test]
fn version_one_migration_preserves_existing_capture() {
    let temp = TempDir::new().expect("temporary directory must create");
    let path = temp.path().join("ensub.sqlite3");
    let connection = Connection::open(&path).expect("legacy database must open");
    connection
        .execute_batch(
            "CREATE TABLE words (
                id TEXT PRIMARY KEY NOT NULL,
                term TEXT NOT NULL,
                lemma TEXT NOT NULL COLLATE NOCASE UNIQUE,
                phonetic TEXT NOT NULL,
                definition TEXT NOT NULL,
                created_at INTEGER NOT NULL
             ) STRICT;
             CREATE TABLE contexts (
                id TEXT PRIMARY KEY NOT NULL,
                word_id TEXT NOT NULL REFERENCES words(id) ON DELETE CASCADE,
                sentence TEXT NOT NULL,
                source TEXT NOT NULL,
                captured_at INTEGER NOT NULL
             ) STRICT;
             CREATE INDEX contexts_word_captured_idx
                ON contexts(word_id, captured_at DESC, id ASC);
             CREATE TABLE review_state (
                word_id TEXT PRIMARY KEY NOT NULL REFERENCES words(id) ON DELETE CASCADE,
                ease_factor REAL NOT NULL CHECK(ease_factor >= 1.3),
                repetitions INTEGER NOT NULL CHECK(repetitions >= 0),
                interval_days INTEGER NOT NULL CHECK(interval_days >= 0),
                next_review_at INTEGER NOT NULL,
                last_rating INTEGER CHECK(last_rating IS NULL OR last_rating BETWEEN 0 AND 5)
             ) STRICT;
             CREATE INDEX review_state_due_idx ON review_state(next_review_at, word_id);
             INSERT INTO words VALUES ('legacy', 'legacy', 'legacy', '/legacy/', 'old data', 1786672800000);
             INSERT INTO review_state VALUES ('legacy', 2.5, 0, 0, 1786672800000, NULL);
             PRAGMA user_version = 1;",
        )
        .expect("legacy schema must create");
    drop(connection);

    let storage = SqliteStorage::open(&path).expect("legacy database must migrate");
    let card = storage
        .review_card(&WordId::new("legacy"))
        .expect("legacy card must query")
        .expect("legacy card must remain");
    let history = storage
        .review_history(&ReviewHistoryQuery::default())
        .expect("empty history must query");

    assert_eq!(card.word.definition, "old data");
    assert!(history.entries.is_empty());
}

#[test]
fn repeated_capture_preserves_initial_review_state_and_context() {
    let temp = TempDir::new().expect("temporary directory must create");
    let (mut storage, _path) = open(&temp);
    let mut first = capture("word-go", "go", 0, timestamp(10));
    let first_result = storage
        .save_capture(&first)
        .expect("first capture must save");

    first.initial_review_state.next_review_at = timestamp(12);
    first.word.definition = "updated definition".to_string();
    let second_result = storage
        .save_capture(&first)
        .expect("second capture must save idempotently");
    let due = storage
        .due_reviews(timestamp(11))
        .expect("due reviews must query");

    assert!(first_result.word_created);
    assert_eq!(first_result.contexts_created, 1);
    assert!(!second_result.word_created);
    assert_eq!(second_result.contexts_created, 0);
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].state.next_review_at, timestamp(10));
    assert_eq!(due[0].word.definition, "updated definition");
    assert_eq!(due[0].contexts.len(), 1);
}

#[test]
fn due_reviews_are_earliest_first_with_newest_context_first() {
    let temp = TempDir::new().expect("temporary directory must create");
    let (mut storage, _path) = open(&temp);
    let later = capture("word-later", "later", 1, timestamp(11));
    let mut earlier = capture("word-earlier", "earlier", 1, timestamp(10));
    earlier.contexts.push(ContextRecord {
        id: ContextId::new("context-earlier-new"),
        word_id: earlier.word.id.clone(),
        sentence: "The newest context.".to_string(),
        source: "cli:test".to_string(),
        captured_at: timestamp(12),
    });
    storage
        .save_captures(&[later, earlier])
        .expect("captures must save");

    let due = storage
        .due_reviews(timestamp(12))
        .expect("due reviews must query");

    assert_eq!(due[0].word.id, WordId::new("word-earlier"));
    assert_eq!(due[1].word.id, WordId::new("word-later"));
    assert_eq!(due[0].contexts[0].sentence, "The newest context.");
}

#[test]
fn statistics_count_due_cards_and_interval_buckets() {
    let temp = TempDir::new().expect("temporary directory must create");
    let (mut storage, _path) = open(&temp);
    let captures = [
        capture("new", "newword", 0, timestamp(9)),
        capture("short", "short", 6, timestamp(10)),
        capture("medium", "medium", 7, timestamp(11)),
        capture("long", "long", 31, timestamp(12)),
        capture("mature", "mature", 91, timestamp(13)),
    ];
    storage
        .save_captures(&captures)
        .expect("captures must save");

    let statistics = storage
        .review_statistics(timestamp(11))
        .expect("statistics must query");

    assert_eq!(statistics.total_cards, 5);
    assert_eq!(statistics.due_cards, 3);
    assert_eq!(statistics.intervals.new, 1);
    assert_eq!(statistics.intervals.days_1_to_6, 1);
    assert_eq!(statistics.intervals.days_7_to_30, 1);
    assert_eq!(statistics.intervals.days_31_to_90, 1);
    assert_eq!(statistics.intervals.days_91_plus, 1);
}

#[test]
fn review_state_lookup_finds_scheduled_cards_and_reports_missing_words() {
    let temp = TempDir::new().expect("temporary directory must create");
    let (mut storage, _path) = open(&temp);
    let scheduled = capture("scheduled", "scheduled", 15, timestamp(13));
    storage.save_capture(&scheduled).expect("capture must save");

    let stored = storage
        .review_state(&WordId::new("scheduled"))
        .expect("state lookup must query")
        .expect("scheduled state must exist");
    let missing = storage
        .review_state(&WordId::new("missing"))
        .expect("missing lookup must query");

    assert_eq!(stored.interval_days, 15);
    assert_eq!(stored.next_review_at, timestamp(13));
    assert_eq!(missing, None);
}

#[test]
fn invalid_capture_rolls_back_the_entire_batch() {
    let temp = TempDir::new().expect("temporary directory must create");
    let (mut storage, _path) = open(&temp);
    let valid = capture("valid", "valid", 0, timestamp(10));
    let mut invalid = capture("invalid", "invalid", 0, timestamp(10));
    invalid.initial_review_state.word_id = WordId::new("different-word");

    let result = storage.save_captures(&[valid, invalid]);

    assert!(result.is_err());
    assert_eq!(
        storage.due_count(timestamp(10)).expect("count must query"),
        0
    );
}

#[test]
fn stale_review_update_reports_conflict_without_overwriting() {
    let temp = TempDir::new().expect("temporary directory must create");
    let (mut storage, _path) = open(&temp);
    let capture = capture("word-review", "review", 0, timestamp(10));
    storage.save_capture(&capture).expect("capture must save");
    let expected = capture.initial_review_state.clone();
    let mut replacement = expected.clone();
    replacement.repetitions = 1;
    replacement.interval_days = 1;
    replacement.next_review_at = timestamp(10) + Duration::days(1);
    replacement.last_rating = Some(ReviewRating::try_from(4).expect("rating must be valid"));

    let first = storage
        .compare_and_swap_review_state(&expected, &replacement)
        .expect("first update must execute");
    let second = storage
        .compare_and_swap_review_state(&expected, &replacement)
        .expect("stale update must execute");

    assert_eq!(first, ReviewUpdate::Updated);
    assert_eq!(second, ReviewUpdate::Conflict);
}

#[test]
fn ease_factor_only_change_makes_review_update_stale() {
    let temp = TempDir::new().expect("temporary directory must create");
    let (mut storage, _path) = open(&temp);
    let capture = capture("word-ease", "ease", 0, timestamp(10));
    storage.save_capture(&capture).expect("capture must save");
    let expected = capture.initial_review_state.clone();
    let mut concurrent = expected.clone();
    concurrent.ease_factor = 2.0;
    storage
        .save_review_state(&concurrent)
        .expect("concurrent state must save");
    let mut stale_replacement = expected.clone();
    stale_replacement.ease_factor = 2.6;

    let update = storage
        .compare_and_swap_review_state(&expected, &stale_replacement)
        .expect("stale update must execute");
    let stored = storage
        .due_reviews(timestamp(10))
        .expect("stored state must query")
        .remove(0)
        .state;

    assert_eq!(update, ReviewUpdate::Conflict);
    assert_eq!(stored.ease_factor, 2.0);
}

#[test]
fn commit_review_atomically_updates_state_and_appends_history() {
    let temp = TempDir::new().expect("temporary directory must create");
    let (mut storage, _path) = open(&temp);
    let captured = capture("word-history", "history", 0, timestamp(10));
    storage.save_capture(&captured).expect("capture must save");
    let expected = captured.initial_review_state;
    let rating = ReviewRating::try_from(4).expect("rating must be valid");
    let replacement =
        schedule_review(&expected, rating, timestamp(11)).expect("review must schedule");

    let update = storage
        .commit_review(&expected, &replacement, timestamp(11))
        .expect("review must commit");
    let history = storage
        .review_history(&ReviewHistoryQuery::default())
        .expect("history must query");

    assert_eq!(update, ReviewUpdate::Updated);
    assert_eq!(history.total, 1);
    assert_eq!(history.entries[0].word.id, WordId::new("word-history"));
    assert_eq!(history.entries[0].reviewed_at, timestamp(11));
    assert_eq!(history.entries[0].rating, rating);
    assert_eq!(history.entries[0].previous_state, expected);
    assert_eq!(history.entries[0].resulting_state, replacement);
}

#[test]
fn stale_commit_creates_no_history_event() {
    let temp = TempDir::new().expect("temporary directory must create");
    let (mut storage, _path) = open(&temp);
    let captured = capture("word-conflict", "conflict", 0, timestamp(10));
    storage.save_capture(&captured).expect("capture must save");
    let expected = captured.initial_review_state;
    let rating = ReviewRating::try_from(4).expect("rating must be valid");
    let replacement =
        schedule_review(&expected, rating, timestamp(11)).expect("review must schedule");
    storage
        .commit_review(&expected, &replacement, timestamp(11))
        .expect("first review must commit");

    let stale = storage
        .commit_review(&expected, &replacement, timestamp(11))
        .expect("stale review must execute");
    let history = storage
        .review_history(&ReviewHistoryQuery::default())
        .expect("history must query");

    assert_eq!(stale, ReviewUpdate::Conflict);
    assert_eq!(history.total, 1);
}

#[test]
fn failed_history_insert_rolls_back_state_update() {
    let temp = TempDir::new().expect("temporary directory must create");
    let (mut storage, path) = open(&temp);
    let captured = capture("word-rollback", "rollback", 0, timestamp(10));
    storage.save_capture(&captured).expect("capture must save");
    let connection = Connection::open(path).expect("database must reopen");
    connection
        .execute_batch(
            "CREATE TRIGGER reject_review_event
             BEFORE INSERT ON review_events
             BEGIN
                SELECT RAISE(ABORT, 'event rejected');
             END;",
        )
        .expect("failure trigger must create");
    drop(connection);
    let expected = captured.initial_review_state;
    let rating = ReviewRating::try_from(4).expect("rating must be valid");
    let replacement =
        schedule_review(&expected, rating, timestamp(11)).expect("review must schedule");

    let result = storage.commit_review(&expected, &replacement, timestamp(11));
    let stored = storage
        .review_state(&expected.word_id)
        .expect("state must query")
        .expect("state must exist");

    assert!(result.is_err());
    assert_eq!(stored, expected);
}

#[test]
fn library_search_treats_percent_and_underscore_as_literal_text() {
    let temp = TempDir::new().expect("temporary directory must create");
    let (mut storage, _path) = open(&temp);
    let mut percent = capture("percent", "100%", 0, timestamp(10));
    percent.word.definition = "a literal percentage".to_string();
    let mut underscore = capture("underscore", "under_score", 0, timestamp(10));
    underscore.contexts[0].source = "source_with_mark".to_string();
    let ordinary = capture("ordinary", "ordinary", 0, timestamp(10));
    storage
        .save_captures(&[percent, underscore, ordinary])
        .expect("captures must save");

    let percent_page = storage
        .library_page(&LibraryQuery {
            search: "%".to_string(),
            ..LibraryQuery::default()
        })
        .expect("percent search must query");
    let underscore_page = storage
        .library_page(&LibraryQuery {
            search: "_".to_string(),
            ..LibraryQuery::default()
        })
        .expect("underscore search must query");

    assert_eq!(percent_page.total, 1);
    assert_eq!(percent_page.cards[0].word.id, WordId::new("percent"));
    assert_eq!(underscore_page.total, 1);
    assert_eq!(underscore_page.cards[0].word.id, WordId::new("underscore"));
}

#[test]
fn library_pages_apply_sort_offset_limit_and_stable_id_ties() {
    let temp = TempDir::new().expect("temporary directory must create");
    let (mut storage, _path) = open(&temp);
    let mut beta_b = capture("beta-b", "beta", 0, timestamp(10));
    beta_b.word.created_at = timestamp(9);
    let mut alpha = capture("alpha", "alpha", 0, timestamp(10));
    alpha.word.created_at = timestamp(9);
    let mut beta_a = capture("beta-a", "beta-two", 0, timestamp(10));
    beta_a.word.term = "beta".to_string();
    beta_a.word.created_at = timestamp(9);
    storage
        .save_captures(&[beta_b, alpha, beta_a])
        .expect("captures must save");

    let page = storage
        .library_page(&LibraryQuery {
            order: LibraryOrder::Alphabetical,
            offset: 1,
            limit: 2,
            ..LibraryQuery::default()
        })
        .expect("library page must query");

    assert_eq!(page.total, 3);
    assert_eq!(page.offset, 1);
    assert_eq!(page.limit, 2);
    assert_eq!(
        page.cards
            .iter()
            .map(|card| card.word.id.clone())
            .collect::<Vec<_>>(),
        vec![WordId::new("beta-a"), WordId::new("beta-b")]
    );
}

#[test]
fn due_review_batch_honors_limit_and_due_order() {
    let temp = TempDir::new().expect("temporary directory must create");
    let (mut storage, _path) = open(&temp);
    storage
        .save_captures(&[
            capture("third", "third", 0, timestamp(11)),
            capture("first", "first", 0, timestamp(9)),
            capture("second", "second", 0, timestamp(10)),
        ])
        .expect("captures must save");

    let cards = storage
        .due_review_batch(timestamp(12), 2)
        .expect("due batch must query");

    assert_eq!(cards.len(), 2);
    assert_eq!(cards[0].word.id, WordId::new("first"));
    assert_eq!(cards[1].word.id, WordId::new("second"));
}

#[test]
fn history_can_filter_by_word_and_activity_uses_utc_half_open_range() {
    let temp = TempDir::new().expect("temporary directory must create");
    let (mut storage, _path) = open(&temp);
    let first = capture("first-history", "first", 0, timestamp(8));
    let second = capture("second-history", "second", 0, timestamp(8));
    storage
        .save_captures(&[first.clone(), second.clone()])
        .expect("captures must save");
    for (captured, quality, reviewed_at) in
        [(&first, 4_u8, timestamp(9)), (&second, 2_u8, timestamp(10))]
    {
        let rating = ReviewRating::try_from(quality).expect("rating must be valid");
        let replacement = schedule_review(&captured.initial_review_state, rating, reviewed_at)
            .expect("review must schedule");
        storage
            .commit_review(&captured.initial_review_state, &replacement, reviewed_at)
            .expect("review must commit");
    }

    let history = storage
        .review_history(&ReviewHistoryQuery {
            word_id: Some(first.word.id.clone()),
            ..ReviewHistoryQuery::default()
        })
        .expect("filtered history must query");
    let activity = storage
        .review_activity(timestamp(9), timestamp(11))
        .expect("activity must query");

    assert_eq!(history.total, 1);
    assert_eq!(history.entries[0].word.id, first.word.id);
    assert_eq!(activity.total_reviews, 2);
    assert_eq!(activity.passing_reviews, 1);
    assert_eq!(activity.ratings, [0, 0, 1, 0, 1, 0]);
    assert_eq!(activity.days.len(), 1);
    assert_eq!(activity.days[0].reviews, 2);
}
