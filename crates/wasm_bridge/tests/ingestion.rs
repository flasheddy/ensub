use core_engine::{TranscriptFormat, TranscriptResource};
use ensub_wasm::{
    parse_podcast_feed_dto, parse_transcript_dto, IngestionError, TranscriptResourceDto,
};
use language_engine::{TimestampEndpoint, TranscriptParseError};
use serde_json::Value;

fn resource(format: TranscriptFormat) -> TranscriptResourceDto {
    let (url, mime_type) = match format {
        TranscriptFormat::WebVtt => ("captions.vtt", "text/vtt"),
        TranscriptFormat::Srt => ("captions.srt", "application/srt"),
    };
    TranscriptResourceDto::from(TranscriptResource {
        url: format!("https://media.example.test/{url}"),
        mime_type: mime_type.to_string(),
        format: Some(format),
        language: Some("en".to_string()),
        relation: Some("captions".to_string()),
    })
}

#[test]
fn feed_adapter_serializes_transcript_resources_identity_and_epoch_timestamps() {
    let rss = br#"<rss version="2.0"
      xmlns:p="https://podcastindex.org/namespace/1.0">
      <channel><title>WASM Feed</title><item>
        <guid>wasm-episode</guid><title>WASM Episode</title>
        <pubDate>Mon, 17 Aug 2026 08:00:00 +0000</pubDate>
        <enclosure url="/episode.mp3" type="audio/mpeg" />
        <p:transcript url="/captions.vtt" type="text/vtt" language="en" rel="captions" />
        <p:transcript url="/captions.srt" type="application/srt" language="en" />
      </item></channel>
    </rss>"#;
    let output = parse_podcast_feed_dto("https://media.example.test/feed.xml", rss)
        .expect("WASM feed DTO must parse");
    let json = serde_json::to_value(output).expect("WASM feed DTO must serialize");

    assert_eq!(
        json["feed"]["sourceUrl"],
        "https://media.example.test/feed.xml"
    );
    assert_eq!(json["episodes"][0]["publishedAtMs"], 1_786_953_600_000_i64);
    assert!(json["episodes"][0]["identity"]["internalId"]
        .as_str()
        .is_some());
    assert!(json["episodes"][0]["identity"].get("internal_id").is_none());
    assert_eq!(
        json["episodes"][0]["transcriptResources"][0]["url"],
        "https://media.example.test/captions.vtt"
    );
    assert_eq!(
        json["episodes"][0]["transcriptResources"][0]["format"],
        "web_vtt"
    );
    assert_eq!(
        json["episodes"][0]["transcriptResources"][0]["language"],
        "en"
    );
    assert_eq!(
        json["episodes"][0]["transcriptResources"][0]["relation"],
        "captions"
    );
    assert_eq!(
        json["episodes"][0]["transcriptResources"][1]["format"],
        "srt"
    );
    assert!(json["episodes"][0].get("published_at").is_none());
}

#[test]
fn webvtt_and_srt_adapters_emit_equivalent_cue_relative_utf16_offsets() {
    let cases = [
        (
            TranscriptFormat::WebVtt,
            "WEBVTT\n\n00:00.000 --> 00:01.000\n🙂 (café),\nnaïve!",
        ),
        (
            TranscriptFormat::Srt,
            "1\n00:00:00,000 --> 00:00:01,000\n🙂 (café),\nnaïve!",
        ),
    ];

    for (format, source) in cases {
        let output =
            parse_transcript_dto(resource(format), source).expect("WASM transcript DTO must parse");
        let tokens: Vec<(&str, usize, usize)> = output.cues[0]
            .tokens
            .iter()
            .map(|token| (token.surface.as_str(), token.start_utf16, token.end_utf16))
            .collect();

        assert_eq!(output.cues[0].text, "🙂 (café),\nnaïve!");
        assert_eq!(tokens, [("café", 4, 8), ("naïve", 11, 16)]);

        let json: Value = serde_json::to_value(output).expect("transcript DTO must serialize");
        assert_eq!(json["cues"][0]["tokens"][0]["startUtf16"], 4);
        assert_eq!(json["cues"][0]["tokens"][1]["endUtf16"], 16);
        assert!(json["cues"][0]["tokens"][0].get("startByte").is_none());
        assert!(json["cues"][0]["tokens"][0].get("start_byte").is_none());
    }
}

#[test]
fn malformed_srt_error_preserves_typed_endpoint_and_location_metadata() {
    let error = parse_transcript_dto(
        resource(TranscriptFormat::Srt),
        "1\n00:00:00,000 --> not-a-time\nText",
    )
    .expect_err("invalid SRT must return a typed ingestion error");

    assert_eq!(error.code(), "transcript_invalid_timestamp");
    assert_eq!(error.line(), Some(2));
    assert_eq!(error.cue_index(), Some(0));
    assert!(matches!(
        error,
        IngestionError::Transcript(TranscriptParseError::InvalidTimestamp {
            cue_index: 0,
            line: 2,
            endpoint: TimestampEndpoint::End,
        })
    ));
}
