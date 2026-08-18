use chrono::{TimeZone, Utc};
use core_engine::{
    calculate_padded_audio_slice, initial_review_state, Capture, ContextId, ContextRecord,
    CueRange, MediaDomainError, PodcastCapture, PodcastContext, PodcastContextDraft,
    PodcastContextQuality, PodcastContextRecord, PodcastEpisodeProvenance, PodcastFeedProvenance,
    TranscriptFormat, TranscriptProvenance, TranscriptToken, WordId, WordRecord,
};

fn captured_at() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 8, 18, 9, 0, 0)
        .single()
        .expect("test timestamp must be valid")
}

fn draft() -> PodcastContextDraft {
    let cue_range = CueRange::try_new("cue-1".to_string(), "cue-2".to_string(), 1_000, 3_000)
        .expect("test cue range must be valid");
    let audio_slice = calculate_padded_audio_slice(
        "https://media.example.test/episode.mp3".to_string(),
        &cue_range,
        Some(10_000),
    )
    .expect("test slice must be valid");
    PodcastContextDraft {
        sentence: "The transcript keeps words close.".to_string(),
        quality: PodcastContextQuality::CompleteSentence,
        feed: PodcastFeedProvenance {
            source_url: "https://media.example.test/feed.xml".to_string(),
            title: "Synthetic Signal".to_string(),
        },
        episode: PodcastEpisodeProvenance {
            internal_id: "episode-1".to_string(),
            publisher_guid: Some("guid-1".to_string()),
            title: "Context in Motion".to_string(),
            published_at: Some(captured_at()),
            enclosure_url: "https://media.example.test/episode.mp3".to_string(),
        },
        transcript: TranscriptProvenance {
            url: "https://media.example.test/transcript.vtt".to_string(),
            format: TranscriptFormat::WebVtt,
            language: Some("en".to_string()),
        },
        selected_cue_id: "cue-1".to_string(),
        selected_token: TranscriptToken::try_new("words".to_string(), 21, 26)
            .expect("test token must be valid"),
        cue_range,
        playback_position_ms: 1_750,
        audio_slice,
    }
}

fn capture(context: &PodcastContext) -> Capture {
    let word_id = WordId::new("word-words");
    Capture {
        word: WordRecord {
            id: word_id.clone(),
            term: "words".to_string(),
            lemma: "word".to_string(),
            phonetic: "wɝd".to_string(),
            definition: "noun: a unit of language".to_string(),
            created_at: captured_at(),
        },
        contexts: vec![ContextRecord {
            id: ContextId::new("context-episode-1-cue-1"),
            word_id: word_id.clone(),
            sentence: context.sentence.clone(),
            source: "podcast:episode-1".to_string(),
            captured_at: captured_at(),
        }],
        initial_review_state: initial_review_state(word_id, captured_at()),
    }
}

#[test]
fn podcast_capture_round_trip_preserves_linked_structured_context() {
    let context = PodcastContext::try_from_draft(draft(), "word".to_string(), captured_at())
        .expect("valid draft must finalize");
    let capture = capture(&context);
    let record = PodcastContextRecord {
        context_id: capture.contexts[0].id.clone(),
        word_id: capture.word.id.clone(),
        context,
    };
    let aggregate = PodcastCapture::try_new(capture, record).expect("links must validate");

    let encoded = serde_json::to_string(&aggregate).expect("aggregate must serialize");
    let decoded: PodcastCapture =
        serde_json::from_str(&encoded).expect("aggregate must deserialize");

    assert_eq!(decoded, aggregate);
    assert_eq!(decoded.podcast_context.context.selected_cue_id, "cue-1");
    assert_eq!(decoded.podcast_context.context.audio_slice.start_ms(), 500);
    assert_eq!(decoded.podcast_context.context.audio_slice.end_ms(), 3_500);
}

#[test]
fn podcast_context_rejects_an_audio_source_other_than_the_episode_enclosure() {
    let mut draft = draft();
    draft.audio_slice = core_engine::AudioSlice::try_new(
        "https://media.example.test/other.mp3".to_string(),
        500,
        3_500,
    )
    .expect("standalone slice must be valid");

    assert_eq!(
        PodcastContext::try_from_draft(draft, "word".to_string(), captured_at()),
        Err(MediaDomainError::PodcastAudioSourceMismatch)
    );
}

#[test]
fn podcast_capture_rejects_mismatched_generic_context_without_partial_semantics() {
    let context = PodcastContext::try_from_draft(draft(), "word".to_string(), captured_at())
        .expect("valid draft must finalize");
    let mut capture = capture(&context);
    capture.contexts[0].sentence = "Different sentence.".to_string();
    let record = PodcastContextRecord {
        context_id: capture.contexts[0].id.clone(),
        word_id: capture.word.id.clone(),
        context,
    };

    assert_eq!(
        PodcastCapture::try_new(capture, record),
        Err(MediaDomainError::PodcastContextSentenceMismatch)
    );
}
