use core_engine::{PodcastContextQuality, TranscriptFormat, TranscriptResource};
use language_engine::{parse_transcript, reconstruct_transcript_context, TranscriptContextError};

fn document(cues: &[&str]) -> core_engine::TranscriptDocument {
    let mut source = String::from("WEBVTT\n");
    for (index, text) in cues.iter().enumerate() {
        let start = index * 2;
        let end = start + 2;
        source.push_str(&format!(
            "\ncue-{index}\n00:{start:02}.000 --> 00:{end:02}.000\n{text}\n"
        ));
    }
    parse_transcript(
        TranscriptResource {
            url: "https://media.example.test/captions.vtt".to_string(),
            mime_type: "text/vtt".to_string(),
            format: Some(TranscriptFormat::WebVtt),
            language: Some("en".to_string()),
            relation: Some("captions".to_string()),
        },
        &source,
    )
    .expect("context fixture must parse")
}

#[test]
fn reconstructs_a_complete_sentence_across_cues() {
    let document = document(&[
        "Earlier sentence.",
        "The transcript keeps the words",
        "close without interrupting the story.",
        "Later sentence.",
    ]);

    let context =
        reconstruct_transcript_context(&document, "cue-1", 4).expect("selected word must resolve");

    assert_eq!(
        context.sentence,
        "The transcript keeps the words close without interrupting the story."
    );
    assert_eq!(context.quality, PodcastContextQuality::CompleteSentence);
    assert_eq!(context.selected_cue_id, "cue-1");
    assert_eq!(context.selected_token.surface(), "words");
    assert_eq!(context.cue_range.first_cue_id(), "cue-1");
    assert_eq!(context.cue_range.last_cue_id(), "cue-2");
    assert_eq!(context.cue_range.start_ms(), 2_000);
    assert_eq!(context.cue_range.end_ms(), 6_000);
}

#[test]
fn missing_punctuation_saves_the_bounded_fallback_window() {
    let document = document(&[
        "zero",
        "one",
        "two",
        "selected word",
        "four",
        "five",
        "six",
        "excluded",
    ]);

    let context = reconstruct_transcript_context(&document, "cue-3", 1)
        .expect("fallback context must resolve");

    assert_eq!(context.quality, PodcastContextQuality::FallbackCueWindow);
    assert_eq!(context.sentence, "zero one two selected word four five six");
    assert_eq!(context.cue_range.first_cue_id(), "cue-0");
    assert_eq!(context.cue_range.last_cue_id(), "cue-6");
}

#[test]
fn a_sentence_boundary_beyond_three_adjacent_cues_is_not_used() {
    let document = document(&["selected word", "one", "two", "three", "ends here."]);

    let context = reconstruct_transcript_context(&document, "cue-0", 0)
        .expect("bounded fallback must resolve");

    assert_eq!(context.quality, PodcastContextQuality::FallbackCueWindow);
    assert_eq!(context.sentence, "selected word one two three");
    assert_eq!(context.cue_range.last_cue_id(), "cue-3");
}

#[test]
fn a_sentence_start_more_than_sixty_words_away_forces_fallback() {
    let prefix = std::iter::repeat_n("word", 61)
        .collect::<Vec<_>>()
        .join(" ");
    let document = document(&[&format!("{prefix} selected.")]);

    let context = reconstruct_transcript_context(&document, "cue-0", 61)
        .expect("word-limited fallback must resolve");

    assert_eq!(context.quality, PodcastContextQuality::FallbackCueWindow);
    assert!(!context.sentence.starts_with("word0 "));
    assert_eq!(context.sentence.split_whitespace().count(), 61);
    assert!(context.sentence.ends_with("selected."));
}

#[test]
fn invalid_selection_returns_typed_errors() {
    let document = document(&["Known word."]);

    assert_eq!(
        reconstruct_transcript_context(&document, "missing", 0),
        Err(TranscriptContextError::CueNotFound {
            cue_id: "missing".to_string(),
        })
    );
    assert_eq!(
        reconstruct_transcript_context(&document, "cue-0", 8),
        Err(TranscriptContextError::TokenNotFound {
            cue_id: "cue-0".to_string(),
            token_index: 8,
        })
    );
}
