use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io;
use std::rc::Rc;

use core_engine::{
    calculate_padded_audio_slice, CueRange, PodcastContextDraft, PodcastContextQuality,
    PodcastEpisodeProvenance, PodcastFeedProvenance, TranscriptFormat, TranscriptProvenance,
    TranscriptToken,
};
use ensub_wasm::{
    CapturePodcastInput, CapturePodcastStatus, DueCountInputDto, DueReviewsInputDto,
    PlayerLearning, PlayerLearningError, PrepareDisambiguationInputDto, RateReviewInputDto,
    RevealReviewInputDto, SnapshotAccess, SnapshotBackend, TokenLookupDto,
    ValidateDisambiguationResponseInputDto,
};
use language_engine::{
    BrowserLexiconAsset, BrowserLexiconForm, Definition, LexiconEntry,
    BROWSER_LEXICON_SCHEMA_VERSION,
};

#[derive(Clone, Default)]
struct MemoryBackend {
    values: Rc<RefCell<BTreeMap<String, String>>>,
    race: Rc<RefCell<Option<RaceOnLoad>>>,
}

struct RaceOnLoad {
    key: String,
    replacement: String,
    loads: u8,
}

impl MemoryBackend {
    fn with_snapshot(key: &str, snapshot: String) -> Self {
        let backend = Self::default();
        backend
            .values
            .borrow_mut()
            .insert(key.to_string(), snapshot);
        backend
    }

    fn snapshot(&self, key: &str) -> String {
        self.values
            .borrow()
            .get(key)
            .cloned()
            .expect("test snapshot must exist")
    }

    fn replace_on_second_load(&self, key: &str, replacement: String) {
        *self.race.borrow_mut() = Some(RaceOnLoad {
            key: key.to_string(),
            replacement,
            loads: 0,
        });
    }
}

impl SnapshotBackend for MemoryBackend {
    type Error = io::Error;

    fn load(&self, key: &str) -> Result<Option<String>, Self::Error> {
        let replacement = self.race.borrow_mut().as_mut().and_then(|race| {
            if race.key != key {
                return None;
            }
            race.loads = race.loads.saturating_add(1);
            (race.loads == 2).then(|| race.replacement.clone())
        });
        if let Some(replacement) = replacement {
            self.values
                .borrow_mut()
                .insert(key.to_string(), replacement);
        }
        Ok(self.values.borrow().get(key).cloned())
    }

    fn store(&mut self, key: &str, value: &str) -> Result<(), Self::Error> {
        self.values
            .borrow_mut()
            .insert(key.to_string(), value.to_string());
        Ok(())
    }

    fn remove(&mut self, key: &str) -> Result<(), Self::Error> {
        self.values.borrow_mut().remove(key);
        Ok(())
    }
}

fn lexicon() -> Vec<u8> {
    BrowserLexiconAsset {
        schema_version: BROWSER_LEXICON_SCHEMA_VERSION,
        definition_source: "test".to_string(),
        pronunciation_source: "test".to_string(),
        entries: vec![LexiconEntry {
            lemma: "go".to_string(),
            phonetic: "go".to_string(),
            definitions: vec![Definition {
                part_of_speech: "verb".to_string(),
                text: "move".to_string(),
            }],
        }],
        forms: vec![BrowserLexiconForm {
            surface: "went".to_string(),
            entry_index: 0,
            priority: 0,
        }],
    }
    .encode()
    .expect("fixture lexicon must encode")
}

fn draft() -> PodcastContextDraft {
    let range = CueRange::try_new("cue-0".to_string(), "cue-1".to_string(), 1_000, 3_000)
        .expect("fixture range must be valid");
    PodcastContextDraft {
        sentence: "We went home.".to_string(),
        quality: PodcastContextQuality::CompleteSentence,
        feed: PodcastFeedProvenance {
            source_url: "https://example.test/feed.xml".to_string(),
            title: "Feed".to_string(),
        },
        episode: PodcastEpisodeProvenance {
            internal_id: "episode-1".to_string(),
            publisher_guid: None,
            title: "Episode".to_string(),
            published_at: None,
            enclosure_url: "https://example.test/audio.mp3".to_string(),
        },
        transcript: TranscriptProvenance {
            url: "https://example.test/transcript.vtt".to_string(),
            format: TranscriptFormat::WebVtt,
            language: Some("en".to_string()),
        },
        selected_cue_id: "cue-0".to_string(),
        selected_token: TranscriptToken::try_new("went".to_string(), 3, 7)
            .expect("fixture token must be valid"),
        cue_range: range.clone(),
        playback_position_ms: 1_500,
        audio_slice: calculate_padded_audio_slice(
            "https://example.test/audio.mp3".to_string(),
            &range,
            None,
        )
        .expect("fixture slice must be valid"),
    }
}

#[test]
fn offline_lookup_distinguishes_found_and_unknown_without_writing() {
    let learning = PlayerLearning::open(
        MemoryBackend::default(),
        "ensub.test",
        SnapshotAccess::ReadWrite,
        &lexicon(),
    )
    .expect("learning runtime must open");

    assert!(matches!(
        learning.lookup_token("went"),
        TokenLookupDto::Found { .. }
    ));
    assert!(matches!(
        learning.lookup_token("unlisted"),
        TokenLookupDto::Unknown { .. }
    ));
}

#[test]
fn explicit_capture_reports_created_then_already_captured() {
    let mut learning = PlayerLearning::open(
        MemoryBackend::default(),
        "ensub.test",
        SnapshotAccess::ReadWrite,
        &lexicon(),
    )
    .expect("learning runtime must open");
    let input = CapturePodcastInput {
        draft: draft(),
        selected_lemma: None,
        captured_at_ms: 1_776_499_200_000,
    };

    let first = learning
        .capture_podcast(input.clone())
        .expect("first capture must save");
    let mut retry_input = input;
    retry_input.captured_at_ms += 60_000;
    retry_input.draft.playback_position_ms = 1_700;
    let retry = learning
        .capture_podcast(retry_input)
        .expect("retry must be idempotent");

    assert_eq!(first.status, CapturePodcastStatus::CreatedCard);
    assert_eq!(retry.status, CapturePodcastStatus::AlreadyCaptured);
    assert_eq!(first.word_id, retry.word_id);
    assert_eq!(first.context_id, retry.context_id);
}

#[test]
fn due_prompt_hashes_the_canonical_state_and_omits_answer_fields() {
    let next_review_at_ms = 1_735_689_600_123_i64;
    let snapshot = serde_json::json!({
        "format": "ensub-browser-storage",
        "schemaVersion": 2,
        "revision": 1,
        "words": {
            "word-1": {
                "id": "word-1", "term": "Went", "lemma": "go", "phonetic": "go",
                "definition": "verb: move", "createdAt": next_review_at_ms
            }
        },
        "contexts": {
            "context-1": {
                "id": "context-1", "wordId": "word-1", "sentence": "We went home.",
                "source": "podcast:episode-1", "capturedAt": next_review_at_ms
            }
        },
        "reviewStates": {
            "word-1": {
                "wordId": "word-1", "easeFactor": 2.5, "repetitions": 2,
                "intervalDays": 6, "nextReviewAt": next_review_at_ms, "lastRating": 5
            }
        },
        "podcastContexts": {}
    })
    .to_string();
    let learning = PlayerLearning::open(
        MemoryBackend::with_snapshot("ensub.test", snapshot),
        "ensub.test",
        SnapshotAccess::ReadWrite,
        &lexicon(),
    )
    .expect("learning runtime must open");

    let count = learning
        .due_count(DueCountInputDto {
            as_of_ms: next_review_at_ms,
        })
        .expect("due count must load");
    let card = learning
        .due_reviews(DueReviewsInputDto {
            as_of_ms: next_review_at_ms,
            limit: 50,
        })
        .expect("due reviews must load")
        .cards
        .remove(0);
    let serialized = serde_json::to_value(&card).expect("prompt DTO must serialize");

    assert_eq!(count.due_count, 1);
    assert_eq!(
        card.review_token,
        "rs1.ae7f57222652c38a05592f9bd547effd83220431fcba497fa4c714a69daffdd5"
    );
    assert_eq!(card.default_context_id.as_deref(), Some("context-1"));
    assert_eq!(card.contexts[0].sentence, "We went home.");
    for hidden in ["term", "lemma", "phonetic", "definition"] {
        assert!(
            serialized.get(hidden).is_none(),
            "{hidden} must stay hidden"
        );
    }
}

#[test]
fn reveal_then_rating_uses_rust_validation_and_rejects_stale_state() {
    let backend = MemoryBackend::default();
    let mut first = PlayerLearning::open(
        backend.clone(),
        "ensub.test",
        SnapshotAccess::ReadWrite,
        &lexicon(),
    )
    .expect("first learning runtime must open");
    let mut second =
        PlayerLearning::open(backend, "ensub.test", SnapshotAccess::ReadWrite, &lexicon())
            .expect("second learning runtime must open");
    let captured_at_ms = 1_776_499_200_000;
    first
        .capture_podcast(CapturePodcastInput {
            draft: draft(),
            selected_lemma: None,
            captured_at_ms,
        })
        .expect("capture must save");
    let card = first
        .due_reviews(DueReviewsInputDto {
            as_of_ms: captured_at_ms,
            limit: 50,
        })
        .expect("due card must load")
        .cards
        .remove(0);
    let answer = first
        .reveal_review(RevealReviewInputDto {
            word_id: card.word_id.clone(),
            review_token: card.review_token.clone(),
        })
        .expect("current card must reveal");

    assert_eq!(answer.lemma, "go");
    assert_eq!(answer.definition, "verb: move");
    assert!(matches!(
        first.review(RateReviewInputDto {
            word_id: card.word_id.clone(),
            review_token: card.review_token.clone(),
            rating: 6,
            reviewed_at_ms: captured_at_ms,
        }),
        Err(PlayerLearningError::InvalidRating(6))
    ));

    let transition = second
        .review(RateReviewInputDto {
            word_id: card.word_id.clone(),
            review_token: card.review_token.clone(),
            rating: 4,
            reviewed_at_ms: captured_at_ms,
        })
        .expect("current review must commit");
    assert_eq!(transition.interval_days, 1);
    assert_eq!(transition.repetitions, 1);
    assert!(matches!(
        first.reveal_review(RevealReviewInputDto {
            word_id: card.word_id.clone(),
            review_token: card.review_token.clone(),
        }),
        Err(PlayerLearningError::ReviewConflict)
    ));
    assert!(matches!(
        first.review(RateReviewInputDto {
            word_id: card.word_id,
            review_token: card.review_token,
            rating: 4,
            reviewed_at_ms: captured_at_ms,
        }),
        Err(PlayerLearningError::ReviewConflict)
    ));
}

#[test]
fn podcast_review_prompt_exposes_only_saved_context_and_audio_descriptor() {
    let captured_at_ms = 1_776_499_200_000;
    let mut learning = PlayerLearning::open(
        MemoryBackend::default(),
        "ensub.test",
        SnapshotAccess::ReadWrite,
        &lexicon(),
    )
    .expect("learning runtime must open");
    learning
        .capture_podcast(CapturePodcastInput {
            draft: draft(),
            selected_lemma: None,
            captured_at_ms,
        })
        .expect("capture must save");

    let card = learning
        .due_reviews(DueReviewsInputDto {
            as_of_ms: captured_at_ms,
            limit: 50,
        })
        .expect("due card must load")
        .cards
        .remove(0);
    let context = &card.contexts[0];
    let audio = context
        .audio_slice
        .as_ref()
        .expect("podcast context must expose its audio slice");

    assert_eq!(context.episode_title.as_deref(), Some("Episode"));
    assert_eq!(context.captured_at_ms, captured_at_ms);
    assert_eq!(audio.audio_source_url, "https://example.test/audio.mp3");
    assert_eq!(audio.slice_start_ms, 500);
    assert_eq!(audio.slice_end_ms, 3_500);
}

#[test]
fn player_review_rejects_invalid_queue_limits() {
    let learning = PlayerLearning::open(
        MemoryBackend::default(),
        "ensub.test",
        SnapshotAccess::ReadWrite,
        &lexicon(),
    )
    .expect("learning runtime must open");

    for limit in [0, 51] {
        assert!(matches!(
            learning.due_reviews(DueReviewsInputDto {
                as_of_ms: 1_776_499_200_000,
                limit,
            }),
            Err(PlayerLearningError::InvalidReviewLimit(value)) if value == limit
        ));
    }
}

#[test]
fn storage_cas_rejects_a_change_after_token_validation() {
    let backend = MemoryBackend::default();
    let control = backend.clone();
    let mut learning =
        PlayerLearning::open(backend, "ensub.test", SnapshotAccess::ReadWrite, &lexicon())
            .expect("learning runtime must open");
    let reviewed_at_ms = 1_776_499_200_000;
    learning
        .capture_podcast(CapturePodcastInput {
            draft: draft(),
            selected_lemma: None,
            captured_at_ms: reviewed_at_ms,
        })
        .expect("capture must save");
    let card = learning
        .due_reviews(DueReviewsInputDto {
            as_of_ms: reviewed_at_ms,
            limit: 50,
        })
        .expect("due card must load")
        .cards
        .remove(0);
    let mut concurrent: serde_json::Value =
        serde_json::from_str(&control.snapshot("ensub.test")).expect("snapshot must decode");
    concurrent["reviewStates"][&card.word_id]["repetitions"] = serde_json::json!(9);
    control.replace_on_second_load("ensub.test", concurrent.to_string());

    let result = learning.review(RateReviewInputDto {
        word_id: card.word_id.clone(),
        review_token: card.review_token,
        rating: 4,
        reviewed_at_ms,
    });
    let stored: serde_json::Value =
        serde_json::from_str(&control.snapshot("ensub.test")).expect("snapshot must decode");

    assert!(matches!(result, Err(PlayerLearningError::ReviewConflict)));
    assert_eq!(stored["reviewStates"][&card.word_id]["repetitions"], 9);
    assert_eq!(
        stored["reviewStates"][&card.word_id]["lastRating"],
        serde_json::Value::Null
    );
}

#[test]
fn wasm_disambiguation_preparation_whitelists_minimal_context() {
    let learning = PlayerLearning::open(
        MemoryBackend::default(),
        "ensub.test",
        SnapshotAccess::ReadWrite,
        &lexicon(),
    )
    .expect("learning runtime must open");

    let prepared = learning
        .prepare_disambiguation(PrepareDisambiguationInputDto { draft: draft() })
        .expect("disambiguation request must prepare");
    let request = serde_json::to_value(&prepared.request).expect("request must serialize");

    assert_eq!(request["selectedWord"], "went");
    assert_eq!(request["savedSentence"], "We went home.");
    assert_eq!(request["episodeLabel"], "Episode");
    assert_eq!(request["candidateSenses"][0]["senseId"], "sense-0-0");
    let serialized = request.to_string();
    for forbidden in ["feed.xml", "transcript.vtt", "audio.mp3", "episode-1"] {
        assert!(!serialized.contains(forbidden));
    }
}

#[test]
fn wasm_disambiguation_validation_rejects_unsubmitted_sense_ids() {
    let learning = PlayerLearning::open(
        MemoryBackend::default(),
        "ensub.test",
        SnapshotAccess::ReadWrite,
        &lexicon(),
    )
    .expect("learning runtime must open");
    let prepared = learning
        .prepare_disambiguation(PrepareDisambiguationInputDto { draft: draft() })
        .expect("disambiguation request must prepare");

    let valid = learning
        .validate_disambiguation_response(ValidateDisambiguationResponseInputDto {
            request: prepared.request.clone(),
            response_json: r#"{"matchedSenseId":"sense-0-0","explanation":"Physical movement.","confidence":"high"}"#.to_string(),
        })
        .expect("submitted sense must validate");
    let invalid =
        learning.validate_disambiguation_response(ValidateDisambiguationResponseInputDto {
            request: prepared.request,
            response_json:
                r#"{"matchedSenseId":"sense-9-9","explanation":"Invented.","confidence":"high"}"#
                    .to_string(),
        });

    assert_eq!(valid.matched_sense_id.as_deref(), Some("sense-0-0"));
    assert!(matches!(
        invalid,
        Err(PlayerLearningError::Disambiguation(_))
    ));
}
