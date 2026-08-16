use chrono::{TimeZone, Utc};
use core_engine::{
    initial_review_state, ReviewActivity, ReviewCard, ReviewHistoryPage, ReviewRating,
    ReviewStatistics, WordId, WordRecord,
};
use ensub_gui::{update, DashboardData, Effect, Message, Model, Page, ReviewPhase};

fn now() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 8, 15, 12, 0, 0)
        .single()
        .expect("test timestamp must be valid")
}

fn card() -> ReviewCard {
    let word_id = WordId::new("word-read");
    ReviewCard {
        word: WordRecord {
            id: word_id.clone(),
            term: "read".to_string(),
            lemma: "read".to_string(),
            phonetic: "riːd".to_string(),
            definition: "look at written words".to_string(),
            created_at: now(),
        },
        contexts: Vec::new(),
        state: initial_review_state(word_id, now()),
    }
}

#[test]
fn navigation_requests_page_data_and_ignores_stale_library_results() {
    let mut model = Model::new(now());
    let navigated_at = now() + chrono::TimeDelta::minutes(20);
    let effects = update(
        &mut model,
        Message::Navigate {
            page: Page::Library,
            as_of: navigated_at,
        },
    );
    let generation = model.library_generation;

    assert_eq!(effects, vec![Effect::LoadLibrary { generation }]);
    assert_eq!(model.page, Page::Library);
    assert_eq!(model.as_of, navigated_at);

    let _ = update(
        &mut model,
        Message::LibraryLoaded {
            generation: generation.saturating_sub(1),
            result: Err("stale".to_string()),
        },
    );
    assert_eq!(model.error, None);
}

#[test]
fn page_navigation_cycles_in_display_order_and_wraps() {
    assert_eq!(Page::Dashboard.next(), Page::Library);
    assert_eq!(Page::Library.next(), Page::Reader);
    assert_eq!(Page::Reader.next(), Page::ParseText);
    assert_eq!(Page::ParseText.next(), Page::Review);
    assert_eq!(Page::Review.next(), Page::Dashboard);

    assert_eq!(Page::Dashboard.previous(), Page::Review);
    assert_eq!(Page::Reader.previous(), Page::Library);
}

#[test]
fn review_requires_reveal_then_emits_timestamped_commit() {
    let mut model = Model::new(now());
    let review_card = card();
    let _ = update(
        &mut model,
        Message::ReviewBatchLoaded(Ok(vec![review_card.clone()])),
    );
    assert_eq!(model.review.phase, ReviewPhase::Prompt);
    assert!(update(
        &mut model,
        Message::Rate(
            ReviewRating::try_from(4).expect("rating must be valid"),
            now()
        )
    )
    .is_empty());

    let _ = update(&mut model, Message::RevealReview);
    let effects = update(
        &mut model,
        Message::Rate(
            ReviewRating::try_from(4).expect("rating must be valid"),
            now(),
        ),
    );

    assert!(matches!(
        effects.as_slice(),
        [Effect::CommitReview { expected, reviewed_at, .. }]
            if expected == &review_card.state && *reviewed_at == now()
    ));
    assert_eq!(model.review.phase, ReviewPhase::Saving);
}

#[test]
fn review_conflict_discards_stale_card_and_reloads() {
    let mut model = Model::new(now());
    let _ = update(&mut model, Message::ReviewBatchLoaded(Ok(vec![card()])));
    let _ = update(&mut model, Message::RevealReview);
    let _ = update(
        &mut model,
        Message::Rate(
            ReviewRating::try_from(4).expect("rating must be valid"),
            now(),
        ),
    );

    let conflicted_at = now() + chrono::TimeDelta::minutes(30);
    let effects = update(
        &mut model,
        Message::ReviewCommitted(Ok(false), conflicted_at),
    );

    assert_eq!(
        effects,
        vec![Effect::LoadReviewBatch {
            as_of: conflicted_at
        }]
    );
    assert_eq!(model.as_of, conflicted_at);
    assert!(model.review.cards.is_empty());
}

#[test]
fn dashboard_keeps_counts_activity_and_recent_history_together() {
    let mut model = Model::new(now());
    let dashboard = DashboardData {
        statistics: ReviewStatistics {
            total_cards: 7,
            due_cards: 2,
            ..ReviewStatistics::default()
        },
        activity: ReviewActivity {
            total_reviews: 5,
            passing_reviews: 4,
            ..ReviewActivity::default()
        },
        history: ReviewHistoryPage {
            entries: Vec::new(),
            total: 5,
            offset: 0,
            limit: 10,
        },
    };

    let _ = update(&mut model, Message::DashboardLoaded(Ok(dashboard.clone())));

    assert_eq!(model.dashboard, Some(dashboard));
}

#[test]
fn skip_advances_without_emitting_a_review_commit() {
    let mut model = Model::new(now());
    let _ = update(
        &mut model,
        Message::ReviewBatchLoaded(Ok(vec![card(), card()])),
    );

    let effects = update(&mut model, Message::SkipReview);

    assert!(effects.is_empty());
    assert_eq!(model.review.active_index, 1);
    assert_eq!(model.review.phase, ReviewPhase::Prompt);
}
