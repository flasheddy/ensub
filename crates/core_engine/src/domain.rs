use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

use crate::CoreError;

/// Storage-neutral identifier for a word.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WordId(String);

impl WordId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl From<String> for WordId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for WordId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<WordId> for String {
    fn from(value: WordId) -> Self {
        value.into_inner()
    }
}

/// Storage-neutral identifier for a captured context.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContextId(String);

impl ContextId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl From<String> for ContextId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for ContextId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<ContextId> for String {
    fn from(value: ContextId) -> Self {
        value.into_inner()
    }
}

/// Validated SM-2 recall quality in the inclusive range 0 through 5.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct ReviewRating(u8);

impl ReviewRating {
    pub const fn value(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for ReviewRating {
    type Error = CoreError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if value <= 5 {
            Ok(Self(value))
        } else {
            Err(CoreError::InvalidReviewRating(value))
        }
    }
}

impl From<ReviewRating> for u8 {
    fn from(value: ReviewRating) -> Self {
        value.value()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WordRecord {
    pub id: WordId,
    pub term: String,
    pub lemma: String,
    pub phonetic: String,
    pub definition: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextRecord {
    pub id: ContextId,
    pub word_id: WordId,
    pub sentence: String,
    pub source: String,
    pub captured_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReviewState {
    pub word_id: WordId,
    pub ease_factor: f64,
    pub repetitions: u32,
    pub interval_days: u32,
    pub next_review_at: DateTime<Utc>,
    pub last_rating: Option<ReviewRating>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReviewCard {
    pub word: WordRecord,
    pub contexts: Vec<ContextRecord>,
    pub state: ReviewState,
}

/// Atomic capture payload shared by storage implementations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Capture {
    pub word: WordRecord,
    pub contexts: Vec<ContextRecord>,
    pub initial_review_state: ReviewState,
}

/// Result of idempotently persisting one capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureResult {
    pub word_created: bool,
    pub contexts_created: u64,
}

/// Result of replacing a review state only when the expected state is current.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewUpdate {
    Updated,
    Conflict,
}

/// Counts grouped by the card's current interval.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntervalDistribution {
    pub new: u64,
    pub days_1_to_6: u64,
    pub days_7_to_30: u64,
    pub days_31_to_90: u64,
    pub days_91_plus: u64,
}

/// Storage-neutral summary used by command-line and future dashboard surfaces.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewStatistics {
    pub total_cards: u64,
    pub due_cards: u64,
    pub intervals: IntervalDistribution,
}

/// Stable ordering choices for vocabulary-library pages.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LibraryOrder {
    #[default]
    RecentlyCaptured,
    Alphabetical,
    DueFirst,
}

/// Storage-neutral vocabulary-library query.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryQuery {
    pub search: String,
    pub order: LibraryOrder,
    pub offset: u64,
    pub limit: u32,
}

impl Default for LibraryQuery {
    fn default() -> Self {
        Self {
            search: String::new(),
            order: LibraryOrder::RecentlyCaptured,
            offset: 0,
            limit: 50,
        }
    }
}

/// One page of vocabulary cards and the total matching row count.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LibraryPage {
    pub cards: Vec<ReviewCard>,
    pub total: u64,
    pub offset: u64,
    pub limit: u32,
}

/// Filter and pagination for immutable review history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewHistoryQuery {
    pub word_id: Option<WordId>,
    pub offset: u64,
    pub limit: u32,
}

impl Default for ReviewHistoryQuery {
    fn default() -> Self {
        Self {
            word_id: None,
            offset: 0,
            limit: 50,
        }
    }
}

/// One immutable review event with the complete before and after states.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReviewHistoryEntry {
    pub sequence: u64,
    pub word: WordRecord,
    pub reviewed_at: DateTime<Utc>,
    pub rating: ReviewRating,
    pub previous_state: ReviewState,
    pub resulting_state: ReviewState,
}

/// One page of review events ordered newest first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReviewHistoryPage {
    pub entries: Vec<ReviewHistoryEntry>,
    pub total: u64,
    pub offset: u64,
    pub limit: u32,
}

/// Review totals for one UTC calendar day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DailyReviewActivity {
    pub date: NaiveDate,
    pub reviews: u64,
    pub passing_reviews: u64,
    pub ratings: [u64; 6],
}

/// Review totals for a requested UTC time range.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewActivity {
    pub days: Vec<DailyReviewActivity>,
    pub total_reviews: u64,
    pub passing_reviews: u64,
    pub ratings: [u64; 6],
}
