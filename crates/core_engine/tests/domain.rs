use chrono::{DateTime, Utc};
use core_engine::{
    initial_review_state, ContextId, ContextRecord, DailyReviewActivity, LibraryOrder,
    LibraryQuery, ReviewActivity, ReviewCard, ReviewHistoryEntry, ReviewHistoryPage,
    ReviewHistoryQuery, ReviewRating, WordId, WordRecord,
};
use serde_json::{json, Value};

fn capture_time() -> DateTime<Utc> {
    DateTime::from_timestamp(1_700_000_000, 123_000_000).expect("test timestamp must be valid")
}

fn review_card() -> ReviewCard {
    let captured_at = capture_time();
    let word_id = WordId::new("word-1");

    ReviewCard {
        word: WordRecord {
            id: word_id.clone(),
            term: "immersed".to_string(),
            lemma: "immerse".to_string(),
            phonetic: "ɪˈmɜːs".to_string(),
            definition: "to involve deeply".to_string(),
            created_at: captured_at,
        },
        contexts: vec![ContextRecord {
            id: ContextId::new("context-1"),
            word_id: word_id.clone(),
            sentence: "She was immersed in the book.".to_string(),
            source: "cli".to_string(),
            captured_at,
        }],
        state: initial_review_state(word_id, captured_at),
    }
}

#[test]
fn review_card_round_trip_preserves_portable_domain_values() {
    let card = review_card();

    let serialized = serde_json::to_value(&card).expect("review card must serialize");
    let restored: ReviewCard =
        serde_json::from_value(serialized.clone()).expect("review card must deserialize");

    assert_eq!(serialized["word"]["id"], json!("word-1"));
    assert_eq!(serialized["contexts"][0]["id"], json!("context-1"));
    assert_eq!(serialized["state"]["last_rating"], Value::Null);
    assert_eq!(restored, card);
}

#[test]
fn nested_review_rating_accepts_valid_value_and_rejects_invalid_value() {
    let mut serialized = serde_json::to_value(review_card()).expect("review card must serialize");
    serialized["state"]["last_rating"] = json!(4);

    let restored: ReviewCard =
        serde_json::from_value(serialized.clone()).expect("valid rating must deserialize");
    assert_eq!(restored.state.last_rating.map(ReviewRating::value), Some(4));

    serialized["state"]["last_rating"] = json!(6);
    assert!(serde_json::from_value::<ReviewCard>(serialized).is_err());
}

#[test]
fn library_query_defaults_to_recent_first_page() {
    let query = LibraryQuery::default();

    assert_eq!(query.search, String::new());
    assert_eq!(query.order, LibraryOrder::RecentlyCaptured);
    assert_eq!(query.offset, 0);
    assert_eq!(query.limit, 50);
}

#[test]
fn history_contracts_round_trip_as_owned_portable_values() {
    let card = review_card();
    let rating = ReviewRating::try_from(4).expect("test rating must be valid");
    let resulting_state = core_engine::schedule_review(&card.state, rating, capture_time())
        .expect("test review must schedule");
    let entry = ReviewHistoryEntry {
        sequence: 42,
        word: card.word,
        reviewed_at: capture_time(),
        rating,
        previous_state: card.state,
        resulting_state,
    };
    let page = ReviewHistoryPage {
        entries: vec![entry],
        total: 1,
        offset: 0,
        limit: 50,
    };
    let activity = ReviewActivity {
        days: vec![DailyReviewActivity {
            date: capture_time().date_naive(),
            reviews: 1,
            passing_reviews: 1,
            ratings: [0, 0, 0, 0, 1, 0],
        }],
        total_reviews: 1,
        passing_reviews: 1,
        ratings: [0, 0, 0, 0, 1, 0],
    };

    let encoded = serde_json::to_string(&(page.clone(), activity.clone()))
        .expect("history contracts must serialize");
    let decoded: (ReviewHistoryPage, ReviewActivity) =
        serde_json::from_str(&encoded).expect("history contracts must deserialize");

    assert_eq!(decoded, (page, activity));
}

#[test]
fn review_history_query_defaults_to_global_recent_first_page() {
    let query = ReviewHistoryQuery::default();

    assert_eq!(query.word_id, None);
    assert_eq!(query.offset, 0);
    assert_eq!(query.limit, 50);
}
