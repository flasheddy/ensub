use ensub_wasm::{PlayerWorkspace, PlayerWorkspaceError, TranscriptStateDto};

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
