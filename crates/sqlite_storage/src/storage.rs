use std::path::Path;
use std::time::Duration;

use chrono::{DateTime, Utc};
use core_engine::{
    Capture, CaptureResult, ContextId, ContextRecord, DailyReviewActivity, IntervalDistribution,
    LibraryOrder, LibraryPage, LibraryQuery, LibraryStorageAdapter, ReviewActivity, ReviewCard,
    ReviewHistoryEntry, ReviewHistoryPage, ReviewHistoryQuery, ReviewRating, ReviewState,
    ReviewStatistics, ReviewUpdate, StorageAdapter, WordId, WordRecord, MIN_EASE_FACTOR,
};
use rusqlite::{params, Connection, ErrorCode, OptionalExtension, TransactionBehavior};

use crate::SqliteError;

const SCHEMA_VERSION: i64 = 2;
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const INITIALIZATION_RETRY_DELAYS: [Duration; 5] = [
    Duration::from_millis(10),
    Duration::from_millis(20),
    Duration::from_millis(40),
    Duration::from_millis(80),
    Duration::from_millis(160),
];

pub struct SqliteStorage {
    connection: Connection,
}

impl SqliteStorage {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SqliteError> {
        let path = path.as_ref();
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent).map_err(|error| SqliteError::io(parent, error))?;
        }

        let connection = Connection::open(path)?;
        Self::from_connection(connection)
    }

    pub fn open_in_memory() -> Result<Self, SqliteError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(mut connection: Connection) -> Result<Self, SqliteError> {
        connection.busy_timeout(BUSY_TIMEOUT)?;
        initialize_connection(&mut connection)?;
        Ok(Self { connection })
    }
}

fn initialize_connection(connection: &mut Connection) -> Result<(), SqliteError> {
    let mut retry_delays = INITIALIZATION_RETRY_DELAYS.into_iter();
    loop {
        match initialize_connection_once(connection) {
            Ok(()) => return Ok(()),
            Err(error) if is_database_busy(&error) => {
                let Some(delay) = retry_delays.next() else {
                    return Err(error);
                };
                std::thread::sleep(delay);
            }
            Err(error) => return Err(error),
        }
    }
}

fn initialize_connection_once(connection: &mut Connection) -> Result<(), SqliteError> {
    connection.pragma_update(None, "foreign_keys", "ON")?;
    let _: String = connection.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
    connection.pragma_update(None, "synchronous", "NORMAL")?;
    migrate(connection)
}

fn is_database_busy(error: &SqliteError) -> bool {
    matches!(
        error,
        SqliteError::Database(rusqlite::Error::SqliteFailure(sqlite_error, _))
            if sqlite_error.code == ErrorCode::DatabaseBusy
    )
}

impl StorageAdapter for SqliteStorage {
    type Error = SqliteError;

    fn save_word(&mut self, word: &WordRecord) -> Result<(), Self::Error> {
        save_word(&self.connection, word)
    }

    fn save_context(&mut self, context: &ContextRecord) -> Result<(), Self::Error> {
        save_context(&self.connection, context).map(|_| ())
    }

    fn save_review_state(&mut self, state: &ReviewState) -> Result<(), Self::Error> {
        validate_state(state)?;
        self.connection.execute(
            "INSERT INTO review_state (
                word_id, ease_factor, repetitions, interval_days, next_review_at, last_rating
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(word_id) DO UPDATE SET
                ease_factor = excluded.ease_factor,
                repetitions = excluded.repetitions,
                interval_days = excluded.interval_days,
                next_review_at = excluded.next_review_at,
                last_rating = excluded.last_rating",
            params![
                state.word_id.as_str(),
                state.ease_factor,
                i64::from(state.repetitions),
                i64::from(state.interval_days),
                to_millis(state.next_review_at),
                state.last_rating.map(|rating| i64::from(rating.value())),
            ],
        )?;
        Ok(())
    }

    fn save_capture(&mut self, capture: &Capture) -> Result<CaptureResult, Self::Error> {
        let mut results = self.save_captures(std::slice::from_ref(capture))?;
        results.pop().ok_or(SqliteError::InvalidCapture)
    }

    fn save_captures(&mut self, captures: &[Capture]) -> Result<Vec<CaptureResult>, Self::Error> {
        for capture in captures {
            validate_capture(capture)?;
        }

        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut results = Vec::with_capacity(captures.len());
        for capture in captures {
            results.push(save_capture(&transaction, capture)?);
        }
        transaction.commit()?;
        Ok(results)
    }

    fn compare_and_swap_review_state(
        &mut self,
        expected: &ReviewState,
        replacement: &ReviewState,
    ) -> Result<ReviewUpdate, Self::Error> {
        validate_review_transition(expected, replacement)?;
        let changed = update_review_state(&self.connection, expected, replacement)?;

        Ok(if changed == 1 {
            ReviewUpdate::Updated
        } else {
            ReviewUpdate::Conflict
        })
    }

    fn commit_review(
        &mut self,
        expected: &ReviewState,
        replacement: &ReviewState,
        reviewed_at: DateTime<Utc>,
    ) -> Result<ReviewUpdate, Self::Error> {
        validate_review_transition(expected, replacement)?;
        let rating = replacement
            .last_rating
            .ok_or(SqliteError::MissingCommittedReviewRating)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if update_review_state(&transaction, expected, replacement)? == 0 {
            transaction.rollback()?;
            return Ok(ReviewUpdate::Conflict);
        }
        transaction.execute(
            "INSERT INTO review_events (
                word_id, reviewed_at, rating,
                previous_ease_factor, previous_repetitions, previous_interval_days,
                previous_next_review_at, previous_last_rating,
                resulting_ease_factor, resulting_repetitions, resulting_interval_days,
                resulting_next_review_at, resulting_last_rating
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                expected.word_id.as_str(),
                to_millis(reviewed_at),
                i64::from(rating.value()),
                expected.ease_factor,
                i64::from(expected.repetitions),
                i64::from(expected.interval_days),
                to_millis(expected.next_review_at),
                expected.last_rating.map(|value| i64::from(value.value())),
                replacement.ease_factor,
                i64::from(replacement.repetitions),
                i64::from(replacement.interval_days),
                to_millis(replacement.next_review_at),
                replacement
                    .last_rating
                    .map(|value| i64::from(value.value())),
            ],
        )?;
        transaction.commit()?;
        Ok(ReviewUpdate::Updated)
    }

    fn review_state(&self, word_id: &WordId) -> Result<Option<ReviewState>, Self::Error> {
        let raw = self
            .connection
            .query_row(
                "SELECT ease_factor, repetitions, interval_days, next_review_at, last_rating
                 FROM review_state
                 WHERE word_id = ?1",
                [word_id.as_str()],
                |row| {
                    Ok((
                        row.get::<_, f64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, Option<i64>>(4)?,
                    ))
                },
            )
            .optional()?;

        raw.map(
            |(ease_factor, repetitions, interval_days, next_review_at, last_rating)| {
                Ok(ReviewState {
                    word_id: word_id.clone(),
                    ease_factor: valid_ease_factor(ease_factor)?,
                    repetitions: nonnegative_u32("repetitions", repetitions)?,
                    interval_days: nonnegative_u32("interval days", interval_days)?,
                    next_review_at: from_millis(next_review_at)?,
                    last_rating: decode_rating(last_rating)?,
                })
            },
        )
        .transpose()
    }

    fn due_reviews(&self, as_of: DateTime<Utc>) -> Result<Vec<ReviewCard>, Self::Error> {
        let mut statement = self.connection.prepare(
            "SELECT
                w.id, w.term, w.lemma, w.phonetic, w.definition, w.created_at,
                r.ease_factor, r.repetitions, r.interval_days, r.next_review_at, r.last_rating
             FROM review_state r
             JOIN words w ON w.id = r.word_id
             WHERE r.next_review_at <= ?1
             ORDER BY r.next_review_at ASC, w.id ASC",
        )?;
        let raw_cards = statement
            .query_map([to_millis(as_of)], |row| {
                Ok(RawCard {
                    word_id: row.get(0)?,
                    term: row.get(1)?,
                    lemma: row.get(2)?,
                    phonetic: row.get(3)?,
                    definition: row.get(4)?,
                    created_at: row.get(5)?,
                    ease_factor: row.get(6)?,
                    repetitions: row.get(7)?,
                    interval_days: row.get(8)?,
                    next_review_at: row.get(9)?,
                    last_rating: row.get(10)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);

        raw_cards
            .into_iter()
            .map(|raw| self.build_card(raw))
            .collect()
    }

    fn due_count(&self, as_of: DateTime<Utc>) -> Result<u64, Self::Error> {
        let value: i64 = self.connection.query_row(
            "SELECT COUNT(*) FROM review_state WHERE next_review_at <= ?1",
            [to_millis(as_of)],
            |row| row.get(0),
        )?;
        nonnegative_u64("due count", value)
    }

    fn review_statistics(&self, as_of: DateTime<Utc>) -> Result<ReviewStatistics, Self::Error> {
        let values: (i64, i64, i64, i64, i64, i64, i64) = self.connection.query_row(
            "SELECT
                COUNT(*),
                COALESCE(SUM(CASE WHEN next_review_at <= ?1 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN interval_days = 0 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN interval_days BETWEEN 1 AND 6 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN interval_days BETWEEN 7 AND 30 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN interval_days BETWEEN 31 AND 90 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN interval_days >= 91 THEN 1 ELSE 0 END), 0)
             FROM review_state",
            [to_millis(as_of)],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                ))
            },
        )?;

        Ok(ReviewStatistics {
            total_cards: nonnegative_u64("total cards", values.0)?,
            due_cards: nonnegative_u64("due cards", values.1)?,
            intervals: IntervalDistribution {
                new: nonnegative_u64("new interval bucket", values.2)?,
                days_1_to_6: nonnegative_u64("1-6 day interval bucket", values.3)?,
                days_7_to_30: nonnegative_u64("7-30 day interval bucket", values.4)?,
                days_31_to_90: nonnegative_u64("31-90 day interval bucket", values.5)?,
                days_91_plus: nonnegative_u64("91+ day interval bucket", values.6)?,
            },
        })
    }
}

impl LibraryStorageAdapter for SqliteStorage {
    fn library_page(&self, query: &LibraryQuery) -> Result<LibraryPage, Self::Error> {
        let pattern = escaped_like_pattern(&query.search);
        let predicate = "(?1 = '%%' OR w.term LIKE ?1 ESCAPE '\\' COLLATE NOCASE
            OR w.lemma LIKE ?1 ESCAPE '\\' COLLATE NOCASE
            OR w.definition LIKE ?1 ESCAPE '\\' COLLATE NOCASE
            OR EXISTS (
                SELECT 1 FROM contexts search_context
                WHERE search_context.word_id = w.id
                  AND (search_context.sentence LIKE ?1 ESCAPE '\\' COLLATE NOCASE
                       OR search_context.source LIKE ?1 ESCAPE '\\' COLLATE NOCASE)
            ))";
        let count_sql = format!(
            "SELECT COUNT(*) FROM words w JOIN review_state r ON r.word_id = w.id WHERE {predicate}"
        );
        let total_raw: i64 = self
            .connection
            .query_row(&count_sql, [&pattern], |row| row.get(0))?;
        let order = match query.order {
            LibraryOrder::RecentlyCaptured => "w.created_at DESC, w.id ASC",
            LibraryOrder::Alphabetical => "w.term COLLATE NOCASE ASC, w.id ASC",
            LibraryOrder::DueFirst => "r.next_review_at ASC, w.id ASC",
        };
        let select_sql = format!(
            "SELECT
                w.id, w.term, w.lemma, w.phonetic, w.definition, w.created_at,
                r.ease_factor, r.repetitions, r.interval_days, r.next_review_at, r.last_rating
             FROM words w JOIN review_state r ON r.word_id = w.id
             WHERE {predicate}
             ORDER BY {order}
             LIMIT ?2 OFFSET ?3"
        );
        let mut statement = self.connection.prepare(&select_sql)?;
        let raw_cards = statement
            .query_map(
                params![
                    pattern,
                    i64::from(query.limit),
                    bounded_offset(query.offset)
                ],
                raw_card_from_row,
            )?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        let cards = raw_cards
            .into_iter()
            .map(|raw| self.build_card(raw))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(LibraryPage {
            cards,
            total: nonnegative_u64("library total", total_raw)?,
            offset: query.offset,
            limit: query.limit,
        })
    }

    fn review_card(&self, word_id: &WordId) -> Result<Option<ReviewCard>, Self::Error> {
        let raw = self
            .connection
            .query_row(
                "SELECT
                    w.id, w.term, w.lemma, w.phonetic, w.definition, w.created_at,
                    r.ease_factor, r.repetitions, r.interval_days, r.next_review_at, r.last_rating
                 FROM words w JOIN review_state r ON r.word_id = w.id
                 WHERE w.id = ?1",
                [word_id.as_str()],
                raw_card_from_row,
            )
            .optional()?;
        raw.map(|value| self.build_card(value)).transpose()
    }

    fn due_review_batch(
        &self,
        as_of: DateTime<Utc>,
        limit: u32,
    ) -> Result<Vec<ReviewCard>, Self::Error> {
        let mut statement = self.connection.prepare(
            "SELECT
                w.id, w.term, w.lemma, w.phonetic, w.definition, w.created_at,
                r.ease_factor, r.repetitions, r.interval_days, r.next_review_at, r.last_rating
             FROM review_state r JOIN words w ON w.id = r.word_id
             WHERE r.next_review_at <= ?1
             ORDER BY r.next_review_at ASC, w.id ASC
             LIMIT ?2",
        )?;
        let raw_cards = statement
            .query_map(
                params![to_millis(as_of), i64::from(limit)],
                raw_card_from_row,
            )?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        raw_cards
            .into_iter()
            .map(|raw| self.build_card(raw))
            .collect()
    }

    fn review_history(&self, query: &ReviewHistoryQuery) -> Result<ReviewHistoryPage, Self::Error> {
        let total_raw: i64 = match &query.word_id {
            Some(word_id) => self.connection.query_row(
                "SELECT COUNT(*) FROM review_events WHERE word_id = ?1",
                [word_id.as_str()],
                |row| row.get(0),
            )?,
            None => self
                .connection
                .query_row("SELECT COUNT(*) FROM review_events", [], |row| row.get(0))?,
        };
        let sql = format!(
            "SELECT
                e.id, w.id, w.term, w.lemma, w.phonetic, w.definition, w.created_at,
                e.reviewed_at, e.rating,
                e.previous_ease_factor, e.previous_repetitions, e.previous_interval_days,
                e.previous_next_review_at, e.previous_last_rating,
                e.resulting_ease_factor, e.resulting_repetitions, e.resulting_interval_days,
                e.resulting_next_review_at, e.resulting_last_rating
             FROM review_events e JOIN words w ON w.id = e.word_id
             {}
             ORDER BY e.reviewed_at DESC, e.id DESC
             LIMIT ?2 OFFSET ?3",
            if query.word_id.is_some() {
                "WHERE e.word_id = ?1"
            } else {
                "WHERE ?1 IS NULL"
            }
        );
        let mut statement = self.connection.prepare(&sql)?;
        let word_parameter = query.word_id.as_ref().map(WordId::as_str);
        let rows = statement
            .query_map(
                params![
                    word_parameter,
                    i64::from(query.limit),
                    bounded_offset(query.offset)
                ],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, i64>(6)?,
                        row.get::<_, i64>(7)?,
                        row.get::<_, i64>(8)?,
                        row.get::<_, f64>(9)?,
                        row.get::<_, i64>(10)?,
                        row.get::<_, i64>(11)?,
                        row.get::<_, i64>(12)?,
                        row.get::<_, Option<i64>>(13)?,
                        row.get::<_, f64>(14)?,
                        row.get::<_, i64>(15)?,
                        row.get::<_, i64>(16)?,
                        row.get::<_, i64>(17)?,
                        row.get::<_, Option<i64>>(18)?,
                    ))
                },
            )?
            .collect::<Result<Vec<_>, _>>()?;
        let mut entries = Vec::with_capacity(rows.len());
        for row in rows {
            let word_id = WordId::new(row.1);
            entries.push(ReviewHistoryEntry {
                sequence: nonnegative_u64("review event sequence", row.0)?,
                word: WordRecord {
                    id: word_id.clone(),
                    term: row.2,
                    lemma: row.3,
                    phonetic: row.4,
                    definition: row.5,
                    created_at: from_millis(row.6)?,
                },
                reviewed_at: from_millis(row.7)?,
                rating: decode_required_rating(row.8)?,
                previous_state: decode_state(&word_id, row.9, row.10, row.11, row.12, row.13)?,
                resulting_state: decode_state(&word_id, row.14, row.15, row.16, row.17, row.18)?,
            });
        }
        Ok(ReviewHistoryPage {
            entries,
            total: nonnegative_u64("review history total", total_raw)?,
            offset: query.offset,
            limit: query.limit,
        })
    }

    fn review_activity(
        &self,
        from: DateTime<Utc>,
        before: DateTime<Utc>,
    ) -> Result<ReviewActivity, Self::Error> {
        use std::collections::BTreeMap;

        let mut statement = self.connection.prepare(
            "SELECT reviewed_at, rating FROM review_events
             WHERE reviewed_at >= ?1 AND reviewed_at < ?2
             ORDER BY reviewed_at ASC, id ASC",
        )?;
        let rows = statement
            .query_map(params![to_millis(from), to_millis(before)], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut by_day = BTreeMap::new();
        let mut activity = ReviewActivity::default();
        for (reviewed_at, raw_rating) in rows {
            let date = from_millis(reviewed_at)?.date_naive();
            let rating = decode_required_rating(raw_rating)?;
            let passing = u64::from(rating.value() >= 3);
            activity.total_reviews = activity.total_reviews.saturating_add(1);
            activity.passing_reviews = activity.passing_reviews.saturating_add(passing);
            activity.ratings[usize::from(rating.value())] =
                activity.ratings[usize::from(rating.value())].saturating_add(1);
            let day = by_day.entry(date).or_insert_with(|| DailyReviewActivity {
                date,
                reviews: 0,
                passing_reviews: 0,
                ratings: [0; 6],
            });
            day.reviews = day.reviews.saturating_add(1);
            day.passing_reviews = day.passing_reviews.saturating_add(passing);
            day.ratings[usize::from(rating.value())] =
                day.ratings[usize::from(rating.value())].saturating_add(1);
        }
        activity.days = by_day.into_values().collect();
        Ok(activity)
    }
}

impl SqliteStorage {
    fn build_card(&self, raw: RawCard) -> Result<ReviewCard, SqliteError> {
        let word_id = WordId::new(raw.word_id);
        let word = WordRecord {
            id: word_id.clone(),
            term: raw.term,
            lemma: raw.lemma,
            phonetic: raw.phonetic,
            definition: raw.definition,
            created_at: from_millis(raw.created_at)?,
        };
        let state = ReviewState {
            word_id: word_id.clone(),
            ease_factor: valid_ease_factor(raw.ease_factor)?,
            repetitions: nonnegative_u32("repetitions", raw.repetitions)?,
            interval_days: nonnegative_u32("interval days", raw.interval_days)?,
            next_review_at: from_millis(raw.next_review_at)?,
            last_rating: decode_rating(raw.last_rating)?,
        };
        let contexts = self.load_contexts(&word_id)?;
        Ok(ReviewCard {
            word,
            contexts,
            state,
        })
    }

    fn load_contexts(&self, word_id: &WordId) -> Result<Vec<ContextRecord>, SqliteError> {
        let mut statement = self.connection.prepare(
            "SELECT id, sentence, source, captured_at
             FROM contexts
             WHERE word_id = ?1
             ORDER BY captured_at DESC, id ASC",
        )?;
        let raw = statement
            .query_map([word_id.as_str()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        raw.into_iter()
            .map(|(id, sentence, source, captured_at)| {
                Ok(ContextRecord {
                    id: ContextId::new(id),
                    word_id: word_id.clone(),
                    sentence,
                    source,
                    captured_at: from_millis(captured_at)?,
                })
            })
            .collect()
    }
}

struct RawCard {
    word_id: String,
    term: String,
    lemma: String,
    phonetic: String,
    definition: String,
    created_at: i64,
    ease_factor: f64,
    repetitions: i64,
    interval_days: i64,
    next_review_at: i64,
    last_rating: Option<i64>,
}

fn migrate(connection: &mut Connection) -> Result<(), SqliteError> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let version: i64 = transaction.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version > SCHEMA_VERSION {
        return Err(SqliteError::UnsupportedSchema(version));
    }
    if version == SCHEMA_VERSION {
        transaction.commit()?;
        return Ok(());
    }
    if version == 0 {
        transaction.execute_batch(
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
             CREATE INDEX review_state_due_idx ON review_state(next_review_at, word_id);",
        )?;
    }
    transaction.execute_batch(
        "CREATE TABLE review_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            word_id TEXT NOT NULL REFERENCES words(id) ON DELETE CASCADE,
            reviewed_at INTEGER NOT NULL,
            rating INTEGER NOT NULL CHECK(rating BETWEEN 0 AND 5),
            previous_ease_factor REAL NOT NULL CHECK(previous_ease_factor >= 1.3),
            previous_repetitions INTEGER NOT NULL CHECK(previous_repetitions >= 0),
            previous_interval_days INTEGER NOT NULL CHECK(previous_interval_days >= 0),
            previous_next_review_at INTEGER NOT NULL,
            previous_last_rating INTEGER CHECK(previous_last_rating IS NULL OR previous_last_rating BETWEEN 0 AND 5),
            resulting_ease_factor REAL NOT NULL CHECK(resulting_ease_factor >= 1.3),
            resulting_repetitions INTEGER NOT NULL CHECK(resulting_repetitions >= 0),
            resulting_interval_days INTEGER NOT NULL CHECK(resulting_interval_days >= 0),
            resulting_next_review_at INTEGER NOT NULL,
            resulting_last_rating INTEGER CHECK(resulting_last_rating IS NULL OR resulting_last_rating BETWEEN 0 AND 5)
         ) STRICT;
         CREATE INDEX review_events_global_idx ON review_events(reviewed_at DESC, id DESC);
         CREATE INDEX review_events_word_idx ON review_events(word_id, reviewed_at DESC, id DESC);",
    )?;
    transaction.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    transaction.commit()?;
    Ok(())
}

fn validate_review_transition(
    expected: &ReviewState,
    replacement: &ReviewState,
) -> Result<(), SqliteError> {
    if expected.word_id != replacement.word_id {
        return Err(SqliteError::InvalidReviewReplacement);
    }
    validate_state(expected)?;
    validate_state(replacement)
}

fn update_review_state(
    connection: &Connection,
    expected: &ReviewState,
    replacement: &ReviewState,
) -> Result<usize, SqliteError> {
    Ok(connection.execute(
        "UPDATE review_state SET
            ease_factor = ?1, repetitions = ?2, interval_days = ?3,
            next_review_at = ?4, last_rating = ?5
         WHERE word_id = ?6 AND next_review_at = ?7 AND repetitions = ?8
           AND interval_days = ?9 AND last_rating IS ?10 AND ease_factor = ?11",
        params![
            replacement.ease_factor,
            i64::from(replacement.repetitions),
            i64::from(replacement.interval_days),
            to_millis(replacement.next_review_at),
            replacement
                .last_rating
                .map(|value| i64::from(value.value())),
            replacement.word_id.as_str(),
            to_millis(expected.next_review_at),
            i64::from(expected.repetitions),
            i64::from(expected.interval_days),
            expected.last_rating.map(|value| i64::from(value.value())),
            expected.ease_factor,
        ],
    )?)
}

fn raw_card_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawCard> {
    Ok(RawCard {
        word_id: row.get(0)?,
        term: row.get(1)?,
        lemma: row.get(2)?,
        phonetic: row.get(3)?,
        definition: row.get(4)?,
        created_at: row.get(5)?,
        ease_factor: row.get(6)?,
        repetitions: row.get(7)?,
        interval_days: row.get(8)?,
        next_review_at: row.get(9)?,
        last_rating: row.get(10)?,
    })
}

fn escaped_like_pattern(search: &str) -> String {
    let escaped = search
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}

fn bounded_offset(offset: u64) -> i64 {
    i64::try_from(offset).unwrap_or(i64::MAX)
}

fn decode_state(
    word_id: &WordId,
    ease_factor: f64,
    repetitions: i64,
    interval_days: i64,
    next_review_at: i64,
    last_rating: Option<i64>,
) -> Result<ReviewState, SqliteError> {
    Ok(ReviewState {
        word_id: word_id.clone(),
        ease_factor: valid_ease_factor(ease_factor)?,
        repetitions: nonnegative_u32("repetitions", repetitions)?,
        interval_days: nonnegative_u32("interval days", interval_days)?,
        next_review_at: from_millis(next_review_at)?,
        last_rating: decode_rating(last_rating)?,
    })
}

fn decode_required_rating(value: i64) -> Result<ReviewRating, SqliteError> {
    decode_rating(Some(value))?.ok_or(SqliteError::InvalidInteger {
        field: "review rating",
        value,
    })
}

fn validate_capture(capture: &Capture) -> Result<(), SqliteError> {
    if capture.initial_review_state.word_id != capture.word.id
        || capture
            .contexts
            .iter()
            .any(|context| context.word_id != capture.word.id)
    {
        return Err(SqliteError::InvalidCapture);
    }
    validate_state(&capture.initial_review_state)
}

fn validate_state(state: &ReviewState) -> Result<(), SqliteError> {
    valid_ease_factor(state.ease_factor).map(|_| ())
}

fn valid_ease_factor(value: f64) -> Result<f64, SqliteError> {
    if value.is_finite() && value >= MIN_EASE_FACTOR {
        Ok(value)
    } else {
        Err(SqliteError::InvalidEaseFactor(value))
    }
}

fn save_capture(connection: &Connection, capture: &Capture) -> Result<CaptureResult, SqliteError> {
    let word_created = connection.execute(
        "INSERT OR IGNORE INTO words (id, term, lemma, phonetic, definition, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            capture.word.id.as_str(),
            capture.word.term,
            capture.word.lemma,
            capture.word.phonetic,
            capture.word.definition,
            to_millis(capture.word.created_at),
        ],
    )? == 1;
    if !word_created {
        connection.execute(
            "UPDATE words SET lemma = ?1, phonetic = ?2, definition = ?3 WHERE id = ?4",
            params![
                capture.word.lemma,
                capture.word.phonetic,
                capture.word.definition,
                capture.word.id.as_str(),
            ],
        )?;
    }

    let mut contexts_created = 0_u64;
    for context in &capture.contexts {
        contexts_created =
            contexts_created.saturating_add(save_context(connection, context)? as u64);
    }

    connection.execute(
        "INSERT OR IGNORE INTO review_state (
            word_id, ease_factor, repetitions, interval_days, next_review_at, last_rating
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            capture.initial_review_state.word_id.as_str(),
            capture.initial_review_state.ease_factor,
            i64::from(capture.initial_review_state.repetitions),
            i64::from(capture.initial_review_state.interval_days),
            to_millis(capture.initial_review_state.next_review_at),
            capture
                .initial_review_state
                .last_rating
                .map(|rating| i64::from(rating.value())),
        ],
    )?;

    Ok(CaptureResult {
        word_created,
        contexts_created,
    })
}

fn save_word(connection: &Connection, word: &WordRecord) -> Result<(), SqliteError> {
    connection.execute(
        "INSERT INTO words (id, term, lemma, phonetic, definition, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(id) DO UPDATE SET
            term = excluded.term,
            lemma = excluded.lemma,
            phonetic = excluded.phonetic,
            definition = excluded.definition,
            created_at = excluded.created_at",
        params![
            word.id.as_str(),
            word.term,
            word.lemma,
            word.phonetic,
            word.definition,
            to_millis(word.created_at),
        ],
    )?;
    Ok(())
}

fn save_context(connection: &Connection, context: &ContextRecord) -> Result<usize, SqliteError> {
    Ok(connection.execute(
        "INSERT OR IGNORE INTO contexts (id, word_id, sentence, source, captured_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            context.id.as_str(),
            context.word_id.as_str(),
            context.sentence,
            context.source,
            to_millis(context.captured_at),
        ],
    )?)
}

fn to_millis(value: DateTime<Utc>) -> i64 {
    value.timestamp_millis()
}

fn from_millis(value: i64) -> Result<DateTime<Utc>, SqliteError> {
    DateTime::from_timestamp_millis(value).ok_or(SqliteError::InvalidTimestamp(value))
}

fn nonnegative_u32(field: &'static str, value: i64) -> Result<u32, SqliteError> {
    u32::try_from(value).map_err(|_| SqliteError::InvalidInteger { field, value })
}

fn nonnegative_u64(field: &'static str, value: i64) -> Result<u64, SqliteError> {
    u64::try_from(value).map_err(|_| SqliteError::InvalidInteger { field, value })
}

fn decode_rating(value: Option<i64>) -> Result<Option<ReviewRating>, SqliteError> {
    value
        .map(|rating| {
            let rating = u8::try_from(rating).map_err(|_| SqliteError::InvalidInteger {
                field: "last rating",
                value: rating,
            })?;
            ReviewRating::try_from(rating).map_err(SqliteError::from)
        })
        .transpose()
}
