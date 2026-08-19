use chrono::{TimeZone, Utc};
use core_engine::{
    initial_review_state, schedule_review, Capture, ContextId, ContextRecord,
    LibraryStorageAdapter, ReviewHistoryQuery, ReviewRating, ReviewUpdate, StorageAdapter, WordId,
    WordRecord,
};
use ensub_applet::{update, Effect, Message, Model, ReviewPhase};
use ensub_sqlite::SqliteStorage;
use tempfile::TempDir;

fn now() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 8, 17, 10, 0, 0)
        .single()
        .expect("test timestamp must be valid")
}

fn capture() -> Capture {
    let word_id = WordId::new("word-concurrent");
    Capture {
        word: WordRecord {
            id: word_id.clone(),
            term: "concurrent".to_string(),
            lemma: "concurrent".to_string(),
            phonetic: "kənˈkʌrənt".to_string(),
            definition: "happening at the same time".to_string(),
            created_at: now(),
        },
        contexts: vec![ContextRecord {
            id: ContextId::new("context-concurrent"),
            word_id: word_id.clone(),
            sentence: "Two reviews may be concurrent.".to_string(),
            source: "applet:test".to_string(),
            captured_at: now(),
        }],
        initial_review_state: initial_review_state(word_id, now()),
    }
}

#[test]
fn concurrent_review_conflict_cannot_desynchronize_applet_state() {
    let temp = TempDir::new().expect("temporary directory must create");
    let path = temp.path().join("ensub.sqlite3");
    let mut first = SqliteStorage::open(&path).expect("first handle must open");
    first
        .save_capture(&capture())
        .expect("fixture capture must save");
    let stale_card = first
        .due_review_batch(now(), 1)
        .expect("due card must load")
        .pop()
        .expect("fixture must be due");
    let expected = stale_card.state.clone();
    let first_replacement = schedule_review(
        &expected,
        ReviewRating::try_from(4).expect("rating must be valid"),
        now(),
    )
    .expect("first review must schedule");
    let second_replacement = schedule_review(
        &expected,
        ReviewRating::try_from(2).expect("rating must be valid"),
        now(),
    )
    .expect("second review must schedule");
    let mut second = SqliteStorage::open(&path).expect("second handle must open");

    let first_result = first
        .commit_review(&expected, &first_replacement, now())
        .expect("first commit must execute");
    let second_result = second
        .commit_review(&expected, &second_replacement, now())
        .expect("second commit must execute");

    assert_eq!(first_result, ReviewUpdate::Updated);
    assert_eq!(second_result, ReviewUpdate::Conflict);
    assert_eq!(
        second
            .review_state(&expected.word_id)
            .expect("authoritative state must query"),
        Some(first_replacement)
    );
    assert_eq!(
        second
            .review_history(&ReviewHistoryQuery::default())
            .expect("review history must query")
            .entries
            .len(),
        1
    );

    let mut model = Model::new(now());
    let _ = update(&mut model, Message::DueCardLoaded(Ok(Some(stale_card))));
    let _ = update(&mut model, Message::Reveal);
    let _ = update(
        &mut model,
        Message::Rate(
            ReviewRating::try_from(2).expect("rating must be valid"),
            now(),
        ),
    );
    let effects = update(
        &mut model,
        Message::ReviewCommitted(Ok(second_result), now()),
    );

    assert!(model.card.is_none());
    assert_eq!(model.review_phase, ReviewPhase::Empty);
    assert_eq!(
        effects,
        vec![
            Effect::RefreshDueCount { as_of: now() },
            Effect::LoadDueCard { as_of: now() }
        ]
    );
}
