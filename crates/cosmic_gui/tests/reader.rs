use std::path::PathBuf;

use chrono::{TimeDelta, TimeZone, Utc};
use core_engine::{initial_review_state, CaptureResult, ReviewRating, WordId};
use ensub_gui::{
    build_block_runs, reader_badge, reader_uses_split_layout, update_reader, GlobalShortcut,
    KeyEventKind, Page, ReaderBadge, ReaderEffect, ReaderKey, ReaderMessage, ReaderModel,
    ReaderShortcut, ReaderWordDetails,
};
use language_engine::{Definition, Document, DocumentFormat, LexiconEntry};

fn now() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 8, 15, 12, 0, 0)
        .single()
        .expect("test timestamp must be valid")
}

fn document(source: &str) -> Document {
    Document::parse(PathBuf::from("reader.md"), DocumentFormat::Markdown, source)
}

fn details(surface: &str) -> ReaderWordDetails {
    ReaderWordDetails {
        surface: surface.to_string(),
        entry: Some(LexiconEntry {
            lemma: surface.to_lowercase(),
            phonetic: "test".to_string(),
            definitions: vec![Definition {
                part_of_speech: "noun".to_string(),
                text: "A test definition".to_string(),
            }],
        }),
        state: None,
    }
}

#[test]
fn cursor_moves_and_clamps_at_document_boundaries() {
    let mut model = ReaderModel::default();
    let effects = update_reader(
        &mut model,
        ReaderMessage::DocumentOpened {
            generation: 0,
            result: Ok(Some(document("One two three."))),
        },
    );
    assert_eq!(model.cursor, Some(0));
    assert_eq!(effects.len(), 1);

    let _ = update_reader(&mut model, ReaderMessage::MovePrevious);
    assert_eq!(model.cursor, Some(0));
    let _ = update_reader(&mut model, ReaderMessage::MoveNext);
    let _ = update_reader(&mut model, ReaderMessage::MoveNext);
    let _ = update_reader(&mut model, ReaderMessage::MoveNext);
    assert_eq!(model.cursor, Some(2));

    let _ = update_reader(&mut model, ReaderMessage::SelectToken(99));
    assert_eq!(model.cursor, Some(2));
}

#[test]
fn empty_and_tokenless_documents_have_no_cursor() {
    for source in ["", "```rust\nlet value = 1;\n```"] {
        let mut model = ReaderModel::default();
        let _ = update_reader(
            &mut model,
            ReaderMessage::DocumentOpened {
                generation: 0,
                result: Ok(Some(document(source))),
            },
        );
        assert_eq!(model.cursor, None);
    }
}

#[test]
fn review_badges_distinguish_new_captured_due_and_future_cards() {
    let as_of = now();
    assert_eq!(reader_badge(None, as_of), ReaderBadge::New);

    let mut state = initial_review_state(WordId::new("word"), as_of);
    assert_eq!(reader_badge(Some(&state), as_of), ReaderBadge::Captured);

    state.last_rating = Some(ReviewRating::try_from(4).expect("valid rating"));
    assert_eq!(reader_badge(Some(&state), as_of), ReaderBadge::DueNow);

    state.next_review_at = as_of + TimeDelta::hours(25);
    assert_eq!(reader_badge(Some(&state), as_of), ReaderBadge::DueInDays(2));
}

#[test]
fn block_runs_reconstruct_source_and_map_only_word_segments() {
    let document = document("Read **deeply**, then _pause_.");
    let block = &document.blocks()[0];
    let runs = build_block_runs(&document, 0);

    assert_eq!(
        runs.iter().map(|run| run.text.as_str()).collect::<String>(),
        block.text
    );
    assert_eq!(
        runs.iter()
            .filter_map(|run| run.token_index.map(|index| (run.text.as_str(), index)))
            .collect::<Vec<_>>(),
        vec![("Read", 0), ("deeply", 1), ("then", 2), ("pause", 3)]
    );
    assert!(runs
        .iter()
        .any(|run| run.text == "deeply" && run.style.bold));
    assert!(runs
        .iter()
        .any(|run| run.text == "pause" && run.style.italic));
    assert!(runs
        .iter()
        .filter(|run| run.text.chars().all(|character| !character.is_alphabetic()))
        .all(|run| run.token_index.is_none()));
}

#[test]
fn stale_open_and_hydration_results_do_not_replace_active_reader_state() {
    let mut model = ReaderModel::default();
    let first = update_reader(&mut model, ReaderMessage::OpenRequested);
    let second = update_reader(&mut model, ReaderMessage::OpenRequested);
    assert_eq!(first, vec![ReaderEffect::PickDocument { generation: 1 }]);
    assert_eq!(second, vec![ReaderEffect::PickDocument { generation: 2 }]);

    let _ = update_reader(
        &mut model,
        ReaderMessage::DocumentOpened {
            generation: 1,
            result: Ok(Some(document("Stale file."))),
        },
    );
    assert!(model.document.is_none());

    let effects = update_reader(
        &mut model,
        ReaderMessage::DocumentOpened {
            generation: 2,
            result: Ok(Some(document("Fresh words."))),
        },
    );
    let hydration_generation = match effects.as_slice() {
        [ReaderEffect::HydrateWord { generation, .. }] => *generation,
        other => panic!("expected hydration effect, got {other:?}"),
    };
    let _ = update_reader(&mut model, ReaderMessage::MoveNext);
    let _ = update_reader(
        &mut model,
        ReaderMessage::WordHydrated {
            generation: hydration_generation,
            cache_key: "fresh".to_string(),
            result: Ok(details("Fresh")),
        },
    );
    assert_ne!(
        model.details.as_ref().map(|value| value.surface.as_str()),
        Some("Fresh")
    );
    assert!(model.details_cache.contains_key("fresh"));
}

#[test]
fn capture_is_suppressed_while_a_request_is_in_flight_and_uses_its_own_feedback() {
    let mut model = ReaderModel::default();
    let effects = update_reader(
        &mut model,
        ReaderMessage::DocumentOpened {
            generation: 0,
            result: Ok(Some(document("Capture this word."))),
        },
    );
    let hydration_generation = match effects.as_slice() {
        [ReaderEffect::HydrateWord { generation, .. }] => *generation,
        other => panic!("expected hydration effect, got {other:?}"),
    };
    let _ = update_reader(
        &mut model,
        ReaderMessage::WordHydrated {
            generation: hydration_generation,
            cache_key: "capture".to_string(),
            result: Ok(details("Capture")),
        },
    );

    let first = update_reader(
        &mut model,
        ReaderMessage::CaptureRequested { captured_at: now() },
    );
    assert!(matches!(
        first.as_slice(),
        [ReaderEffect::CaptureWord { .. }]
    ));
    assert!(update_reader(
        &mut model,
        ReaderMessage::CaptureRequested { captured_at: now() }
    )
    .is_empty());

    let generation = model.capture_generation;
    let state = initial_review_state(WordId::new("capture"), now());
    let _ = update_reader(
        &mut model,
        ReaderMessage::CaptureFinished {
            generation,
            cache_key: "capture".to_string(),
            lemma: "capture".to_string(),
            result: Ok((
                CaptureResult {
                    word_created: false,
                    contexts_created: 1,
                },
                state,
            )),
        },
    );
    assert_eq!(model.feedback.as_deref(), Some("Added context for capture"));
    assert!(!model.capturing);
}

#[test]
fn keyboard_mapping_filters_modifiers_capture_and_repeat() {
    for key in [
        ReaderKey::Character("w".to_string()),
        ReaderKey::Character("l".to_string()),
        ReaderKey::NamedRight,
    ] {
        assert_eq!(
            ReaderShortcut::from_key(key, KeyEventKind::Pressed),
            Some(ReaderShortcut::MoveNext)
        );
    }
    for key in [
        ReaderKey::Character("b".to_string()),
        ReaderKey::Character("h".to_string()),
        ReaderKey::NamedLeft,
    ] {
        assert_eq!(
            ReaderShortcut::from_key(key, KeyEventKind::Pressed),
            Some(ReaderShortcut::MovePrevious)
        );
    }
    for (key, shortcut) in [
        (
            ReaderKey::Character("j".to_string()),
            ReaderShortcut::ScrollDown,
        ),
        (ReaderKey::NamedDown, ReaderShortcut::ScrollDown),
        (
            ReaderKey::Character("k".to_string()),
            ReaderShortcut::ScrollUp,
        ),
        (ReaderKey::NamedUp, ReaderShortcut::ScrollUp),
        (
            ReaderKey::Character("c".to_string()),
            ReaderShortcut::Capture,
        ),
        (ReaderKey::Enter, ReaderShortcut::Capture),
        (ReaderKey::Character("o".to_string()), ReaderShortcut::Open),
    ] {
        assert_eq!(
            ReaderShortcut::from_key(key, KeyEventKind::Pressed),
            Some(shortcut)
        );
    }
    assert_eq!(
        ReaderShortcut::from_key(ReaderKey::NamedRight, KeyEventKind::Repeated),
        Some(ReaderShortcut::MoveNext)
    );
    assert_eq!(
        ReaderShortcut::from_key(
            ReaderKey::Character("c".to_string()),
            KeyEventKind::Repeated
        ),
        None
    );
    assert_eq!(
        ReaderShortcut::from_key(
            ReaderKey::Character("o".to_string()),
            KeyEventKind::Captured
        ),
        None
    );
    assert_eq!(
        ReaderShortcut::from_key(
            ReaderKey::ModifiedCharacter("l".to_string()),
            KeyEventKind::Pressed
        ),
        None
    );
    assert_eq!(
        GlobalShortcut::from_key(ReaderKey::ShiftTab, KeyEventKind::Pressed),
        Some(GlobalShortcut::PreviousPage)
    );
    assert_eq!(
        GlobalShortcut::from_key(ReaderKey::Character("5".to_string()), KeyEventKind::Pressed),
        Some(GlobalShortcut::Navigate(Page::Review))
    );
}

#[test]
fn global_navigation_shortcuts_ignore_text_input_and_repeat_events() {
    assert_eq!(
        GlobalShortcut::from_key(ReaderKey::Tab, KeyEventKind::Pressed),
        Some(GlobalShortcut::NextPage)
    );
    assert_eq!(
        GlobalShortcut::from_key(ReaderKey::Escape, KeyEventKind::Pressed),
        Some(GlobalShortcut::ReleaseFocus)
    );
    assert_eq!(
        GlobalShortcut::from_key(ReaderKey::Tab, KeyEventKind::Captured),
        None
    );
    assert_eq!(
        GlobalShortcut::from_key(
            ReaderKey::Character("3".to_string()),
            KeyEventKind::Captured
        ),
        None
    );
    assert_eq!(
        GlobalShortcut::from_key(ReaderKey::Tab, KeyEventKind::Repeated),
        None
    );
    for (number, page) in [
        ("1", Page::Dashboard),
        ("2", Page::Library),
        ("3", Page::Reader),
        ("4", Page::ParseText),
        ("5", Page::Review),
    ] {
        assert_eq!(
            GlobalShortcut::from_key(
                ReaderKey::Character(number.to_string()),
                KeyEventKind::Pressed
            ),
            Some(GlobalShortcut::Navigate(page))
        );
    }
}

#[test]
fn reader_shortcuts_do_not_claim_global_navigation_keys() {
    assert_eq!(
        ReaderShortcut::from_key(ReaderKey::Tab, KeyEventKind::Pressed),
        None
    );
    assert_eq!(
        ReaderShortcut::from_key(ReaderKey::Character("1".to_string()), KeyEventKind::Pressed),
        None
    );
    assert_eq!(
        ReaderShortcut::from_key(ReaderKey::Escape, KeyEventKind::Pressed),
        None
    );
}

#[test]
fn reader_split_layout_stacks_only_below_750_pixels() {
    assert!(!reader_uses_split_layout(749.0));
    assert!(reader_uses_split_layout(750.0));
    assert!(reader_uses_split_layout(1180.0));
}
