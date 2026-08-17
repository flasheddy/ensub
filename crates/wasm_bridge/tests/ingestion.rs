use core_engine::{TranscriptFormat, TranscriptResource};
use ensub_wasm::{parse_podcast_feed_dto, parse_transcript_dto, TranscriptResourceDto};
use serde_json::Value;

#[test]
fn feed_adapter_serializes_camel_case_dtos_and_epoch_timestamps() {
    let rss = br#"<rss version="2.0">
      <channel><title>WASM Feed</title><item>
        <guid>wasm-episode</guid><title>WASM Episode</title>
        <pubDate>Mon, 17 Aug 2026 08:00:00 +0000</pubDate>
        <enclosure url="/episode.mp3" type="audio/mpeg" />
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
    assert!(json["episodes"][0].get("published_at").is_none());
}

#[test]
fn transcript_adapter_converts_utf8_byte_offsets_to_explicit_utf16_offsets() {
    let resource = TranscriptResourceDto::from(TranscriptResource {
        url: "https://media.example.test/captions.vtt".to_string(),
        mime_type: "text/vtt".to_string(),
        format: Some(TranscriptFormat::WebVtt),
        language: Some("en".to_string()),
        relation: Some("captions".to_string()),
    });
    let output = parse_transcript_dto(resource, "WEBVTT\n\n00:00.000 --> 00:01.000\n🙂 café")
        .expect("WASM transcript DTO must parse");

    assert_eq!(output.cues[0].text, "🙂 café");
    assert_eq!(output.cues[0].tokens.len(), 1);
    assert_eq!(output.cues[0].tokens[0].surface, "café");
    assert_eq!(output.cues[0].tokens[0].start_utf16, 3);
    assert_eq!(output.cues[0].tokens[0].end_utf16, 7);

    let json: Value = serde_json::to_value(output).expect("transcript DTO must serialize");
    assert_eq!(json["cues"][0]["tokens"][0]["startUtf16"], 3);
    assert!(json["cues"][0]["tokens"][0].get("start_byte").is_none());
}
