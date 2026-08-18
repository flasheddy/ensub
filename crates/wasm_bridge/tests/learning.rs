use std::collections::BTreeMap;
use std::io;

use core_engine::{
    calculate_padded_audio_slice, CueRange, PodcastContextDraft, PodcastContextQuality,
    PodcastEpisodeProvenance, PodcastFeedProvenance, TranscriptFormat, TranscriptProvenance,
    TranscriptToken,
};
use ensub_wasm::{
    CapturePodcastInput, CapturePodcastStatus, PlayerLearning, SnapshotAccess, SnapshotBackend,
    TokenLookupDto,
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
