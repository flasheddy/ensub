use chrono::{TimeZone, Utc};
use core_engine::{
    calculate_padded_audio_slice, reconcile_episode_identity, AudioSlice, CueRange,
    EpisodeIdentity, MediaDomainError, PodcastContext, PodcastContextQuality, PodcastEpisode,
    PodcastEpisodeProvenance, PodcastFeed, PodcastFeedProvenance, TranscriptCue,
    TranscriptDocument, TranscriptFormat, TranscriptProvenance, TranscriptResource,
    TranscriptToken, AUDIO_SLICE_PADDING_MS,
};
use serde_json::json;

fn transcript_resource(format: Option<TranscriptFormat>) -> TranscriptResource {
    TranscriptResource {
        url: "https://media.example.test/transcript.vtt".to_string(),
        mime_type: "text/vtt".to_string(),
        format,
        language: Some("en".to_string()),
        relation: Some("captions".to_string()),
    }
}

#[test]
fn episode_identity_reconciliation_preserves_internal_id_and_merges_aliases() {
    let existing = EpisodeIdentity {
        internal_id: "stable-id".to_string(),
        feed_url: "https://media.example.test/feed.xml".to_string(),
        publisher_guid_aliases: vec!["guid-b".to_string(), "guid-a".to_string()],
        enclosure_url_aliases: vec!["https://media.example.test/old.mp3".to_string()],
    };
    let observed = EpisodeIdentity {
        internal_id: "new-parser-id".to_string(),
        feed_url: existing.feed_url.clone(),
        publisher_guid_aliases: vec!["guid-a".to_string(), "guid-c".to_string()],
        enclosure_url_aliases: vec!["https://media.example.test/new.mp3".to_string()],
    };

    let reconciled = reconcile_episode_identity(&existing, &observed)
        .expect("intersecting aliases must identify the same episode");

    assert_eq!(reconciled.internal_id, "stable-id");
    assert_eq!(reconciled.feed_url, existing.feed_url);
    assert_eq!(
        reconciled.publisher_guid_aliases,
        ["guid-a", "guid-b", "guid-c"]
    );
    assert_eq!(
        reconciled.enclosure_url_aliases,
        [
            "https://media.example.test/new.mp3",
            "https://media.example.test/old.mp3"
        ]
    );
}

#[test]
fn episode_identity_reconciliation_requires_feed_and_identity_evidence() {
    let existing = EpisodeIdentity {
        internal_id: "stable-id".to_string(),
        feed_url: "https://media.example.test/feed.xml".to_string(),
        publisher_guid_aliases: vec!["guid-1".to_string()],
        enclosure_url_aliases: vec!["https://media.example.test/episode.mp3".to_string()],
    };
    let matching_id = EpisodeIdentity {
        internal_id: existing.internal_id.clone(),
        feed_url: existing.feed_url.clone(),
        publisher_guid_aliases: Vec::new(),
        enclosure_url_aliases: Vec::new(),
    };
    assert!(reconcile_episode_identity(&existing, &matching_id).is_some());

    let mut unrelated = matching_id.clone();
    unrelated.internal_id = "different-id".to_string();
    assert_eq!(reconcile_episode_identity(&existing, &unrelated), None);

    let mut other_feed = matching_id;
    other_feed.feed_url = "https://other.example.test/feed.xml".to_string();
    assert_eq!(reconcile_episode_identity(&existing, &other_feed), None);
}

fn cue_with(
    id: &str,
    source_order: u32,
    start_ms: u64,
    end_ms: u64,
    text: &str,
    tokens: Vec<TranscriptToken>,
) -> Result<TranscriptCue, MediaDomainError> {
    TranscriptCue::try_new(
        id.to_string(),
        source_order,
        start_ms,
        end_ms,
        text.to_string(),
        tokens,
        TranscriptFormat::WebVtt,
        None,
    )
}

fn document_with(cues: Vec<TranscriptCue>) -> Result<TranscriptDocument, MediaDomainError> {
    TranscriptDocument::try_new(transcript_resource(Some(TranscriptFormat::WebVtt)), cues)
}

fn cue(id: &str, source_order: u32, start_ms: u64, end_ms: u64) -> TranscriptCue {
    cue_with(id, source_order, start_ms, end_ms, id, Vec::new()).expect("test cue must be valid")
}

#[test]
fn podcast_metadata_records_are_owned_and_serializable() {
    let feed = PodcastFeed {
        source_url: "https://media.example.test/feed.xml".to_string(),
        title: "Synthetic English".to_string(),
        language: Some("en".to_string()),
        description: Some("Synthetic test feed".to_string()),
        artwork_url: Some("https://media.example.test/feed.png".to_string()),
    };
    let episode = PodcastEpisode {
        identity: EpisodeIdentity {
            internal_id: "episode-1".to_string(),
            feed_url: feed.source_url.clone(),
            publisher_guid_aliases: vec!["publisher-episode-1".to_string()],
            enclosure_url_aliases: vec!["https://media.example.test/episode.mp3".to_string()],
        },
        publisher_guid: Some("publisher-episode-1".to_string()),
        title: "A Synthetic Episode".to_string(),
        published_at: Some(
            Utc.with_ymd_and_hms(2026, 8, 17, 8, 0, 0)
                .single()
                .expect("test timestamp must be valid"),
        ),
        language: Some("en".to_string()),
        enclosure_url: "https://media.example.test/episode.mp3".to_string(),
        enclosure_mime_type: "audio/mpeg".to_string(),
        duration_ms: Some(60_000),
        artwork_url: Some("https://media.example.test/episode.png".to_string()),
        transcript_resources: vec![transcript_resource(Some(TranscriptFormat::WebVtt))],
    };

    let encoded = serde_json::to_string(&(feed.clone(), episode.clone()))
        .expect("media metadata must serialize");
    let decoded: (PodcastFeed, PodcastEpisode) =
        serde_json::from_str(&encoded).expect("media metadata must deserialize");

    assert_eq!(decoded, (feed, episode));
}

#[test]
fn cue_time_bounds_are_validated_before_empty_text_handling() {
    let cases = [
        (0, 1, true),
        (u64::MAX - 1, u64::MAX, true),
        (5, 5, false),
        (6, 5, false),
    ];

    for (start_ms, end_ms, is_valid) in cases {
        let result = cue_with("cue-0", 0, start_ms, end_ms, "", Vec::new());
        if is_valid {
            let cue = result.expect("strictly increasing cue bounds must be valid");
            assert_eq!((cue.start_ms(), cue.end_ms()), (start_ms, end_ms));
        } else {
            assert_eq!(
                result,
                Err(MediaDomainError::InvalidCueBounds {
                    cue_id: "cue-0".to_string(),
                    start_ms,
                    end_ms,
                })
            );
        }
    }
}

#[test]
fn token_and_cue_text_spans_preserve_utf8_boundaries_and_surface_text() {
    let token = TranscriptToken::try_new("café".to_string(), 0, 5)
        .expect("valid token bounds must construct");
    let cue = cue_with("cue-0", 0, 100, 200, "café", vec![token.clone()])
        .expect("matching UTF-8 token must construct");

    assert_eq!(cue.tokens(), &[token]);
    assert_eq!(cue.tokens()[0].surface(), "café");
    assert_eq!(
        (cue.tokens()[0].start_byte(), cue.tokens()[0].end_byte()),
        (0, 5)
    );

    assert_eq!(
        TranscriptToken::try_new("x".to_string(), 2, 2),
        Err(MediaDomainError::InvalidTokenBounds {
            start_byte: 2,
            end_byte: 2,
        })
    );

    let split_code_point = TranscriptToken::try_new("x".to_string(), 3, 4)
        .expect("standalone token bounds are structurally valid");
    assert_eq!(
        cue_with("cue-0", 0, 100, 200, "café", vec![split_code_point]),
        Err(MediaDomainError::TokenSpanNotOnCharBoundary {
            cue_id: "cue-0".to_string(),
            offset: 4,
        })
    );

    let out_of_bounds = TranscriptToken::try_new("x".to_string(), 0, 6)
        .expect("standalone token bounds are structurally valid");
    assert_eq!(
        cue_with("cue-0", 0, 100, 200, "café", vec![out_of_bounds]),
        Err(MediaDomainError::TokenSpanOutOfBounds {
            cue_id: "cue-0".to_string(),
            start_byte: 0,
            end_byte: 6,
            text_len_bytes: 5,
        })
    );

    let mismatched = TranscriptToken::try_new("tea".to_string(), 0, 4)
        .expect("standalone token bounds are structurally valid");
    assert_eq!(
        cue_with("cue-0", 0, 100, 200, "cafe", vec![mismatched]),
        Err(MediaDomainError::TokenSurfaceMismatch {
            cue_id: "cue-0".to_string(),
            start_byte: 0,
            end_byte: 4,
        })
    );
}

#[test]
fn transcript_document_rejects_unsupported_mismatched_or_unordered_cues() {
    assert_eq!(
        TranscriptDocument::try_new(transcript_resource(None), Vec::new()),
        Err(MediaDomainError::UnsupportedTranscriptResource)
    );

    let srt_cue = TranscriptCue::try_new(
        "cue-0".to_string(),
        0,
        0,
        1,
        "cue".to_string(),
        Vec::new(),
        TranscriptFormat::Srt,
        None,
    )
    .expect("test cue must be valid");
    assert_eq!(
        document_with(vec![srt_cue]),
        Err(MediaDomainError::CueFormatMismatch {
            cue_id: "cue-0".to_string(),
            expected: TranscriptFormat::WebVtt,
            actual: TranscriptFormat::Srt,
        })
    );

    let cases = [
        (
            vec![cue("cue-0", 1, 0, 10)],
            MediaDomainError::InvalidCueSourceOrder {
                cue_id: "cue-0".to_string(),
                expected: 0,
                actual: 1,
            },
        ),
        (
            vec![cue("same", 0, 0, 10), cue("same", 1, 10, 20)],
            MediaDomainError::DuplicateCueId {
                cue_id: "same".to_string(),
            },
        ),
        (
            vec![cue("cue-0", 0, 10, 20), cue("cue-1", 1, 5, 15)],
            MediaDomainError::CueStartOutOfOrder {
                previous_cue_id: "cue-0".to_string(),
                previous_start_ms: 10,
                cue_id: "cue-1".to_string(),
                start_ms: 5,
            },
        ),
    ];

    for (cues, expected) in cases {
        assert_eq!(document_with(cues), Err(expected));
    }

    document_with(vec![cue("cue-0", 0, 10, 30), cue("cue-1", 1, 10, 20)])
        .expect("equal starts and non-monotonic ends must remain valid");
}

#[test]
fn active_cues_obey_half_open_boundaries_gaps_and_source_order() {
    let basic = document_with(vec![
        cue("cue-0", 0, 1_000, 2_000),
        cue("cue-1", 1, 2_000, 3_000),
        cue("cue-2", 2, 3_500, 4_000),
    ])
    .expect("test document must be valid");
    let basic_cases = [
        (999, vec![]),
        (1_000, vec![0]),
        (1_999, vec![0]),
        (2_000, vec![1]),
        (3_000, vec![]),
        (3_499, vec![]),
        (3_500, vec![2]),
        (4_000, vec![]),
    ];
    for (position_ms, expected) in basic_cases {
        assert_eq!(basic.active_cue_indices(position_ms), expected);
    }

    let overlaps = document_with(vec![
        cue("cue-0", 0, 1_000, 3_000),
        cue("cue-1", 1, 1_500, 2_500),
        cue("cue-2", 2, 2_000, 3_500),
    ])
    .expect("overlapping document must be valid");
    let overlap_cases = [
        (1_999, vec![0, 1]),
        (2_000, vec![0, 1, 2]),
        (2_500, vec![0, 2]),
        (3_000, vec![2]),
    ];
    for (position_ms, expected) in overlap_cases {
        assert_eq!(overlaps.active_cue_indices(position_ms), expected);
    }

    let nested = document_with(vec![
        cue("cue-0", 0, 0, 10_000),
        cue("cue-1", 1, 1_000, 2_000),
        cue("cue-2", 2, 3_000, 4_000),
    ])
    .expect("nested document must be valid");
    assert_eq!(nested.active_cue_indices(2_500), vec![0]);
    assert_eq!(nested.active_cue_indices(3_500), vec![0, 2]);

    let maximum = document_with(vec![cue("cue-0", 0, u64::MAX - 1, u64::MAX)])
        .expect("maximum timestamp document must be valid");
    assert_eq!(maximum.active_cue_indices(u64::MAX - 1), vec![0]);
    assert!(maximum.active_cue_indices(u64::MAX).is_empty());

    let empty = document_with(Vec::new()).expect("empty supported document must be valid");
    assert!(empty.active_cue_indices(0).is_empty());
}

#[test]
fn indexed_lookup_matches_the_naive_active_predicate_at_every_position() {
    let document = document_with(vec![
        cue("cue-0", 0, 0, 20),
        cue("cue-1", 1, 2, 4),
        cue("cue-2", 2, 4, 8),
        cue("cue-3", 3, 10, 12),
        cue("cue-4", 4, 10, 18),
    ])
    .expect("test document must be valid");

    for position_ms in 0..=21 {
        let expected: Vec<u32> = document
            .cues()
            .iter()
            .filter(|cue| cue.start_ms() <= position_ms && position_ms < cue.end_ms())
            .map(TranscriptCue::source_order)
            .collect();
        assert_eq!(document.active_cue_indices(position_ms), expected);
    }
}

#[test]
fn transcript_document_serde_rebuilds_the_active_index() {
    let document = document_with(vec![
        cue("cue-0", 0, 0, 10_000),
        cue("cue-1", 1, 1_000, 2_000),
        cue("cue-2", 2, 3_000, 4_000),
    ])
    .expect("test document must be valid");

    let encoded = serde_json::to_string(&document).expect("document must serialize");
    assert!(!encoded.contains("active_index"));
    let decoded: TranscriptDocument =
        serde_json::from_str(&encoded).expect("document must deserialize through validation");

    assert_eq!(decoded, document);
    assert_eq!(decoded.active_cue_indices(3_500), vec![0, 2]);
}

#[test]
fn cue_range_uses_source_endpoints_and_earliest_latest_time_bounds() {
    let cues = vec![
        cue("cue-0", 0, 1_000, 1_200),
        cue("cue-1", 1, 1_100, 5_000),
        cue("cue-2", 2, 3_000, 3_500),
    ];

    let range = CueRange::try_from_cues(&cues).expect("non-empty cue range must construct");

    assert_eq!(range.first_cue_id(), "cue-0");
    assert_eq!(range.last_cue_id(), "cue-2");
    assert_eq!((range.start_ms(), range.end_ms()), (1_000, 5_000));
    assert_eq!(
        CueRange::try_from_cues(&[]),
        Err(MediaDomainError::EmptyCueRange)
    );
    assert_eq!(
        CueRange::try_new("cue-0".to_string(), "cue-0".to_string(), 5, 5),
        Err(MediaDomainError::InvalidCueRangeBounds {
            start_ms: 5,
            end_ms: 5,
        })
    );
}

#[test]
fn padded_audio_slice_math_clamps_zero_and_known_duration() {
    let cases = [
        (1_000, 2_000, None, 500, 2_500),
        (200, 1_000, None, 0, 1_500),
        (500, 1_000, None, 0, 1_500),
        (1_000, 2_000, Some(10_000), 500, 2_500),
        (1_000, 2_000, Some(2_500), 500, 2_500),
        (1_000, 2_000, Some(2_200), 500, 2_200),
    ];

    assert_eq!(AUDIO_SLICE_PADDING_MS, 500);
    for (start_ms, end_ms, duration_ms, expected_start, expected_end) in cases {
        let range = CueRange::try_new("cue-0".to_string(), "cue-0".to_string(), start_ms, end_ms)
            .expect("test range must be valid");
        let slice = calculate_padded_audio_slice(
            "https://media.example.test/episode.mp3".to_string(),
            &range,
            duration_ms,
        )
        .expect("valid padded slice must construct");

        assert_eq!(
            (slice.start_ms(), slice.end_ms()),
            (expected_start, expected_end)
        );
        assert_eq!(
            slice.audio_source_url(),
            "https://media.example.test/episode.mp3"
        );
    }
}

#[test]
fn padded_audio_slice_reports_overflow_and_collapsed_duration_ranges() {
    let reaches_maximum = CueRange::try_new(
        "cue-0".to_string(),
        "cue-0".to_string(),
        u64::MAX - 1_000,
        u64::MAX - AUDIO_SLICE_PADDING_MS,
    )
    .expect("test range must be valid");
    let maximum_slice = calculate_padded_audio_slice(
        "https://media.example.test/episode.mp3".to_string(),
        &reaches_maximum,
        None,
    )
    .expect("padding exactly to u64::MAX must remain valid");
    assert_eq!(maximum_slice.end_ms(), u64::MAX);

    let overflowing = CueRange::try_new(
        "cue-0".to_string(),
        "cue-0".to_string(),
        u64::MAX - 1_000,
        u64::MAX - AUDIO_SLICE_PADDING_MS + 1,
    )
    .expect("test range must be valid");
    for duration_ms in [None, Some(u64::MAX)] {
        assert_eq!(
            calculate_padded_audio_slice(
                "https://media.example.test/episode.mp3".to_string(),
                &overflowing,
                duration_ms,
            ),
            Err(MediaDomainError::PaddedEndOverflow {
                end_ms: u64::MAX - AUDIO_SLICE_PADDING_MS + 1,
                padding_ms: AUDIO_SLICE_PADDING_MS,
            })
        );
    }

    let ordinary = CueRange::try_new("cue-0".to_string(), "cue-0".to_string(), 1_000, 2_000)
        .expect("test range must be valid");
    for duration_ms in [500, 499] {
        assert_eq!(
            calculate_padded_audio_slice(
                "https://media.example.test/episode.mp3".to_string(),
                &ordinary,
                Some(duration_ms),
            ),
            Err(MediaDomainError::InvalidAudioSliceBounds {
                start_ms: 500,
                end_ms: duration_ms,
                duration_ms: Some(duration_ms),
            })
        );
    }
}

#[test]
fn validated_media_ranges_reject_invalid_deserialized_payloads() {
    assert!(serde_json::from_value::<TranscriptToken>(json!({
        "surface": "x", "start_byte": 1, "end_byte": 1
    }))
    .is_err());
    assert!(serde_json::from_value::<TranscriptCue>(json!({
        "id": "cue-0", "source_order": 0, "start_ms": -1, "end_ms": 10,
        "text": "", "tokens": [], "source_format": "web_vtt", "source_cue_id": null
    }))
    .is_err());
    assert!(serde_json::from_value::<CueRange>(json!({
        "first_cue_id": "cue-0", "last_cue_id": "cue-0", "start_ms": 10, "end_ms": 10
    }))
    .is_err());
    assert!(serde_json::from_value::<AudioSlice>(json!({
        "audio_source_url": "https://media.example.test/episode.mp3",
        "start_ms": 10, "end_ms": 10
    }))
    .is_err());
}

#[test]
fn podcast_context_round_trip_preserves_lean_structured_provenance() {
    let captured_at = Utc
        .with_ymd_and_hms(2026, 8, 17, 9, 30, 0)
        .single()
        .expect("test timestamp must be valid");
    let cue = cue("cue-0", 0, 1_000, 2_000);
    let cue_range =
        CueRange::try_from_cues(std::slice::from_ref(&cue)).expect("test cue range must be valid");
    let audio_slice = calculate_padded_audio_slice(
        "https://media.example.test/episode.mp3".to_string(),
        &cue_range,
        Some(60_000),
    )
    .expect("test audio slice must be valid");
    let context = PodcastContext {
        sentence: "A synthetic sentence.".to_string(),
        quality: PodcastContextQuality::CompleteSentence,
        feed: PodcastFeedProvenance {
            source_url: "https://media.example.test/feed.xml".to_string(),
            title: "Synthetic English".to_string(),
        },
        episode: PodcastEpisodeProvenance {
            internal_id: "episode-1".to_string(),
            publisher_guid: Some("publisher-episode-1".to_string()),
            title: "A Synthetic Episode".to_string(),
            published_at: Some(captured_at),
            enclosure_url: "https://media.example.test/episode.mp3".to_string(),
        },
        transcript: TranscriptProvenance {
            url: "https://media.example.test/transcript.vtt".to_string(),
            format: TranscriptFormat::WebVtt,
            language: Some("en".to_string()),
        },
        selected_token: TranscriptToken::try_new("synthetic".to_string(), 2, 11)
            .expect("test token must be valid"),
        normalized_lemma: "synthetic".to_string(),
        cue_range,
        playback_position_ms: 1_500,
        audio_slice,
        captured_at,
    };

    let encoded = serde_json::to_string(&context).expect("podcast context must serialize");
    let decoded: PodcastContext =
        serde_json::from_str(&encoded).expect("podcast context must deserialize");

    assert_eq!(decoded, context);
}
