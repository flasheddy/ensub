use std::collections::BTreeMap;
use std::io;

use ensub_wasm::{
    CaptureParsedInput, DueReviewsInput, ParseInput, ReviewInput, Sandbox, SandboxError,
    SnapshotAccess, SnapshotBackend, SnapshotMigrationStatus, StatsInput,
};
use language_engine::{
    BrowserLexiconAsset, BrowserLexiconForm, Definition, LexiconEntry,
    BROWSER_LEXICON_SCHEMA_VERSION,
};

#[derive(Default)]
struct MemoryBackend(BTreeMap<String, String>);

impl SnapshotBackend for MemoryBackend {
    type Error = io::Error;

    fn load(&self, key: &str) -> Result<Option<String>, Self::Error> {
        Ok(self.0.get(key).cloned())
    }

    fn store(&mut self, key: &str, value: &str) -> Result<(), Self::Error> {
        self.0.insert(key.to_string(), value.to_string());
        Ok(())
    }

    fn remove(&mut self, key: &str) -> Result<(), Self::Error> {
        self.0.remove(key);
        Ok(())
    }
}

fn lexicon_bytes() -> Vec<u8> {
    BrowserLexiconAsset {
        schema_version: BROWSER_LEXICON_SCHEMA_VERSION,
        definition_source: "test dictionary".to_string(),
        pronunciation_source: "test pronunciation".to_string(),
        entries: vec![
            LexiconEntry {
                lemma: "immersion".to_string(),
                phonetic: "ɪˈmɜːʃən".to_string(),
                definitions: vec![Definition {
                    part_of_speech: "noun".to_string(),
                    text: "deep involvement in an activity".to_string(),
                }],
            },
            LexiconEntry {
                lemma: "learn".to_string(),
                phonetic: "lɜːn".to_string(),
                definitions: vec![Definition {
                    part_of_speech: "verb".to_string(),
                    text: "gain knowledge or skill".to_string(),
                }],
            },
        ],
        forms: vec![
            BrowserLexiconForm {
                surface: "immersion".to_string(),
                entry_index: 0,
                priority: 0,
            },
            BrowserLexiconForm {
                surface: "learn".to_string(),
                entry_index: 1,
                priority: 0,
            },
            BrowserLexiconForm {
                surface: "learning".to_string(),
                entry_index: 1,
                priority: 0,
            },
        ],
    }
    .encode()
    .expect("fixture lexicon must encode")
}

fn sandbox() -> Sandbox<MemoryBackend> {
    Sandbox::open(
        MemoryBackend::default(),
        "ensub.test",
        SnapshotAccess::ReadWrite,
        &lexicon_bytes(),
    )
    .expect("sandbox must open")
}

#[test]
fn sandbox_facade_exposes_storage_initialization_and_recovery_export() {
    let raw_v1 = include_str!("fixtures/browser-snapshot-v1.json").to_string();
    let mut backend = MemoryBackend::default();
    backend.0.insert("ensub.test".to_string(), raw_v1);
    let mut sandbox = Sandbox::open(
        backend,
        "ensub.test",
        SnapshotAccess::ReadWrite,
        &lexicon_bytes(),
    )
    .expect("sandbox must open");

    assert_eq!(
        sandbox
            .initialize_storage()
            .expect("storage migration must succeed"),
        SnapshotMigrationStatus::Migrated
    );
    assert!(!sandbox.is_read_only());

    let corrupt = "{not-json".to_string();
    let mut backend = MemoryBackend::default();
    backend.0.insert("ensub.test".to_string(), corrupt.clone());
    let mut recovery = Sandbox::open(
        backend,
        "ensub.test",
        SnapshotAccess::ReadWrite,
        &lexicon_bytes(),
    )
    .expect("sandbox must open");
    assert!(recovery.initialize_storage().is_err());
    assert!(recovery.is_read_only());
    assert_eq!(
        recovery.raw_snapshot().expect("raw snapshot must load"),
        Some(corrupt)
    );
}

#[test]
fn parse_returns_stable_ids_and_utf16_offsets_for_javascript() {
    let sandbox = sandbox();
    let text = "📚 Learning through immersion works.";
    let request = ParseInput {
        text: text.to_string(),
        include_stopwords: false,
        max_candidates: 20,
    };
    let first = sandbox.parse(&request).expect("text must parse");
    let second = sandbox.parse(&request).expect("text must parse again");

    assert_eq!(first, second);
    assert_eq!(first.candidates.len(), 2);
    assert_eq!(first.candidates[0].surface, "Learning");
    assert_eq!(first.candidates[0].token_start, 3);
    assert_eq!(first.candidates[0].token_end, 11);
    assert_eq!(first.candidates[0].sentence_start, 0);
    assert_eq!(first.candidates[0].sentence_end, 36);
    assert_eq!(first.candidates[0].definitions[0].part_of_speech, "verb");
    assert!(!first.candidates[0].id.is_empty());
}

#[test]
fn selected_candidates_are_captured_atomically_and_become_due() {
    let mut sandbox = sandbox();
    let text = "Learning through immersion works.";
    let parsed = sandbox
        .parse(&ParseInput {
            text: text.to_string(),
            include_stopwords: false,
            max_candidates: 20,
        })
        .expect("text must parse");
    let selected = vec![parsed.candidates[1].id.clone()];
    let captured = sandbox
        .capture_parsed(&CaptureParsedInput {
            text: text.to_string(),
            candidate_ids: selected,
            source: "sandbox".to_string(),
            captured_at_ms: 1_755_244_800_000,
            include_stopwords: false,
            max_candidates: 20,
        })
        .expect("selection must save");

    assert_eq!(captured.captured_cards, 1);
    assert_eq!(captured.created_cards, 1);
    assert_eq!(captured.created_contexts, 1);
    let due = sandbox
        .due_reviews(&DueReviewsInput {
            as_of_ms: 1_755_244_800_000,
            limit: 10,
        })
        .expect("due cards must load");
    assert_eq!(due.cards.len(), 1);
    assert_eq!(due.cards[0].lemma, "immersion");
    assert_eq!(due.cards[0].contexts[0].source, "sandbox");
}

#[test]
fn invalid_candidate_id_rejects_the_whole_capture() {
    let mut sandbox = sandbox();
    let result = sandbox.capture_parsed(&CaptureParsedInput {
        text: "Learning through immersion works.".to_string(),
        candidate_ids: vec!["not-a-candidate".to_string()],
        source: "sandbox".to_string(),
        captured_at_ms: 1_755_244_800_000,
        include_stopwords: false,
        max_candidates: 20,
    });

    assert!(matches!(result, Err(SandboxError::UnknownCandidate(_))));
    assert_eq!(
        sandbox
            .stats(&StatsInput {
                as_of_ms: 1_755_244_800_000
            })
            .expect("stats must load")
            .total_cards,
        0
    );
}

#[test]
fn review_uses_opaque_state_token_and_rejects_stale_replays() {
    let mut sandbox = sandbox();
    let text = "Immersion works.";
    let candidate = sandbox
        .parse(&ParseInput {
            text: text.to_string(),
            include_stopwords: false,
            max_candidates: 20,
        })
        .expect("text must parse")
        .candidates
        .remove(0);
    sandbox
        .capture_parsed(&CaptureParsedInput {
            text: text.to_string(),
            candidate_ids: vec![candidate.id],
            source: "sandbox".to_string(),
            captured_at_ms: 1_755_244_800_000,
            include_stopwords: false,
            max_candidates: 20,
        })
        .expect("capture must save");
    let card = sandbox
        .due_reviews(&DueReviewsInput {
            as_of_ms: 1_755_244_800_000,
            limit: 1,
        })
        .expect("due card must load")
        .cards
        .remove(0);
    assert!(!card.review_token.contains("wordId"));

    let request = ReviewInput {
        word_id: card.word_id,
        review_token: card.review_token,
        rating: 4,
        reviewed_at_ms: 1_755_244_800_000,
    };
    let reviewed = sandbox.review(&request).expect("review must save");
    assert_eq!(reviewed.interval_days, 1);
    assert_eq!(reviewed.repetitions, 1);
    assert_eq!(reviewed.next_review_at_ms, 1_755_331_200_000);
    assert!(matches!(
        sandbox.review(&request),
        Err(SandboxError::ReviewConflict)
    ));
}

#[test]
fn stats_next_due_and_reset_follow_the_virtual_clock() {
    let mut sandbox = sandbox();
    let text = "Immersion works.";
    let candidate = sandbox
        .parse(&ParseInput {
            text: text.to_string(),
            include_stopwords: false,
            max_candidates: 20,
        })
        .expect("text must parse")
        .candidates
        .remove(0);
    sandbox
        .capture_parsed(&CaptureParsedInput {
            text: text.to_string(),
            candidate_ids: vec![candidate.id],
            source: "sandbox".to_string(),
            captured_at_ms: 1_755_244_800_000,
            include_stopwords: false,
            max_candidates: 20,
        })
        .expect("capture must save");

    let stats = sandbox
        .stats(&StatsInput {
            as_of_ms: 1_755_244_799_999,
        })
        .expect("stats must load");
    assert_eq!(stats.total_cards, 1);
    assert_eq!(stats.due_cards, 0);
    assert_eq!(stats.next_review_at_ms, Some(1_755_244_800_000));

    sandbox.reset().expect("reset must succeed");
    assert_eq!(
        sandbox
            .stats(&StatsInput {
                as_of_ms: 1_755_244_800_000
            })
            .expect("stats must load")
            .total_cards,
        0
    );
}
