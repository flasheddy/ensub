use ensub_wasm::{
    PlayerWorkspace, PlayerWorkspaceError, PreparePodcastCaptureInput, TranscriptStateDto,
    MAX_PLAYER_FIXTURE_BYTES, PLAYER_CACHE_FORMAT, PLAYER_CACHE_SCHEMA_VERSION,
};
use serde_json::json;

const FEED_URL: &str = "https://media.example.test/feed.xml";
const TRANSCRIPT_URL: &str = "https://media.example.test/captions.vtt";

fn feed() -> Vec<u8> {
    format!(
        r#"<rss version="2.0"><channel><title>Workspace Feed</title><item>
        <guid>episode-guid</guid><title>Workspace Episode</title>
        <enclosure url="https://media.example.test/audio.mp3" type="audio/mpeg" />
        <podcast:transcript xmlns:podcast="https://podcastindex.org/namespace/1.0"
          url="{TRANSCRIPT_URL}" type="text/vtt" language="en" />
        </item></channel></rss>"#
    )
    .into_bytes()
}

fn transcript() -> &'static str {
    "WEBVTT\n\n00:01.000 --> 00:03.000\nFirst cue\n\n00:02.000 --> 00:04.000\nOverlap cue\n\n00:05.000 --> 00:06.000\nFinal cue"
}

fn nested_overlap_transcript() -> &'static str {
    "WEBVTT\n\n00:01.000 --> 00:04.500\nLong cue\n\n00:01.000 --> 00:01.200\nSame-start cue\n\n00:02.000 --> 00:04.000\nNested cue\n\n00:03.000 --> 00:03.500\nInner cue\n\n00:05.000 --> 00:06.000\nFinal cue"
}

fn workspace_with_transcript(transcript: &str) -> PlayerWorkspace {
    let mut workspace = PlayerWorkspace::open(&[]).expect("empty cache must open");
    let imported = workspace
        .import_feed(FEED_URL, &feed(), 1)
        .expect("feed must import");
    let episode_id = imported.episodes[0].identity.internal_id.clone();
    workspace
        .select_episode(&episode_id)
        .expect("episode must select");
    workspace
        .cache_transcript(&episode_id, TRANSCRIPT_URL, transcript, 2)
        .expect("transcript must cache");
    workspace
}

fn demo_fixture() -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schema_version": 1,
        "feed": {
            "title": "Demo Feed",
            "language": "en",
            "description": "A synthetic player fixture.",
            "artwork_url": "/images/feed.png"
        },
        "episode": {
            "publisher_guid": "demo-episode",
            "title": "Demo Episode",
            "published_at": "2026-08-17T08:00:00Z",
            "language": "en",
            "enclosure_url": "/audio/demo.mp3",
            "enclosure_mime_type": "audio/mpeg",
            "duration_ms": 6_000,
            "artwork_url": "/images/episode.png"
        },
        "transcript": {
            "url": "/captions/demo.vtt#english",
            "mime_type": "text/vtt",
            "format": "web_vtt",
            "language": "en",
            "relation": "captions",
            "cues": [
                {
                    "source_cue_id": "first",
                    "start_ms": 1_000,
                    "end_ms": 3_000,
                    "text": "First cue."
                },
                {
                    "source_cue_id": "overlap",
                    "start_ms": 2_000,
                    "end_ms": 4_000,
                    "text": "Overlap cue."
                },
                {
                    "source_cue_id": "last",
                    "start_ms": 5_000,
                    "end_ms": 6_000,
                    "text": "Final cue."
                }
            ]
        }
    }))
    .expect("demo fixture must serialize")
}

const DEMO_SOURCE_URL: &str = "https://media.example.test/fixtures/demo.json";

#[test]
fn workspace_import_select_cache_sync_and_restore_round_trip() {
    let mut workspace = PlayerWorkspace::open(&[]).expect("empty cache must open");
    let imported = workspace
        .import_feed(FEED_URL, &feed(), 1_787_000_000_000)
        .expect("feed must import");
    assert_eq!(imported.revision, 1);
    assert_eq!(imported.feeds.len(), 1);
    assert_eq!(imported.episodes.len(), 1);

    let episode_id = imported.episodes[0].identity.internal_id.clone();
    let opened = workspace
        .select_episode(&episode_id)
        .expect("episode must select");
    assert_eq!(opened.transcript_state, TranscriptStateDto::Loading);
    assert_eq!(
        opened.selected_transcript_url.as_deref(),
        Some(TRANSCRIPT_URL)
    );

    let cached = workspace
        .cache_transcript(&episode_id, TRANSCRIPT_URL, transcript(), 1_787_000_000_100)
        .expect("transcript must cache");
    assert_eq!(cached.transcript_state, TranscriptStateDto::Ready);
    assert_eq!(
        cached.transcript.as_ref().map(|value| value.cues.len()),
        Some(3)
    );

    let overlap = workspace.sync_at(2_500).expect("sync must resolve");
    assert_eq!(overlap.active_cue_indices, [0, 1]);
    assert_eq!(overlap.anchor_cue_index, Some(0));
    assert_eq!(overlap.preceding_cue_index, None);

    let gap = workspace.sync_at(4_500).expect("gap must resolve");
    assert!(gap.active_cue_indices.is_empty());
    assert_eq!(gap.anchor_cue_index, None);
    assert_eq!(gap.preceding_cue_index, Some(1));

    let snapshot = workspace.snapshot().expect("cache must encode");
    let restored = PlayerWorkspace::open(&snapshot).expect("cache must restore");
    assert_eq!(restored.view().revision, workspace.view().revision);
    assert_eq!(
        restored
            .sync_at(5_000)
            .expect("restored index must resolve")
            .active_cue_indices,
        [2]
    );
}

#[test]
fn failed_mutations_leave_the_previous_workspace_unchanged() {
    let mut workspace = PlayerWorkspace::open(&[]).expect("empty cache must open");
    workspace
        .import_feed(FEED_URL, &feed(), 1)
        .expect("feed must import");
    let before = workspace.snapshot().expect("cache must encode");

    let error = workspace
        .import_feed(FEED_URL, b"not xml", 2)
        .expect_err("malformed refresh must fail");
    assert!(matches!(error, PlayerWorkspaceError::Ingestion(_)));
    assert_eq!(workspace.snapshot().expect("cache must encode"), before);
}

#[test]
fn workspace_enforces_input_and_snapshot_limits() {
    let mut workspace = PlayerWorkspace::open(&[]).expect("empty cache must open");
    let oversized_feed = vec![b'x'; ensub_wasm::MAX_PLAYER_FEED_BYTES + 1];
    assert!(matches!(
        workspace.import_feed(FEED_URL, &oversized_feed, 1),
        Err(PlayerWorkspaceError::FeedTooLarge)
    ));
    let oversized_cache = vec![0; ensub_wasm::MAX_PLAYER_CACHE_BYTES + 1];
    assert!(matches!(
        PlayerWorkspace::open(&oversized_cache),
        Err(PlayerWorkspaceError::CacheTooLarge)
    ));
}

#[test]
fn workspace_prepares_cross_cue_provenance_and_rejects_stale_selection() {
    let mut workspace = PlayerWorkspace::open(&[]).expect("empty cache must open");
    let imported = workspace
        .import_feed(FEED_URL, &feed(), 1)
        .expect("feed must import");
    let episode_id = imported.episodes[0].identity.internal_id.clone();
    workspace
        .select_episode(&episode_id)
        .expect("episode must select");
    workspace
        .cache_transcript(
            &episode_id,
            TRANSCRIPT_URL,
            "WEBVTT\n\n00:01.000 --> 00:02.000\nWe went\n\n00:02.000 --> 00:03.000\nhome.",
            2,
        )
        .expect("transcript must cache");
    let revision = workspace.view().revision;
    let prepared = workspace
        .prepare_podcast_capture(&PreparePodcastCaptureInput {
            revision,
            episode_id: episode_id.clone(),
            transcript_url: TRANSCRIPT_URL.to_string(),
            cue_id: "cue-0".to_string(),
            token_index: 1,
            playback_position_ms: 1_500,
            duration_ms: Some(2_750),
        })
        .expect("selection must prepare");

    assert_eq!(prepared.surface, "went");
    assert_eq!(prepared.draft.sentence, "We went home.");
    assert_eq!(prepared.draft.cue_range.first_cue_id(), "cue-0");
    assert_eq!(prepared.draft.cue_range.last_cue_id(), "cue-1");
    assert_eq!(prepared.draft.audio_slice.start_ms(), 500);
    assert_eq!(prepared.draft.audio_slice.end_ms(), 2_750);

    assert!(matches!(
        workspace.prepare_podcast_capture(&PreparePodcastCaptureInput {
            revision: revision - 1,
            episode_id,
            transcript_url: TRANSCRIPT_URL.to_string(),
            cue_id: "cue-0".to_string(),
            token_index: 1,
            playback_position_ms: 1_500,
            duration_ms: None,
        }),
        Err(PlayerWorkspaceError::StaleSelection)
    ));
}

#[test]
fn empty_workspace_imports_demo_fixture_as_a_ready_episode() {
    let mut workspace = PlayerWorkspace::open(&[]).expect("empty cache must open");
    let opened = workspace
        .import_demo_fixture(DEMO_SOURCE_URL, &demo_fixture(), 1_787_000_000_000)
        .expect("demo fixture must import");

    assert_eq!(opened.revision, 1);
    assert_eq!(opened.episode.title, "Demo Episode");
    assert_eq!(
        opened.selected_transcript_url.as_deref(),
        Some("https://media.example.test/captions/demo.vtt#english")
    );
    assert_eq!(opened.transcript_state, TranscriptStateDto::Ready);
    assert_eq!(
        opened
            .transcript
            .as_ref()
            .map(|document| document.cues.len()),
        Some(3)
    );

    let view = workspace.view();
    assert_eq!(view.revision, 1);
    assert_eq!(view.feeds.len(), 1);
    assert_eq!(view.episodes.len(), 1);
    assert_eq!(
        view.selected_episode_id.as_deref(),
        Some(opened.episode.identity.internal_id.as_str())
    );
    assert_eq!(view.last_transcript_language.as_deref(), Some("en"));
}

#[test]
fn imported_demo_fixture_restores_from_the_unchanged_v1_snapshot_schema() {
    let mut workspace = PlayerWorkspace::open(&[]).expect("empty cache must open");
    let opened = workspace
        .import_demo_fixture(DEMO_SOURCE_URL, &demo_fixture(), 17)
        .expect("demo fixture must import");
    let snapshot = workspace.snapshot().expect("cache must encode");
    let json: serde_json::Value =
        serde_json::from_slice(&snapshot).expect("snapshot must remain JSON");
    assert_eq!(json["format"], PLAYER_CACHE_FORMAT);
    assert_eq!(json["schema_version"], PLAYER_CACHE_SCHEMA_VERSION);

    let restored = PlayerWorkspace::open(&snapshot).expect("v1 cache must restore");
    assert_eq!(restored.view().revision, 1);
    assert_eq!(
        restored
            .sync_at(2_500)
            .expect("restored transcript must sync")
            .active_cue_indices,
        [0, 1]
    );
    assert_eq!(
        restored.view().selected_episode_id,
        Some(opened.episode.identity.internal_id)
    );
}

#[test]
fn demo_import_rejects_nonempty_and_oversized_workspaces_without_mutation() {
    let mut nonempty = PlayerWorkspace::open(&[]).expect("empty cache must open");
    nonempty
        .import_feed(FEED_URL, &feed(), 1)
        .expect("regular feed must import");
    let before = nonempty.snapshot().expect("cache must encode");
    let error = nonempty
        .import_demo_fixture(DEMO_SOURCE_URL, &demo_fixture(), 2)
        .expect_err("nonempty workspace must reject demo import");
    assert!(matches!(&error, PlayerWorkspaceError::WorkspaceNotEmpty));
    assert_eq!(error.code(), "player_workspace_not_empty");
    assert_eq!(nonempty.snapshot().expect("cache must encode"), before);

    let mut empty = PlayerWorkspace::open(&[]).expect("empty cache must open");
    let before = empty.snapshot().expect("cache must encode");
    let oversized = vec![b'x'; MAX_PLAYER_FIXTURE_BYTES + 1];
    let error = empty
        .import_demo_fixture(DEMO_SOURCE_URL, &oversized, 2)
        .expect_err("oversized fixture must fail");
    assert!(matches!(&error, PlayerWorkspaceError::FixtureTooLarge));
    assert_eq!(error.code(), "player_fixture_too_large");
    assert_eq!(empty.snapshot().expect("cache must encode"), before);
}

#[test]
fn demo_import_rolls_back_parse_validation_and_late_revision_failures() {
    let mut workspace = PlayerWorkspace::open(&[]).expect("empty cache must open");
    let before = workspace.snapshot().expect("cache must encode");
    for (case, source_url, bytes) in [
        ("invalid JSON", DEMO_SOURCE_URL, b"{".to_vec()),
        ("invalid source URL", "demo.json", demo_fixture()),
    ] {
        let error = workspace
            .import_demo_fixture(source_url, &bytes, 1)
            .expect_err(case);
        assert!(
            matches!(&error, PlayerWorkspaceError::Ingestion(_)),
            "{case}: {error:?}"
        );
        assert_eq!(
            workspace.snapshot().expect("cache must encode"),
            before,
            "{case}"
        );
    }

    let max_revision_snapshot = serde_json::to_vec(&json!({
        "format": PLAYER_CACHE_FORMAT,
        "schema_version": PLAYER_CACHE_SCHEMA_VERSION,
        "cache": {
            "revision": u64::MAX,
            "feeds": {},
            "transcripts": {},
            "selected_feed_url": null,
            "selected_episode_id": null,
            "selected_transcript_url": null,
            "last_transcript_language": null
        }
    }))
    .expect("revision fixture must encode");
    let mut exhausted =
        PlayerWorkspace::open(&max_revision_snapshot).expect("revision fixture must open");
    let before = exhausted.snapshot().expect("cache must encode");
    let error = exhausted
        .import_demo_fixture(DEMO_SOURCE_URL, &demo_fixture(), 1)
        .expect_err("revision overflow must fail late");
    assert!(matches!(&error, PlayerWorkspaceError::RevisionOverflow));
    assert_eq!(exhausted.snapshot().expect("cache must encode"), before);
}

#[test]
fn adjacent_cue_queries_cover_before_active_overlap_gap_boundaries_and_after() {
    let mut workspace = PlayerWorkspace::open(&[]).expect("empty cache must open");
    workspace
        .import_demo_fixture(DEMO_SOURCE_URL, &demo_fixture(), 1)
        .expect("demo fixture must import");
    let before_queries = workspace.snapshot().expect("cache must encode");

    let next = |position| {
        workspace
            .next_cue_at(position)
            .expect("next cue query must succeed")
            .map(|cue| (cue.cue_index, cue.start_ms))
    };
    let previous = |position| {
        workspace
            .previous_cue_at(position)
            .expect("previous cue query must succeed")
            .map(|cue| (cue.cue_index, cue.start_ms))
    };

    assert_eq!(next(0), Some((0, 1_000)));
    assert_eq!(previous(0), None);
    assert_eq!(next(1_500), Some((1, 2_000)));
    assert_eq!(previous(1_500), Some((0, 1_000)));
    assert_eq!(next(2_500), Some((2, 5_000)));
    assert_eq!(previous(2_500), Some((1, 2_000)));
    assert_eq!(next(3_000), Some((2, 5_000)));
    assert_eq!(previous(3_000), Some((1, 2_000)));
    assert_eq!(next(4_500), Some((2, 5_000)));
    assert_eq!(previous(4_500), Some((1, 2_000)));
    assert_eq!(next(5_500), None);
    assert_eq!(previous(5_500), Some((2, 5_000)));
    assert_eq!(next(6_000), None);
    assert_eq!(previous(6_000), Some((2, 5_000)));
    assert_eq!(
        workspace.snapshot().expect("cache must encode"),
        before_queries
    );
}

#[test]
fn cue_navigation_uses_strict_distinct_start_groups() {
    let workspace = workspace_with_transcript(nested_overlap_transcript());
    let next = |position| {
        workspace
            .next_cue_at(position)
            .expect("next cue query must succeed")
            .map(|cue| (cue.cue_index, cue.start_ms))
    };
    let previous = |position| {
        workspace
            .previous_cue_at(position)
            .expect("previous cue query must succeed")
            .map(|cue| (cue.cue_index, cue.start_ms))
    };

    let cases = [
        (0, Some((0, 1_000)), None),
        (999, Some((0, 1_000)), None),
        (1_000, Some((2, 2_000)), None),
        (1_200, Some((2, 2_000)), Some((0, 1_000))),
        (1_500, Some((2, 2_000)), Some((0, 1_000))),
        (2_000, Some((3, 3_000)), Some((0, 1_000))),
        (2_500, Some((3, 3_000)), Some((2, 2_000))),
        (3_000, Some((4, 5_000)), Some((2, 2_000))),
        (3_500, Some((4, 5_000)), Some((3, 3_000))),
        (4_000, Some((4, 5_000)), Some((3, 3_000))),
        (4_500, Some((4, 5_000)), Some((3, 3_000))),
        (5_000, None, Some((3, 3_000))),
        (6_000, None, Some((4, 5_000))),
        (u64::MAX, None, Some((4, 5_000))),
    ];

    for (position, expected_next, expected_previous) in cases {
        assert_eq!(next(position), expected_next, "next at {position} ms");
        assert_eq!(
            previous(position),
            expected_previous,
            "previous at {position} ms"
        );
    }
}

#[test]
fn repeated_cue_navigation_advances_between_distinct_start_groups() {
    let workspace = workspace_with_transcript(nested_overlap_transcript());
    let next = |position| {
        workspace
            .next_cue_at(position)
            .expect("next cue query must succeed")
            .map(|cue| (cue.cue_index, cue.start_ms))
    };
    let previous = |position| {
        workspace
            .previous_cue_at(position)
            .expect("previous cue query must succeed")
            .map(|cue| (cue.cue_index, cue.start_ms))
    };

    let mut position = 0;
    let mut forward = Vec::new();
    for _ in 0..4 {
        let target = next(position).expect("a later cue group must exist");
        forward.push(target);
        position = target.1;
    }
    assert_eq!(forward, [(0, 1_000), (2, 2_000), (3, 3_000), (4, 5_000)]);
    assert_eq!(next(position), None);

    let mut position = 6_000;
    let mut backward = Vec::new();
    for _ in 0..4 {
        let target = previous(position).expect("an earlier cue group must exist");
        backward.push(target);
        position = target.1;
    }
    assert_eq!(backward, [(4, 5_000), (3, 3_000), (2, 2_000), (0, 1_000)]);
    assert_eq!(previous(position), None);
}
