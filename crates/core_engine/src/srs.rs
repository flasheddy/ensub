use chrono::{DateTime, Duration, Utc};

use crate::{CoreError, ReviewRating, ReviewState, WordId};

pub const DEFAULT_EASE_FACTOR: f64 = 2.5;
pub const MIN_EASE_FACTOR: f64 = 1.3;

/// Creates the initial, immediately-due review state for a word.
pub fn initial_review_state(word_id: WordId, reviewed_at: DateTime<Utc>) -> ReviewState {
    ReviewState {
        word_id,
        ease_factor: DEFAULT_EASE_FACTOR,
        repetitions: 0,
        interval_days: 0,
        next_review_at: reviewed_at,
        last_rating: None,
    }
}

/// Calculates the rating-adjusted ease factor, floored at 1.3.
pub fn calculate_next_ease_factor(current_ease_factor: f64, rating: ReviewRating) -> f64 {
    let difficulty = f64::from(5 - rating.value());
    let adjustment = 0.1 - difficulty * (0.08 + difficulty * 0.02);

    (current_ease_factor + adjustment).max(MIN_EASE_FACTOR)
}

/// Calculates repetitions after applying the pass/fail boundary.
pub fn calculate_next_repetitions(current: u32, rating: ReviewRating) -> u32 {
    if rating.value() < 3 {
        0
    } else {
        current.saturating_add(1)
    }
}

/// Calculates the next interval using the state's pre-review ease factor.
pub fn calculate_next_interval_days(state: &ReviewState, rating: ReviewRating) -> u32 {
    if rating.value() < 3 {
        return 1;
    }

    match state.repetitions {
        0 => 1,
        1 => 6,
        _ => bounded_rounded_interval(state.interval_days, state.ease_factor),
    }
}

/// Applies a review without reading external time or performing I/O.
pub fn schedule_review(
    state: &ReviewState,
    rating: ReviewRating,
    reviewed_at: DateTime<Utc>,
) -> Result<ReviewState, CoreError> {
    let interval_days = calculate_next_interval_days(state, rating);
    let next_review_at = reviewed_at
        .checked_add_signed(Duration::days(i64::from(interval_days)))
        .ok_or(CoreError::ReviewDateOverflow)?;

    Ok(ReviewState {
        word_id: state.word_id.clone(),
        ease_factor: calculate_next_ease_factor(state.ease_factor, rating),
        repetitions: calculate_next_repetitions(state.repetitions, rating),
        interval_days,
        next_review_at,
        last_rating: Some(rating),
    })
}

fn bounded_rounded_interval(interval_days: u32, ease_factor: f64) -> u32 {
    let rounded = (f64::from(interval_days) * ease_factor).round();

    if rounded.is_nan() || rounded.is_sign_negative() {
        1
    } else if rounded >= f64::from(u32::MAX) {
        u32::MAX
    } else {
        (rounded as u32).max(1)
    }
}
