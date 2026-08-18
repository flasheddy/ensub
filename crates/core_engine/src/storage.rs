use std::error::Error;

use chrono::{DateTime, Utc};

use crate::{
    Capture, CaptureResult, ContextRecord, LibraryPage, LibraryQuery, PodcastCapture,
    PodcastCaptureResult, PodcastContextRecord, ReviewActivity, ReviewCard, ReviewHistoryPage,
    ReviewHistoryQuery, ReviewState, ReviewStatistics, ReviewUpdate, WordId, WordRecord,
};

/// Persistence boundary implemented by platform-specific storage crates.
///
/// Save operations are idempotent upserts keyed by their record identifiers.
/// Due reviews include states whose `next_review_at` is at or before `as_of`,
/// ordered from earliest to latest due time.
pub trait StorageAdapter {
    type Error: Error + 'static;

    fn save_word(&mut self, word: &WordRecord) -> Result<(), Self::Error>;

    fn save_context(&mut self, context: &ContextRecord) -> Result<(), Self::Error>;

    fn save_review_state(&mut self, state: &ReviewState) -> Result<(), Self::Error>;

    /// Atomically persists one capture without resetting an existing SRS state.
    fn save_capture(&mut self, capture: &Capture) -> Result<CaptureResult, Self::Error>;

    /// Atomically persists every capture or leaves storage unchanged on error.
    fn save_captures(&mut self, captures: &[Capture]) -> Result<Vec<CaptureResult>, Self::Error>;

    /// Replaces `expected` only when it still matches the stored review state.
    fn compare_and_swap_review_state(
        &mut self,
        expected: &ReviewState,
        replacement: &ReviewState,
    ) -> Result<ReviewUpdate, Self::Error>;

    /// Commits a review state transition.
    ///
    /// Adapters that retain review history should override this method and
    /// atomically persist the state transition and its event. The default
    /// preserves compatibility with adapters that only support CAS updates.
    fn commit_review(
        &mut self,
        expected: &ReviewState,
        replacement: &ReviewState,
        _reviewed_at: DateTime<Utc>,
    ) -> Result<ReviewUpdate, Self::Error> {
        self.compare_and_swap_review_state(expected, replacement)
    }

    /// Returns the current SRS state for a captured word, whether due or scheduled.
    fn review_state(&self, word_id: &crate::WordId) -> Result<Option<ReviewState>, Self::Error>;

    fn due_reviews(&self, as_of: DateTime<Utc>) -> Result<Vec<ReviewCard>, Self::Error>;

    fn due_count(&self, as_of: DateTime<Utc>) -> Result<u64, Self::Error>;

    fn review_statistics(&self, as_of: DateTime<Utc>) -> Result<ReviewStatistics, Self::Error>;
}

/// Optional storage capability for atomic podcast encounters and their media provenance.
pub trait PodcastStorageAdapter: StorageAdapter {
    fn save_podcast_capture(
        &mut self,
        capture: &PodcastCapture,
    ) -> Result<PodcastCaptureResult, Self::Error>;

    fn podcast_contexts(&self, word_id: &WordId) -> Result<Vec<PodcastContextRecord>, Self::Error>;
}

/// Optional read model used by vocabulary-library and review-history surfaces.
pub trait LibraryStorageAdapter: StorageAdapter {
    fn library_page(&self, query: &LibraryQuery) -> Result<LibraryPage, Self::Error>;

    fn review_card(&self, word_id: &WordId) -> Result<Option<ReviewCard>, Self::Error>;

    fn due_review_batch(
        &self,
        as_of: DateTime<Utc>,
        limit: u32,
    ) -> Result<Vec<ReviewCard>, Self::Error>;

    fn review_history(&self, query: &ReviewHistoryQuery) -> Result<ReviewHistoryPage, Self::Error>;

    fn review_activity(
        &self,
        from: DateTime<Utc>,
        before: DateTime<Utc>,
    ) -> Result<ReviewActivity, Self::Error>;
}
