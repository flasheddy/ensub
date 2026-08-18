#![cfg(target_arch = "wasm32")]

use js_sys::{Reflect, Uint8Array};
use language_engine::{
    BrowserLexiconAsset, BrowserLexiconForm, Definition, LexiconEntry,
    BROWSER_LEXICON_SCHEMA_VERSION,
};
use wasm_bindgen_test::*;

use ensub_wasm::{
    parse_podcast_feed, parse_transcript, CaptureParsedInput, CapturePodcastInput,
    CapturePodcastOutput, CapturePodcastStatus, CueNavigationDto, DueCountDto, DueCountInputDto,
    DueReviewsDto, DueReviewsInputDto, EnsubPlayerLearning, EnsubPlayerWorkspace, EnsubSandbox,
    EpisodeOpenDto, ParseInput, ParseOutput, PlayerWorkspaceDto, PodcastFeedParseOutputDto,
    PrepareDisambiguationInputDto, PreparePodcastCaptureInput, PreparedDisambiguationDto,
    PreparedPodcastCaptureDto, RateReviewInputDto, RevealReviewInputDto, ReviewAnswerDto,
    ReviewTransitionDto, StatsInput, StatsOutput, TokenLookupDto, TranscriptDocumentDto,
    TranscriptResourceDto, TranscriptSyncDto, ValidateDisambiguationResponseInputDto,
    MAX_PLAYER_FIXTURE_BYTES,
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
fn exported_demo_import_and_adjacent_navigation_use_browser_dto_shapes() {
    let fixture = br#"{
      "schema_version": 1,
      "feed": {
        "title": "Browser Demo",
        "language": "en",
        "description": "A synthetic browser fixture.",
        "artwork_url": "/feed.png"
      },
      "episode": {
        "publisher_guid": "browser-demo",
        "title": "Browser Demo Episode",
        "published_at": "2026-08-17T08:00:00Z",
        "language": "en",
        "enclosure_url": "/audio.mp3",
        "enclosure_mime_type": "audio/mpeg",
        "duration_ms": 4000,
        "artwork_url": "/episode.png"
      },
      "transcript": {
        "url": "/captions.vtt#en",
        "mime_type": "text/vtt",
        "format": "web_vtt",
        "language": "en",
        "relation": "captions",
        "cues": [
          {"source_cue_id":"one","start_ms":1000,"end_ms":2000,"text":"One."},
          {"source_cue_id":"two","start_ms":3000,"end_ms":4000,"text":"Two."}
        ]
      }
    }"#;
    let mut workspace = EnsubPlayerWorkspace::new(Uint8Array::new_with_length(0))
        .expect("empty player cache must open");
    let opened = workspace
        .import_demo_fixture(
            "https://media.example.test/demo.json".to_string(),
            Uint8Array::from(fixture.as_slice()),
            1_787_000_000_000.0,
        )
        .expect("demo fixture must import");
    assert!(Reflect::has(&opened, &"selectedTranscriptUrl".into())
        .expect("selected transcript field must inspect"));
    assert_eq!(
        Reflect::get(&opened, &"transcriptState".into())
            .expect("transcript state must inspect")
            .as_string()
            .as_deref(),
        Some("ready")
    );
    let opened: EpisodeOpenDto =
        serde_wasm_bindgen::from_value(opened).expect("ready DTO must deserialize");
    assert_eq!(
        opened.transcript_state,
        ensub_wasm::TranscriptStateDto::Ready
    );
    assert_eq!(
        opened
            .transcript
            .as_ref()
            .map(|document| document.cues.len()),
        Some(2)
    );

    let next_value = workspace.next_cue_at(0.0).expect("next cue must serialize");
    assert!(Reflect::has(&next_value, &"cueIndex".into()).expect("cue index must inspect"));
    assert!(Reflect::has(&next_value, &"startMs".into()).expect("cue start must inspect"));
    let next: CueNavigationDto =
        serde_wasm_bindgen::from_value(next_value).expect("next cue must deserialize");
    assert_eq!(next.cue_index, 0);
    assert_eq!(next.start_ms, 1_000);

    let previous = workspace
        .previous_cue_at(0.0)
        .expect("empty previous boundary must serialize");
    assert!(previous.is_null());

    let error = workspace
        .next_cue_at(-1.0)
        .expect_err("negative playback position must fail");
    assert_eq!(
        Reflect::get(&error, &"code".into())
            .expect("error code must inspect")
            .as_string()
            .as_deref(),
        Some("invalid_argument")
    );
}

#[wasm_bindgen_test]
fn exported_demo_import_rejects_oversized_bytes_before_other_validation() {
    let oversized_length = u32::try_from(MAX_PLAYER_FIXTURE_BYTES + 1)
        .expect("fixture limit must fit a browser buffer length");
    let mut workspace = EnsubPlayerWorkspace::new(Uint8Array::new_with_length(0))
        .expect("empty player cache must open");

    let error = workspace
        .import_demo_fixture(
            "https://media.example.test/demo.json".to_string(),
            Uint8Array::new_with_length(oversized_length),
            -1.0,
        )
        .expect_err("oversized fixture must fail before timestamp validation");

    assert_eq!(
        Reflect::get(&error, &"code".into())
            .expect("error code must inspect")
            .as_string()
            .as_deref(),
        Some("player_fixture_too_large")
    );
}

#[wasm_bindgen_test]
fn exported_demo_import_rolls_back_when_ready_dto_serialization_fails() {
    let fixture = br#"{
      "schema_version": 1,
      "feed": {
        "title": "Large Timestamp Demo",
        "language": "en",
        "description": "A synthetic browser fixture.",
        "artwork_url": "/feed.png"
      },
      "episode": {
        "publisher_guid": "large-timestamp-demo",
        "title": "Large Timestamp Episode",
        "published_at": "2026-08-17T08:00:00Z",
        "language": "en",
        "enclosure_url": "/audio.mp3",
        "enclosure_mime_type": "audio/mpeg",
        "duration_ms": 9007199254740992,
        "artwork_url": "/episode.png"
      },
      "transcript": {
        "url": "/captions.vtt",
        "mime_type": "text/vtt",
        "format": "web_vtt",
        "language": "en",
        "relation": "captions",
        "cues": [
          {
            "source_cue_id": "large",
            "start_ms": 0,
            "end_ms": 9007199254740992,
            "text": "Large timestamp."
          }
        ]
      }
    }"#;
    let mut workspace = EnsubPlayerWorkspace::new(Uint8Array::new_with_length(0))
        .expect("empty player cache must open");
    let before = workspace.snapshot().expect("snapshot must encode").to_vec();

    let error = workspace
        .import_demo_fixture(
            "https://media.example.test/demo.json".to_string(),
            Uint8Array::from(fixture.as_slice()),
            1.0,
        )
        .expect_err("unsafe JavaScript integer must fail DTO serialization");
    assert_eq!(
        Reflect::get(&error, &"code".into())
            .expect("error code must inspect")
            .as_string()
            .as_deref(),
        Some("serialization_failed")
    );
    assert_eq!(
        workspace.snapshot().expect("snapshot must encode").to_vec(),
        before
    );
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
                    draft: prepared.draft.clone(),
                    selected_lemma: None,
                    captured_at_ms: 1_755_244_800_000,
                })
                .expect("capture input must encode"),
            )
            .expect("capture must save"),
    )
    .expect("capture output must decode");
    assert_eq!(captured.status, CapturePodcastStatus::CreatedCard);

    let due_count: DueCountDto = serde_wasm_bindgen::from_value(
        learning
            .due_count(
                serde_wasm_bindgen::to_value(&DueCountInputDto {
                    as_of_ms: 1_755_244_800_000,
                })
                .expect("due count input must encode"),
            )
            .expect("due count must execute"),
    )
    .expect("due count must decode");
    assert_eq!(due_count.due_count, 1);
    let due: DueReviewsDto = serde_wasm_bindgen::from_value(
        learning
            .due_reviews(
                serde_wasm_bindgen::to_value(&DueReviewsInputDto {
                    as_of_ms: 1_755_244_800_000,
                    limit: 10,
                })
                .expect("due input must encode"),
            )
            .expect("due reviews must execute"),
    )
    .expect("due reviews must decode");
    let prompt = &due.cards[0];
    let answer: ReviewAnswerDto = serde_wasm_bindgen::from_value(
        learning
            .reveal_review(
                serde_wasm_bindgen::to_value(&RevealReviewInputDto {
                    word_id: prompt.word_id.clone(),
                    review_token: prompt.review_token.clone(),
                })
                .expect("reveal input must encode"),
            )
            .expect("answer must reveal"),
    )
    .expect("answer must decode");
    assert_eq!(answer.lemma, "immersion");

    let disambiguation: PreparedDisambiguationDto = serde_wasm_bindgen::from_value(
        learning
            .prepare_disambiguation(
                serde_wasm_bindgen::to_value(&PrepareDisambiguationInputDto {
                    draft: prepared.draft,
                })
                .expect("disambiguation input must encode"),
            )
            .expect("disambiguation must prepare"),
    )
    .expect("prepared disambiguation must decode");
    let response = learning
        .validate_disambiguation_response(
            serde_wasm_bindgen::to_value(&ValidateDisambiguationResponseInputDto {
                request: disambiguation.request,
                response_json: r#"{"matchedSenseId":"sense-0-0","explanation":"Deep involvement.","confidence":"high"}"#.to_string(),
            })
            .expect("validation input must encode"),
        )
        .expect("provider response must validate");
    assert!(Reflect::has(&response, &"explanation".into()).expect("response must inspect"));

    let transition: ReviewTransitionDto = serde_wasm_bindgen::from_value(
        learning
            .review(
                serde_wasm_bindgen::to_value(&RateReviewInputDto {
                    word_id: prompt.word_id.clone(),
                    review_token: prompt.review_token.clone(),
                    rating: 4,
                    reviewed_at_ms: 1_755_244_800_000,
                })
                .expect("rating input must encode"),
            )
            .expect("rating must commit"),
    )
    .expect("transition must decode");
    assert_eq!(transition.rating, 4);
    storage.remove_item(key).expect("fixture must clean up");
}

#[wasm_bindgen_test]
fn v01_golden_snapshot_migrates_to_v2_in_firefox_after_committed_review() {
    let key = "ensub.player-learning-browser-migration.v1";
    let storage = web_sys::window()
        .and_then(|window| window.local_storage().ok().flatten())
        .expect("browser test requires localStorage");
    let original = include_str!("fixtures/browser-snapshot-v1.json");
    storage
        .set_item(key, original)
        .expect("golden snapshot must install");
    let bytes = lexicon_bytes();
    let mut learning =
        EnsubPlayerLearning::new(Uint8Array::from(bytes.as_slice()), key.to_string(), false)
            .expect("v0.1 snapshot must open");
    let due: DueReviewsDto = serde_wasm_bindgen::from_value(
        learning
            .due_reviews(
                serde_wasm_bindgen::to_value(&DueReviewsInputDto {
                    as_of_ms: 1_755_244_800_000,
                    limit: 10,
                })
                .expect("migration query must encode"),
            )
            .expect("v0.1 review must load"),
    )
    .expect("migration queue must decode");
    let prompt = &due.cards[0];
    learning
        .review(
            serde_wasm_bindgen::to_value(&RateReviewInputDto {
                word_id: prompt.word_id.clone(),
                review_token: prompt.review_token.clone(),
                rating: 4,
                reviewed_at_ms: 1_755_244_800_000,
            })
            .expect("migration rating must encode"),
        )
        .expect("migration rating must commit");
    let migrated = storage
        .get_item(key)
        .expect("migrated snapshot must read")
        .expect("migrated snapshot must exist");
    let value: serde_json::Value =
        serde_json::from_str(&migrated).expect("migrated snapshot must be JSON");
    assert_eq!(value["schemaVersion"], 2);
    assert_ne!(migrated, original);
    storage
        .remove_item(key)
        .expect("migration fixture must clean up");
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
    let rss = br#"<rss version="2.0"
      xmlns:p="https://podcastindex.org/namespace/1.0">
      <channel><title>Browser Feed</title><item>
      <title>Browser Episode</title><enclosure url="/audio.mp3" type="audio/mpeg" />
      <p:transcript url="/captions.srt" type="application/srt"
        language="en" rel="captions" />
    </item></channel></rss>"#;
    let feed = parse_podcast_feed(
        "https://media.example.test/feed.xml".to_string(),
        Uint8Array::from(rss.as_slice()),
    )
    .expect("browser feed must parse");
    let feed: PodcastFeedParseOutputDto =
        serde_wasm_bindgen::from_value(feed).expect("feed DTO must deserialize");
    assert_eq!(feed.episodes[0].title, "Browser Episode");
    assert_eq!(feed.episodes[0].transcript_resources.len(), 1);
    assert_eq!(
        feed.episodes[0].transcript_resources[0].format,
        Some(core_engine::TranscriptFormat::Srt)
    );
    assert_eq!(
        feed.episodes[0].transcript_resources[0].language.as_deref(),
        Some("en")
    );
    assert_eq!(
        feed.episodes[0].transcript_resources[0].relation.as_deref(),
        Some("captions")
    );

    let resource = TranscriptResourceDto {
        url: "https://media.example.test/captions.srt".to_string(),
        mime_type: "application/srt".to_string(),
        format: Some(core_engine::TranscriptFormat::Srt),
        language: Some("en".to_string()),
        relation: Some("captions".to_string()),
    };
    let resource = serde_wasm_bindgen::to_value(&resource).expect("resource must serialize");
    let transcript = parse_transcript(
        resource.clone(),
        "1\n00:00:00,000 --> 00:00:01,000\n🙂 (café),\nnaïve!".to_string(),
    )
    .expect("browser transcript must parse");
    let transcript: TranscriptDocumentDto =
        serde_wasm_bindgen::from_value(transcript).expect("transcript DTO must deserialize");
    let tokens: Vec<(&str, usize, usize)> = transcript.cues[0]
        .tokens
        .iter()
        .map(|token| (token.surface.as_str(), token.start_utf16, token.end_utf16))
        .collect();
    assert_eq!(transcript.cues[0].text, "🙂 (café),\nnaïve!");
    assert_eq!(tokens, [("café", 4, 8), ("naïve", 11, 16)]);

    let error = parse_transcript(resource, "1\n00:00:00,000 --> not-a-time\nText".to_string())
        .expect_err("malformed transcript must return an EnsubError");
    assert_eq!(
        Reflect::get(&error, &"name".into())
            .expect("error name must exist")
            .as_string()
            .as_deref(),
        Some("EnsubError")
    );
    assert_eq!(
        Reflect::get(&error, &"code".into())
            .expect("error code must exist")
            .as_string()
            .as_deref(),
        Some("transcript_invalid_timestamp")
    );
    assert_eq!(
        Reflect::get(&error, &"line".into())
            .expect("error line must exist")
            .as_f64(),
        Some(2.0)
    );
    assert_eq!(
        Reflect::get(&error, &"cueIndex".into())
            .expect("error cue index must exist")
            .as_f64(),
        Some(0.0)
    );

    let malformed_feed = br#"<rss><channel></rss>"#;
    let error = parse_podcast_feed(
        "https://media.example.test/feed.xml".to_string(),
        Uint8Array::from(malformed_feed.as_slice()),
    )
    .expect_err("malformed feed XML must return an EnsubError");
    assert_eq!(
        Reflect::get(&error, &"code".into())
            .expect("feed error code must exist")
            .as_string()
            .as_deref(),
        Some("feed_malformed_xml")
    );
    assert!(Reflect::get(&error, &"byteOffset".into())
        .expect("feed byte offset must exist")
        .as_f64()
        .is_some());
}
