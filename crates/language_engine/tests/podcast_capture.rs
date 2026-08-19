use chrono::{TimeZone, Utc};
use core_engine::{
    calculate_padded_audio_slice, CueRange, PodcastContextDraft, PodcastContextQuality,
    PodcastEpisodeProvenance, PodcastFeedProvenance, TranscriptFormat, TranscriptProvenance,
    TranscriptToken,
};
use language_engine::{podcast_capture_from_entry, Definition, LexiconEntry};

fn draft(cue_id: &str, start_byte: u32) -> PodcastContextDraft {
    let cue_range = CueRange::try_new(cue_id.to_string(), cue_id.to_string(), 1_000, 2_000)
        .expect("fixture cue range must be valid");
    PodcastContextDraft {
        sentence: "We went home.".to_string(),
        quality: PodcastContextQuality::CompleteSentence,
        feed: PodcastFeedProvenance {
            source_url: "https://example.test/feed.xml".to_string(),
            title: "Test Feed".to_string(),
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
        selected_cue_id: cue_id.to_string(),
        selected_token: TranscriptToken::try_new("went".to_string(), start_byte, start_byte + 4)
            .expect("fixture token must be valid"),
        cue_range: cue_range.clone(),
        playback_position_ms: 1_400,
        audio_slice: calculate_padded_audio_slice(
            "https://example.test/audio.mp3".to_string(),
            &cue_range,
            None,
        )
        .expect("fixture slice must be valid"),
    }
}

fn entry() -> LexiconEntry {
    LexiconEntry {
        lemma: "go".to_string(),
        phonetic: "goʊ".to_string(),
        definitions: vec![Definition {
            part_of_speech: "verb".to_string(),
            text: "move from one place to another".to_string(),
        }],
    }
}

#[test]
fn retries_keep_the_same_learning_and_encounter_ids() {
    let captured_at = Utc
        .with_ymd_and_hms(2026, 8, 18, 12, 0, 0)
        .single()
        .expect("fixture timestamp must be valid");
    let first = podcast_capture_from_entry(draft("cue-1", 3), entry(), captured_at)
        .expect("capture must build");
    let retry = podcast_capture_from_entry(draft("cue-1", 3), entry(), captured_at)
        .expect("retry must build");

    assert_eq!(first.capture.word.id, retry.capture.word.id);
    assert_eq!(
        first.podcast_context.context_id,
        retry.podcast_context.context_id
    );
}

#[test]
fn another_token_encounter_reuses_the_word_but_gets_a_new_context() {
    let captured_at = Utc
        .with_ymd_and_hms(2026, 8, 18, 12, 0, 0)
        .single()
        .expect("fixture timestamp must be valid");
    let first = podcast_capture_from_entry(draft("cue-1", 3), entry(), captured_at)
        .expect("capture must build");
    let another = podcast_capture_from_entry(draft("cue-2", 8), entry(), captured_at)
        .expect("capture must build");

    assert_eq!(first.capture.word.id, another.capture.word.id);
    assert_ne!(
        first.podcast_context.context_id,
        another.podcast_context.context_id
    );
}
