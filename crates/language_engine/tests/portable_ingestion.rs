use language_engine::{parse_podcast_feed, parse_transcript};

const RSS: &[u8] = include_bytes!("fixtures/portable_ingestion/rss.xml");
const ATOM: &[u8] = include_bytes!("fixtures/portable_ingestion/atom.xml");
const WEBVTT: &str = include_str!("fixtures/portable_ingestion/captions.vtt");
const SRT: &str = include_str!("fixtures/portable_ingestion/captions.srt");

#[test]
fn rss_webvtt_and_atom_srt_compose_into_equivalent_portable_documents() {
    let cases = [
        (
            "RSS/WebVTT",
            "https://media.example.test/rss/feed.xml",
            RSS,
            WEBVTT,
        ),
        (
            "Atom/SRT",
            "https://media.example.test/atom/feed.xml",
            ATOM,
            SRT,
        ),
    ];
    let mut projections = Vec::new();

    for (case, source_url, feed_source, transcript_source) in cases {
        let report = parse_podcast_feed(source_url, feed_source).expect(case);
        assert!(report.issues.is_empty(), "{case}");
        let resource = report.episodes[0].transcript_resources[0].clone();
        let document = parse_transcript(resource, transcript_source).expect(case);
        projections.push(
            document
                .cues()
                .iter()
                .map(|cue| {
                    (
                        cue.start_ms(),
                        cue.end_ms(),
                        cue.text().to_string(),
                        cue.tokens()
                            .iter()
                            .map(|token| token.surface().to_string())
                            .collect::<Vec<_>>(),
                    )
                })
                .collect::<Vec<_>>(),
        );
    }

    assert_eq!(projections[0], projections[1]);
    assert_eq!(projections[0][0].0, 500);
    assert_eq!(projections[0][0].2, "Portable café text.");
    assert_eq!(projections[0][0].3, ["Portable", "café", "text"]);
}
