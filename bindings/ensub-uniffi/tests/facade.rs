use ensub_uniffi::{BindingError, TranscriptSession};

const SOURCE_URL: &str = "https://fixture.ensub.invalid/demo-fixture.json";
const FIXTURE: &[u8] = include_bytes!("../../../crates/web_player/assets/demo-fixture.json");

fn session() -> TranscriptSession {
    TranscriptSession::from_fixture(SOURCE_URL.to_string(), FIXTURE.to_vec())
        .expect("the committed demo fixture must parse")
}

#[test]
fn fixture_projects_episode_and_twelve_stable_cues() {
    let session = session();

    assert_eq!(session.episode().feed_title, "Synthetic Signal");
    assert_eq!(
        session.episode().episode_title,
        "The Shape of a Listening Habit"
    );
    assert_eq!(session.episode().duration_ms, 120_000);
    let cues = session.cues();
    assert_eq!(cues.len(), 12);
    assert_eq!(cues[0].index, 0);
    assert_eq!(cues[0].id, "cue-0");
    assert_eq!(cues[0].source_cue_id.as_deref(), Some("listening-begins"));
    assert_eq!(cues[11].end_ms, 120_000);
}

#[test]
fn overlap_returns_every_active_cue_in_source_order() {
    let sync = session()
        .sync_at(29_500)
        .expect("position must synchronize");

    assert_eq!(sync.active_cue_indices, [2, 3]);
    assert_eq!(sync.anchor_cue_index, Some(2));
    assert_eq!(sync.preceding_cue_index, None);
}

#[test]
fn gap_returns_the_most_recent_completed_cue() {
    let sync = session()
        .sync_at(40_000)
        .expect("position must synchronize");

    assert!(sync.active_cue_indices.is_empty());
    assert_eq!(sync.anchor_cue_index, None);
    assert_eq!(sync.preceding_cue_index, Some(3));
}

#[test]
fn negative_playback_position_is_rejected() {
    let error = session()
        .sync_at(-1)
        .expect_err("negative positions must not cross into the unsigned core");

    assert!(matches!(error, BindingError::InvalidPlaybackPosition));
}
