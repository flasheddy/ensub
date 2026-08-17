use core_engine::{TranscriptDocument, TranscriptFormat, TranscriptResource};
use language_engine::parse_transcript;

fn resource(format: Option<TranscriptFormat>) -> TranscriptResource {
    let (url, mime_type) = match format {
        Some(TranscriptFormat::WebVtt) => ("captions.vtt", "text/vtt"),
        Some(TranscriptFormat::Srt) => ("captions.srt", "application/srt"),
        None => ("captions.json", "application/json"),
    };
    TranscriptResource {
        url: format!("https://media.example.test/{url}"),
        mime_type: mime_type.to_string(),
        format,
        language: Some("en".to_string()),
        relation: Some("captions".to_string()),
    }
}

fn cue_projection(document: &TranscriptDocument) -> Vec<(u64, u64, String, Vec<String>)> {
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
                    .collect(),
            )
        })
        .collect()
}

#[test]
fn webvtt_and_srt_produce_equivalent_normalized_cues_and_tokens() {
    let webvtt = "\u{feff}WEBVTT Synthetic captions\r\n\r\n\
NOTE generated fixture\r\nignored\r\n\r\n\
intro\r\n00:00.500 --> 00:02.000 align:start\r\n\
<v Narrator><b>Hello</b>, café &amp; tea.\r\nSecond line\r\n\r\n\
00:02.000 --> 00:03.250\r\n<i>Done.</i>\r\n";
    let srt = "1\n00:00:00,500 --> 00:00:02,000\n\
<b>Hello</b>, café &amp; tea.\nSecond line\n\n\
2\n00:00:02.000 --> 00:00:03.250\n<i>Done.</i>\n";

    let webvtt = parse_transcript(resource(Some(TranscriptFormat::WebVtt)), webvtt)
        .expect("WebVTT fixture must parse");
    let srt = parse_transcript(resource(Some(TranscriptFormat::Srt)), srt)
        .expect("SRT fixture must parse");

    assert_eq!(cue_projection(&webvtt), cue_projection(&srt));
    assert_eq!(
        cue_projection(&webvtt),
        vec![
            (
                500,
                2_000,
                "Hello, café & tea.\nSecond line".to_string(),
                vec!["Hello", "café", "tea", "Second", "line"]
                    .into_iter()
                    .map(str::to_string)
                    .collect(),
            ),
            (2_000, 3_250, "Done.".to_string(), vec!["Done".to_string()],),
        ]
    );
    assert_eq!(webvtt.cues()[0].id(), "cue-0");
    assert_eq!(webvtt.cues()[0].source_cue_id(), Some("intro"));
    assert_eq!(srt.cues()[0].source_cue_id(), Some("1"));

    for document in [&webvtt, &srt] {
        for cue in document.cues() {
            for token in cue.tokens() {
                let start = usize::try_from(token.start_byte()).expect("test offset must fit");
                let end = usize::try_from(token.end_byte()).expect("test offset must fit");
                assert_eq!(cue.text().get(start..end), Some(token.surface()));
            }
        }
    }
}

#[test]
fn transcript_failures_report_stable_codes_and_locations() {
    let cases = [
        (
            "unsupported resource",
            resource(None),
            "ignored",
            "transcript_unsupported_format",
            None,
            None,
        ),
        (
            "missing WebVTT header",
            resource(Some(TranscriptFormat::WebVtt)),
            "00:00.000 --> 00:01.000\nText",
            "transcript_missing_webvtt_header",
            None,
            Some(1),
        ),
        (
            "missing timing line",
            resource(Some(TranscriptFormat::WebVtt)),
            "WEBVTT\n\nidentifier\nText only",
            "transcript_missing_timing_line",
            Some(0),
            Some(3),
        ),
        (
            "invalid timestamp component",
            resource(Some(TranscriptFormat::Srt)),
            "1\n00:61:00,000 --> 00:62:00,000\nText",
            "transcript_invalid_timestamp",
            Some(0),
            Some(2),
        ),
        (
            "timestamp overflow",
            resource(Some(TranscriptFormat::Srt)),
            "1\n18446744073709551615:00:00,000 --> 00:00:01,000\nText",
            "transcript_timestamp_overflow",
            Some(0),
            Some(2),
        ),
        (
            "invalid cue bounds",
            resource(Some(TranscriptFormat::WebVtt)),
            "WEBVTT\n\n00:02.000 --> 00:02.000\nText",
            "transcript_invalid_cue_bounds",
            Some(0),
            Some(3),
        ),
        (
            "decreasing cue starts",
            resource(Some(TranscriptFormat::WebVtt)),
            "WEBVTT\n\n00:02.000 --> 00:04.000\nFirst\n\n00:01.000 --> 00:03.000\nSecond",
            "transcript_cue_start_out_of_order",
            Some(1),
            Some(6),
        ),
    ];

    for (case, resource, source, code, cue_index, line) in cases {
        let error = parse_transcript(resource, source).expect_err(case);
        assert_eq!(error.code(), code, "{case}");
        assert_eq!(error.cue_index(), cue_index, "{case}");
        assert_eq!(error.line(), line, "{case}");
    }
}

#[test]
fn empty_cues_are_validated_then_discarded_and_retained_cues_are_renumbered() {
    let source = "1\n00:00:00,000 --> 00:00:01,000\n\n\
2\n00:00:01,000 --> 00:00:02,000\nRetained";
    let document = parse_transcript(resource(Some(TranscriptFormat::Srt)), source)
        .expect("empty cue with valid timing must be discarded");

    assert_eq!(document.cues().len(), 1);
    assert_eq!(document.cues()[0].id(), "cue-0");
    assert_eq!(document.cues()[0].source_order(), 0);
    assert_eq!(document.cues()[0].source_cue_id(), Some("2"));

    let invalid_empty = "1\n00:00:01,000 --> 00:00:01,000\n";
    let error = parse_transcript(resource(Some(TranscriptFormat::Srt)), invalid_empty)
        .expect_err("empty cues must still validate bounds");
    assert_eq!(error.code(), "transcript_invalid_cue_bounds");
}

#[test]
fn timestamp_variants_overlaps_and_text_normalization_are_table_driven() {
    let cases = [
        (
            "WebVTT fractional scaling and equal starts",
            TranscriptFormat::WebVtt,
            "WEBVTT\n\n00:00.1 --> 00:01.25\n{\\an8}<c.note>One</c> &lt;two&gt;\n\n00:00.100 --> 00:00.900\nNested",
            vec![(100, 1_250, "One <two>"), (100, 900, "Nested")],
        ),
        (
            "SRT overlap and unknown literal markup",
            TranscriptFormat::Srt,
            "1\n0:00:00,10 --> 0:00:02,0\n<unknown>Literal</unknown>\n\n2\n0:00:01,000 --> 0:00:03,000\nOverlap",
            vec![
                (100, 2_000, "<unknown>Literal</unknown>"),
                (1_000, 3_000, "Overlap"),
            ],
        ),
    ];

    for (case, format, source, expected) in cases {
        let document = parse_transcript(resource(Some(format)), source).expect(case);
        let actual: Vec<(u64, u64, &str)> = document
            .cues()
            .iter()
            .map(|cue| (cue.start_ms(), cue.end_ms(), cue.text()))
            .collect();
        assert_eq!(actual, expected, "{case}");
    }

    let empty_webvtt = parse_transcript(resource(Some(TranscriptFormat::WebVtt)), "WEBVTT\n")
        .expect("header-only WebVTT must be valid");
    let empty_srt = parse_transcript(resource(Some(TranscriptFormat::Srt)), " \n")
        .expect("blank SRT must be valid");
    assert!(empty_webvtt.cues().is_empty());
    assert!(empty_srt.cues().is_empty());
}
