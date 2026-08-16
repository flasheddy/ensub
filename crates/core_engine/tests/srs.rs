use chrono::{DateTime, Duration, Utc};
use core_engine::{
    calculate_next_ease_factor, calculate_next_interval_days, calculate_next_repetitions,
    initial_review_state, schedule_review, CoreError, ReviewRating, ReviewState, WordId,
    DEFAULT_EASE_FACTOR, MIN_EASE_FACTOR,
};
use serde::de::value::{Error as ValueError, U8Deserializer};
use serde::Deserialize;

fn review_time() -> DateTime<Utc> {
    DateTime::from_timestamp(1_700_000_000, 0).expect("test timestamp must be valid")
}

fn rating(value: u8) -> ReviewRating {
    ReviewRating::try_from(value).expect("test rating must be valid")
}

fn assert_float_eq(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-12,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn initial_state_is_due_immediately_with_sm2_defaults() {
    let now = review_time();

    let state = initial_review_state(WordId::new("word-1"), now);

    assert_eq!(state.word_id.as_str(), "word-1");
    assert_float_eq(state.ease_factor, DEFAULT_EASE_FACTOR);
    assert_eq!(state.repetitions, 0);
    assert_eq!(state.interval_days, 0);
    assert_eq!(state.next_review_at, now);
    assert_eq!(state.last_rating, None);
}

#[test]
fn rating_accepts_only_values_from_zero_through_five() {
    assert_eq!(ReviewRating::try_from(0).map(ReviewRating::value), Ok(0));
    assert_eq!(ReviewRating::try_from(5).map(ReviewRating::value), Ok(5));
    assert_eq!(
        ReviewRating::try_from(6),
        Err(CoreError::InvalidReviewRating(6))
    );
}

#[test]
fn rating_deserialization_preserves_validation() {
    let deserializer = U8Deserializer::<ValueError>::new(6);

    let result = ReviewRating::deserialize(deserializer);

    assert!(result.is_err());
}

#[test]
fn passing_reviews_progress_from_one_to_six_to_rounded_interval() {
    let initial_time = review_time();
    let first = schedule_review(
        &initial_review_state(WordId::new("word-1"), initial_time),
        rating(4),
        initial_time,
    )
    .expect("first review must schedule");
    let second = schedule_review(&first, rating(4), first.next_review_at)
        .expect("second review must schedule");
    let third = schedule_review(&second, rating(4), second.next_review_at)
        .expect("third review must schedule");

    assert_eq!((first.repetitions, first.interval_days), (1, 1));
    assert_eq!((second.repetitions, second.interval_days), (2, 6));
    assert_eq!((third.repetitions, third.interval_days), (3, 15));
    assert_float_eq(first.ease_factor, DEFAULT_EASE_FACTOR);
    assert_float_eq(second.ease_factor, DEFAULT_EASE_FACTOR);
    assert_float_eq(third.ease_factor, DEFAULT_EASE_FACTOR);
    assert_eq!(first.next_review_at, initial_time + Duration::days(1));
    assert_eq!(
        second.next_review_at,
        first.next_review_at + Duration::days(6)
    );
    assert_eq!(
        third.next_review_at,
        second.next_review_at + Duration::days(15)
    );
}

#[test]
fn quality_three_uses_current_ease_before_storing_reduced_ease() {
    let state = ReviewState {
        word_id: WordId::new("word-1"),
        ease_factor: 2.5,
        repetitions: 2,
        interval_days: 6,
        next_review_at: review_time(),
        last_rating: Some(rating(4)),
    };

    let next =
        schedule_review(&state, rating(3), review_time()).expect("passing review must schedule");

    assert_eq!(next.repetitions, 3);
    assert_eq!(next.interval_days, 15);
    assert_float_eq(next.ease_factor, 2.36);
}

#[test]
fn rating_below_three_resets_progress_and_schedules_one_day() {
    let now = review_time();
    let state = ReviewState {
        word_id: WordId::new("word-1"),
        ease_factor: 2.5,
        repetitions: 8,
        interval_days: 90,
        next_review_at: now,
        last_rating: Some(rating(5)),
    };

    let next = schedule_review(&state, rating(2), now).expect("failed review must schedule");

    assert_eq!(next.repetitions, 0);
    assert_eq!(next.interval_days, 1);
    assert_float_eq(next.ease_factor, 2.18);
    assert_eq!(next.next_review_at, now + Duration::days(1));
    assert_eq!(next.last_rating, Some(rating(2)));
}

#[test]
fn ease_factor_never_falls_below_minimum() {
    let after_first_failure = calculate_next_ease_factor(2.5, rating(0));
    let after_second_failure = calculate_next_ease_factor(after_first_failure, rating(0));
    let after_more_failures = calculate_next_ease_factor(after_second_failure, rating(0));

    assert_float_eq(after_first_failure, 1.7);
    assert_float_eq(after_second_failure, MIN_EASE_FACTOR);
    assert_float_eq(after_more_failures, MIN_EASE_FACTOR);
}

#[test]
fn quality_five_increases_ease_factor() {
    assert_float_eq(calculate_next_ease_factor(2.5, rating(5)), 2.6);
}

#[test]
fn repetition_helper_uses_pass_fail_boundary() {
    assert_eq!(calculate_next_repetitions(7, rating(2)), 0);
    assert_eq!(calculate_next_repetitions(7, rating(3)), 8);
}

#[test]
fn interval_helper_uses_pass_fail_boundary() {
    let state = ReviewState {
        word_id: WordId::new("word-1"),
        ease_factor: 2.5,
        repetitions: 2,
        interval_days: 6,
        next_review_at: review_time(),
        last_rating: None,
    };

    assert_eq!(calculate_next_interval_days(&state, rating(2)), 1);
    assert_eq!(calculate_next_interval_days(&state, rating(3)), 15);
}

#[test]
fn scheduling_is_deterministic_for_equal_inputs() {
    let now = review_time();
    let state = initial_review_state(WordId::new("word-1"), now);

    let first = schedule_review(&state, rating(5), now);
    let second = schedule_review(&state, rating(5), now);

    assert_eq!(first, second);
}

#[test]
fn scheduling_reports_date_overflow() {
    let state = initial_review_state(WordId::new("word-1"), DateTime::<Utc>::MAX_UTC);

    let result = schedule_review(&state, rating(4), DateTime::<Utc>::MAX_UTC);

    assert_eq!(result, Err(CoreError::ReviewDateOverflow));
}
