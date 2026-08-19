use core_engine::TranscriptFormat;
use language_engine::{parse_podcast_feed, parse_podcast_fixture, PodcastFixtureError};
use serde_json::{json, Value};

const SOURCE_URL: &str = "https://MEDIA.example.test:443/demo/fixture.json#download";

fn valid_fixture() -> Value {
    json!({
        "schema_version": 1,
        "feed": {
            "title": "Synthetic English",
            "language": "en",
            "description": "A synthetic fixture.",
            "artwork_url": "../images/feed.png"
        },
        "episode": {
            "publisher_guid": "episode-1",
            "title": "A Synthetic Episode",
            "published_at": "2026-08-17T08:00:00Z",
            "language": "en",
            "enclosure_url": "audio/episode-1.mp3",
            "enclosure_mime_type": "Audio/MPEG; charset=binary",
            "duration_ms": 7_000,
            "artwork_url": "/images/episode.png"
        },
        "transcript": {
            "url": "captions/episode-1.vtt#english",
            "mime_type": "Text/VTT; charset=utf-8",
            "format": "web_vtt",
            "language": "en",
            "relation": "captions",
            "cues": [
                {
                    "source_cue_id": "intro",
                    "start_ms": 0,
                    "end_ms": 3_000,
                    "text": "Hello, cafe."
                },
                {
                    "source_cue_id": "overlap",
                    "start_ms": 2_500,
                    "end_ms": 6_000,
                    "text": "Overlap cue."
                }
            ]
        }
    })
}

fn parse(value: &Value) -> Result<language_engine::PodcastFixture, PodcastFixtureError> {
    parse_podcast_fixture(
        SOURCE_URL,
        &serde_json::to_vec(value).expect("test fixture must serialize"),
    )
}

#[test]
fn valid_fixture_resolves_urls_and_builds_portable_domain_values() {
    let fixture = parse(&valid_fixture()).expect("valid fixture must parse");

    assert_eq!(
        fixture.feed.source_url,
        "https://media.example.test/demo/fixture.json"
    );
    assert_eq!(
        fixture.feed.artwork_url.as_deref(),
        Some("https://media.example.test/images/feed.png")
    );
    assert_eq!(fixture.episode.publisher_guid.as_deref(), Some("episode-1"));
    assert_eq!(
        fixture.episode.enclosure_url,
        "https://media.example.test/demo/audio/episode-1.mp3"
    );
    assert_eq!(fixture.episode.enclosure_mime_type, "audio/mpeg");
    assert_eq!(fixture.episode.duration_ms, Some(7_000));
    assert_eq!(
        fixture.episode.artwork_url.as_deref(),
        Some("https://media.example.test/images/episode.png")
    );
    assert_eq!(fixture.episode.transcript_resources.len(), 1);
    assert_eq!(
        fixture.episode.transcript_resources[0].url,
        "https://media.example.test/demo/captions/episode-1.vtt#english"
    );
    assert_eq!(
        fixture.episode.transcript_resources[0].format,
        Some(TranscriptFormat::WebVtt)
    );
    assert_eq!(fixture.transcript.cues().len(), 2);
}

#[test]
fn identity_cue_ids_orders_and_tokens_are_stable_and_match_feed_conventions() {
    let first = parse(&valid_fixture()).expect("fixture must parse");
    let second = parse(&valid_fixture()).expect("fixture must parse deterministically");
    assert_eq!(first, second);

    let rss = br#"<rss version="1.0"><channel><title>Synthetic English</title><item>
      <guid>episode-1</guid><title>A Synthetic Episode</title>
      <enclosure url="audio/episode-1.mp3" type="audio/mpeg" />
    </item></channel></rss>"#;
    let report = parse_podcast_feed(SOURCE_URL, rss).expect("comparison feed must parse");
    assert_eq!(first.episode.identity, report.episodes[0].identity);

    let cues = first.transcript.cues();
    assert_eq!(cues[0].id(), "cue-0");
    assert_eq!(cues[0].source_order(), 0);
    assert_eq!(cues[0].source_cue_id(), Some("intro"));
    assert_eq!(cues[1].id(), "cue-1");
    assert_eq!(cues[1].source_order(), 1);
    assert_eq!(cues[1].source_cue_id(), Some("overlap"));
    assert_eq!(
        cues[0]
            .tokens()
            .iter()
            .map(|token| (token.surface(), token.start_byte(), token.end_byte()))
            .collect::<Vec<_>>(),
        [("Hello", 0, 5), ("cafe", 7, 11)]
    );
}

#[test]
fn unicode_token_spans_use_utf8_byte_offsets() {
    let mut value = valid_fixture();
    value["transcript"]["cues"] = json!([{
        "source_cue_id": "unicode",
        "start_ms": 0,
        "end_ms": 1_000,
        "text": "Crème café 東京"
    }]);

    let fixture = parse(&value).expect("Unicode fixture must parse");
    let cue = &fixture.transcript.cues()[0];
    let tokens = cue
        .tokens()
        .iter()
        .map(|token| (token.surface(), token.start_byte(), token.end_byte()))
        .collect::<Vec<_>>();
    assert_eq!(
        tokens,
        [
            ("Crème", 0, 6),
            ("café", 7, 12),
            ("東", 13, 16),
            ("京", 16, 19),
        ]
    );
    for token in cue.tokens() {
        let start = usize::try_from(token.start_byte()).expect("test offset must fit");
        let end = usize::try_from(token.end_byte()).expect("test offset must fit");
        assert_eq!(cue.text().get(start..end), Some(token.surface()));
    }
}

#[test]
fn malformed_json_unknown_fields_and_unsupported_schema_are_distinct() {
    let malformed = parse_podcast_fixture(SOURCE_URL, br#"{"schema_version":1"#)
        .expect_err("malformed JSON must fail");
    assert!(matches!(
        &malformed,
        PodcastFixtureError::InvalidJson { .. }
    ));
    assert_eq!(malformed.code(), "podcast_fixture_invalid_json");

    let mut unknown = valid_fixture();
    unknown["transcript"]["cues"][0]["unexpected"] = json!(true);
    let unknown = parse(&unknown).expect_err("unknown cue field must fail");
    assert!(matches!(&unknown, PodcastFixtureError::InvalidJson { .. }));

    let mut unsupported = valid_fixture();
    unsupported["schema_version"] = json!(2);
    let unsupported = parse(&unsupported).expect_err("unsupported schema must fail");
    assert!(matches!(
        &unsupported,
        PodcastFixtureError::UnsupportedSchema { .. }
    ));
    assert_eq!(unsupported.code(), "podcast_fixture_unsupported_schema");
}

#[test]
fn empty_or_malformed_metadata_is_rejected_with_a_stable_code() {
    let cases = [
        ("feed title", vec!["feed", "title"], json!("  ")),
        (
            "publication timestamp",
            vec!["episode", "published_at"],
            json!("yesterday"),
        ),
        (
            "audio MIME type",
            vec!["episode", "enclosure_mime_type"],
            json!("text/plain"),
        ),
        ("duration", vec!["episode", "duration_ms"], json!(0)),
        (
            "transcript format",
            vec!["transcript", "format"],
            json!("srt"),
        ),
        (
            "transcript format must be exact",
            vec!["transcript", "format"],
            json!(" web_vtt "),
        ),
        (
            "transcript relation",
            vec!["transcript", "relation"],
            json!("chapters"),
        ),
    ];

    for (case, path, replacement) in cases {
        let mut value = valid_fixture();
        value[path[0]][path[1]] = replacement;
        let error = parse(&value).expect_err(case);
        assert!(
            matches!(&error, PodcastFixtureError::InvalidMetadata { .. }),
            "{case}: {error:?}"
        );
        assert_eq!(error.code(), "podcast_fixture_invalid_metadata", "{case}");
    }
}

#[test]
fn source_and_resolved_resource_urls_are_validated_separately() {
    let source = parse_podcast_fixture(
        "fixture.json",
        &serde_json::to_vec(&valid_fixture()).expect("fixture must serialize"),
    )
    .expect_err("relative source URL must fail");
    assert!(matches!(&source, PodcastFixtureError::InvalidSourceUrl));
    assert_eq!(source.code(), "podcast_fixture_invalid_source_url");

    let mut value = valid_fixture();
    value["episode"]["enclosure_url"] = json!("javascript:alert(1)");
    let resource = parse(&value).expect_err("non-HTTP resource URL must fail");
    assert!(matches!(
        &resource,
        PodcastFixtureError::InvalidResolvedUrl { .. }
    ));
    assert_eq!(resource.code(), "podcast_fixture_invalid_resolved_url");

    let mut empty = valid_fixture();
    empty["episode"]["enclosure_url"] = json!("  ");
    let resource = parse(&empty).expect_err("empty resource URL must fail");
    assert!(matches!(
        &resource,
        PodcastFixtureError::InvalidResolvedUrl { .. }
    ));
}

#[test]
fn cue_list_and_cue_metadata_validation_report_cue_indices() {
    let mut empty = valid_fixture();
    empty["transcript"]["cues"] = json!([]);
    let error = parse(&empty).expect_err("empty cue list must fail");
    assert!(matches!(&error, PodcastFixtureError::EmptyCues));
    assert_eq!(error.code(), "podcast_fixture_empty_cues");
    assert_eq!(error.cue_index(), None);

    let cases = [
        ("empty source ID", 0, json!(""), "source_cue_id"),
        ("empty cue text", 0, json!(" \n "), "text"),
        ("equal bounds", 3_000, json!(3_000), "end_ms"),
    ];
    for (case, start_ms, replacement, field) in cases {
        let mut value = valid_fixture();
        value["transcript"]["cues"][0]["start_ms"] = json!(start_ms);
        value["transcript"]["cues"][0][field] = replacement;
        let error = parse(&value).expect_err(case);
        assert!(
            matches!(&error, PodcastFixtureError::InvalidCue { cue_index: 0, .. }),
            "{case}: {error:?}"
        );
        assert_eq!(error.code(), "podcast_fixture_invalid_cue", "{case}");
        assert_eq!(error.cue_index(), Some(0), "{case}");
    }
}

#[test]
fn unordered_cues_and_cues_beyond_episode_duration_are_rejected() {
    let mut unordered = valid_fixture();
    unordered["transcript"]["cues"][1]["start_ms"] = json!(2_000);
    unordered["transcript"]["cues"][0]["start_ms"] = json!(2_500);
    let error = parse(&unordered).expect_err("decreasing starts must fail");
    assert!(matches!(
        &error,
        PodcastFixtureError::InvalidCue { cue_index: 1, .. }
    ));
    assert_eq!(error.cue_index(), Some(1));

    let mut overflow = valid_fixture();
    overflow["transcript"]["cues"][1]["end_ms"] = json!(7_001);
    let error = parse(&overflow).expect_err("cue beyond duration must fail");
    assert!(matches!(
        &error,
        PodcastFixtureError::InvalidCue { cue_index: 1, .. }
    ));
    assert_eq!(error.cue_index(), Some(1));
}
