#![cfg(target_arch = "wasm32")]

use js_sys::{Reflect, Uint8Array};
use language_engine::{
    BrowserLexiconAsset, BrowserLexiconForm, Definition, LexiconEntry,
    BROWSER_LEXICON_SCHEMA_VERSION,
};
use wasm_bindgen_test::*;

use ensub_wasm::{
    parse_podcast_feed, parse_transcript, CaptureParsedInput, CapturePodcastInput,
    CapturePodcastOutput, CapturePodcastStatus, EnsubPlayerLearning, EnsubPlayerWorkspace,
    EnsubSandbox, EpisodeOpenDto, ParseInput, ParseOutput, PlayerWorkspaceDto,
    PodcastFeedParseOutputDto, PreparePodcastCaptureInput, PreparedPodcastCaptureDto, StatsInput,
    StatsOutput, TokenLookupDto, TranscriptDocumentDto, TranscriptResourceDto, TranscriptSyncDto,
};

#[wasm_bindgen_test]
fn exported_player_workspace_round_trips_cache_and_syncs_cues() {
    let rss = br#"<rss version="2.0"><channel><title>Player</title><item>
      <guid>player-episode</guid><title>Player Episode</title>
      <enclosure url="https://media.example.test/audio.mp3" type="audio/mpeg" />
      <podcast:transcript xmlns:podcast="https://podcastindex.org/namespace/1.0"
        url="https://media.example.test/captions.vtt" type="text/vtt" language="en" />
    </item></channel></rss>"#;
    let mut workspace = EnsubPlayerWorkspace::new(Uint8Array::new_with_length(0))
        .expect("empty player cache must open");
    let view = workspace
        .import_feed(
            "https://media.example.test/feed.xml".to_string(),
            Uint8Array::from(rss.as_slice()),
            1_787_000_000_000.0,
        )
        .expect("feed must import");
    let view: PlayerWorkspaceDto =
        serde_wasm_bindgen::from_value(view).expect("view must deserialize");
    let episode_id = view.episodes[0].identity.internal_id.clone();
    let opened = workspace
        .select_episode(episode_id.clone())
        .expect("episode must select");
    let _: EpisodeOpenDto =
        serde_wasm_bindgen::from_value(opened).expect("episode must deserialize");
    workspace
        .cache_transcript(
            episode_id,
            "https://media.example.test/captions.vtt".to_string(),
            "WEBVTT\n\n00:01.000 --> 00:02.000\nPlayer cue".to_string(),
            1_787_000_000_100.0,
        )
        .expect("transcript must cache");
    let sync: TranscriptSyncDto =
        serde_wasm_bindgen::from_value(workspace.sync_at(1_500.0).expect("cue must sync"))
            .expect("sync must deserialize");
    assert_eq!(sync.active_cue_indices, [0]);

    let snapshot = workspace.snapshot().expect("snapshot must encode");
    let restored = EnsubPlayerWorkspace::new(snapshot).expect("snapshot must restore");
    let restored_view: PlayerWorkspaceDto =
        serde_wasm_bindgen::from_value(restored.view().expect("restored view must serialize"))
            .expect("restored view must deserialize");
    assert_eq!(restored_view.revision, 3);
}

#[wasm_bindgen_test]
fn exported_player_lookup_and_capture_flow_is_offline_and_atomic() {
    let key = "ensub.player-learning-browser-test.v2";
    let storage = web_sys::window()
        .and_then(|window| window.local_storage().ok().flatten())
        .expect("browser test requires localStorage");
    storage.remove_item(key).expect("fixture must reset");
    let rss = br#"<rss version="2.0"><channel><title>Player</title><item>
      <guid>player-episode</guid><title>Player Episode</title>
      <enclosure url="https://media.example.test/audio.mp3" type="audio/mpeg" />
      <podcast:transcript xmlns:podcast="https://podcastindex.org/namespace/1.0"
        url="https://media.example.test/captions.vtt" type="text/vtt" language="en" />
    </item></channel></rss>"#;
    let mut workspace =
        EnsubPlayerWorkspace::new(Uint8Array::new_with_length(0)).expect("workspace must open");
    let view: PlayerWorkspaceDto = serde_wasm_bindgen::from_value(
        workspace
            .import_feed(
                "https://media.example.test/feed.xml".to_string(),
                Uint8Array::from(rss.as_slice()),
                1.0,
            )
            .expect("feed must import"),
    )
    .expect("view must decode");
    let episode_id = view.episodes[0].identity.internal_id.clone();
    workspace
        .select_episode(episode_id.clone())
        .expect("episode must select");
    workspace
        .cache_transcript(
            episode_id.clone(),
            "https://media.example.test/captions.vtt".to_string(),
            "WEBVTT\n\n00:01.000 --> 00:02.000\nImmersion works.".to_string(),
            2.0,
        )
        .expect("transcript must cache");
    let prepared: PreparedPodcastCaptureDto = serde_wasm_bindgen::from_value(
        workspace
            .prepare_podcast_capture(
                serde_wasm_bindgen::to_value(&PreparePodcastCaptureInput {
                    revision: 3,
                    episode_id,
                    transcript_url: "https://media.example.test/captions.vtt".to_string(),
                    cue_id: "cue-0".to_string(),
                    token_index: 0,
                    playback_position_ms: 1_500,
                    duration_ms: Some(2_000),
                })
                .expect("input must encode"),
            )
            .expect("capture must prepare"),
    )
    .expect("prepared capture must decode");
    let bytes = lexicon_bytes();
    let mut learning =
        EnsubPlayerLearning::new(Uint8Array::from(bytes.as_slice()), key.to_string(), false)
            .expect("learning runtime must open");
    let lookup: TokenLookupDto = serde_wasm_bindgen::from_value(
        learning
            .lookup_token(prepared.surface.clone())
            .expect("lookup must execute"),
    )
    .expect("lookup must decode");
    assert!(matches!(lookup, TokenLookupDto::Found { .. }));
    let captured: CapturePodcastOutput = serde_wasm_bindgen::from_value(
        learning
            .capture_podcast(
                serde_wasm_bindgen::to_value(&CapturePodcastInput {
                    draft: prepared.draft,
                    selected_lemma: None,
                    captured_at_ms: 1_755_244_800_000,
                })
                .expect("capture input must encode"),
            )
            .expect("capture must save"),
    )
    .expect("capture output must decode");
    assert_eq!(captured.status, CapturePodcastStatus::CreatedCard);
    storage.remove_item(key).expect("fixture must clean up");
}

wasm_bindgen_test_configure!(run_in_browser);

fn lexicon_bytes() -> Vec<u8> {
    BrowserLexiconAsset {
        schema_version: BROWSER_LEXICON_SCHEMA_VERSION,
        definition_source: "browser test".to_string(),
        pronunciation_source: "browser test".to_string(),
        entries: vec![LexiconEntry {
            lemma: "immersion".to_string(),
            phonetic: "ɪˈmɜːʃən".to_string(),
            definitions: vec![Definition {
                part_of_speech: "noun".to_string(),
                text: "deep involvement".to_string(),
            }],
        }],
        forms: vec![BrowserLexiconForm {
            surface: "immersion".to_string(),
            entry_index: 0,
            priority: 0,
        }],
    }
    .encode()
    .expect("fixture lexicon must encode")
}

#[wasm_bindgen_test]
fn exported_sandbox_round_trips_javascript_dtos_through_local_storage() {
    let key = "ensub.wasm-browser-test.v1";
    let storage = web_sys::window()
        .and_then(|window| window.local_storage().ok().flatten())
        .expect("browser test requires localStorage");
    storage.remove_item(key).expect("fixture must reset");
    let bytes = lexicon_bytes();
    let mut sandbox = EnsubSandbox::new(Uint8Array::from(bytes.as_slice()), key.to_string(), false)
        .expect("sandbox must open");
    let parsed = sandbox
        .parse(
            serde_wasm_bindgen::to_value(&ParseInput {
                text: "Immersion works.".to_string(),
                include_stopwords: false,
                max_candidates: 10,
            })
            .expect("parse input must serialize"),
        )
        .expect("text must parse");
    let parsed: ParseOutput =
        serde_wasm_bindgen::from_value(parsed).expect("parse output must deserialize");
    let candidate_id = parsed.candidates[0].id.clone();
    sandbox
        .capture_parsed(
            serde_wasm_bindgen::to_value(&CaptureParsedInput {
                text: "Immersion works.".to_string(),
                candidate_ids: vec![candidate_id],
                source: "wasm:test".to_string(),
                captured_at_ms: 1_755_244_800_000_i64,
                include_stopwords: false,
                max_candidates: 10,
            })
            .expect("capture input must serialize"),
        )
        .expect("candidate must save");

    let reopened = EnsubSandbox::new(Uint8Array::from(bytes.as_slice()), key.to_string(), true)
        .expect("read-only sandbox must reopen");
    let stats = reopened
        .stats(
            serde_wasm_bindgen::to_value(&StatsInput {
                as_of_ms: 1_755_244_800_000_i64,
            })
            .expect("stats input must serialize"),
        )
        .expect("stats must load");
    let stats: StatsOutput =
        serde_wasm_bindgen::from_value(stats).expect("stats output must deserialize");
    assert_eq!(stats.total_cards, 1);
    assert_eq!(stats.due_cards, 1);
    storage.remove_item(key).expect("fixture must clean up");
}

#[wasm_bindgen_test]
fn exported_ingestion_functions_round_trip_dtos_and_structured_errors() {
    let rss = br#"<rss version="2.0"><channel><title>Browser Feed</title><item>
      <title>Browser Episode</title><enclosure url="/audio.mp3" type="audio/mpeg" />
    </item></channel></rss>"#;
    let feed = parse_podcast_feed(
        "https://media.example.test/feed.xml".to_string(),
        Uint8Array::from(rss.as_slice()),
    )
    .expect("browser feed must parse");
    let feed: PodcastFeedParseOutputDto =
        serde_wasm_bindgen::from_value(feed).expect("feed DTO must deserialize");
    assert_eq!(feed.episodes[0].title, "Browser Episode");

    let resource = TranscriptResourceDto {
        url: "https://media.example.test/captions.vtt".to_string(),
        mime_type: "text/vtt".to_string(),
        format: Some(core_engine::TranscriptFormat::WebVtt),
        language: Some("en".to_string()),
        relation: Some("captions".to_string()),
    };
    let resource = serde_wasm_bindgen::to_value(&resource).expect("resource must serialize");
    let transcript = parse_transcript(
        resource.clone(),
        "WEBVTT\n\n00:00.000 --> 00:01.000\nBrowser text".to_string(),
    )
    .expect("browser transcript must parse");
    let transcript: TranscriptDocumentDto =
        serde_wasm_bindgen::from_value(transcript).expect("transcript DTO must deserialize");
    assert_eq!(transcript.cues[0].text, "Browser text");

    let error = parse_transcript(resource, "WEBVTT\n\nmissing timing".to_string())
        .expect_err("malformed transcript must return an EnsubError");
    assert_eq!(
        Reflect::get(&error, &"code".into())
            .expect("error code must exist")
            .as_string()
            .as_deref(),
        Some("transcript_missing_timing_line")
    );
    assert_eq!(
        Reflect::get(&error, &"line".into())
            .expect("error line must exist")
            .as_f64(),
        Some(3.0)
    );
    assert_eq!(
        Reflect::get(&error, &"cueIndex".into())
            .expect("error cue index must exist")
            .as_f64(),
        Some(0.0)
    );
}
