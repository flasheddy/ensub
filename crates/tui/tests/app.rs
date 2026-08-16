use std::path::PathBuf;

use chrono::{DateTime, TimeZone, Utc};
use core_engine::{
    initial_review_state, CaptureResult, ContextRecord, ReviewCard, ReviewRating, ReviewUpdate,
    WordId, WordRecord,
};
use ensub_tui::{
    update, AppKey, CaptureFeedback, Document, DocumentFormat, Effect, InputEvent, InputKind,
    Message, Mode, Model, WordDetails,
};
use language_engine::{Definition, LexiconEntry};

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 8, 15, 10, 0, 0)
        .single()
        .expect("test timestamp must be valid")
}

fn input(key: AppKey) -> Message {
    Message::Input(InputEvent {
        key,
        kind: InputKind::Press,
        at: now(),
    })
}

fn model(text: &str, width: u16) -> Model {
    Model::with_document(
        Document::parse(
            PathBuf::from("/docs/article.txt"),
            DocumentFormat::PlainText,
            text,
        ),
        width,
        12,
    )
}

fn card(id: &str) -> ReviewCard {
    let word_id = WordId::new(id);
    ReviewCard {
        word: WordRecord {
            id: word_id.clone(),
            term: "immersion".to_string(),
            lemma: "immersion".to_string(),
            phonetic: "phonetic".to_string(),
            definition: "noun: deep involvement".to_string(),
            created_at: now(),
        },
        contexts: Vec::<ContextRecord>::new(),
        state: initial_review_state(word_id, now()),
    }
}

#[test]
fn vim_navigation_tracks_words_lines_and_gg_prefix() {
    let mut model = model("alpha beta gamma delta epsilon zeta eta theta", 20);

    let _ = update(&mut model, input(AppKey::Char('l')));
    assert_eq!(
        model.active_token().map(|token| token.surface.as_str()),
        Some("beta")
    );

    let _ = update(&mut model, input(AppKey::Char('j')));
    assert_eq!(
        model.active_token().map(|token| token.surface.as_str()),
        Some("epsilon")
    );

    let _ = update(&mut model, input(AppKey::Char('G')));
    assert_eq!(
        model.active_token().map(|token| token.surface.as_str()),
        Some("theta")
    );
    let _ = update(&mut model, input(AppKey::Char('g')));
    let _ = update(&mut model, input(AppKey::Char('g')));
    assert_eq!(
        model.active_token().map(|token| token.surface.as_str()),
        Some("alpha")
    );
}

#[test]
fn capture_key_emits_rendered_context_and_file_source() {
    let mut model = model("Reading immersion in context.", 40);
    let _ = update(&mut model, input(AppKey::Char('l')));

    let effects = update(&mut model, input(AppKey::Char('c')));

    assert_eq!(effects.len(), 1);
    let Effect::Capture {
        surface,
        context,
        source,
        captured_at,
    } = &effects[0]
    else {
        panic!("expected capture effect");
    };
    assert_eq!(surface, "immersion");
    assert_eq!(context, "Reading immersion in context.");
    assert_eq!(source, "tui:/docs/article.txt");
    assert_eq!(*captured_at, now());
}

#[test]
fn review_overlay_reveals_rates_and_returns_to_reader() {
    let mut model = model("Reading immersion.", 40);

    let effects = update(&mut model, input(AppKey::Char('r')));
    assert!(matches!(
        effects.as_slice(),
        [Effect::LoadDueReviews { .. }]
    ));
    assert_eq!(model.mode(), Mode::Review);

    let _ = update(
        &mut model,
        Message::DueReviewsLoaded(Ok(vec![card("word-1")])),
    );
    assert!(!model.review_revealed());
    let _ = update(&mut model, input(AppKey::Enter));
    assert!(model.review_revealed());

    let effects = update(&mut model, input(AppKey::Char('4')));
    let Effect::SaveReview {
        replacement,
        reviewed_at,
        ..
    } = &effects[0]
    else {
        panic!("expected save review effect");
    };
    assert_eq!(replacement.interval_days, 1);
    assert_eq!(
        replacement.last_rating,
        Some(ReviewRating::try_from(4).expect("valid rating"))
    );

    let _ = update(
        &mut model,
        Message::ReviewSaved {
            result: Ok(ReviewUpdate::Updated),
            replacement: replacement.clone(),
            reviewed_at: *reviewed_at,
        },
    );
    assert_eq!(model.mode(), Mode::Reader);
}

#[test]
fn escape_closes_overlays_before_quitting_reader() {
    let mut model = model("alpha beta", 40);

    let _ = update(&mut model, input(AppKey::Char('r')));
    let _ = update(&mut model, input(AppKey::Esc));
    assert_eq!(model.mode(), Mode::Reader);
    assert!(!model.should_quit());

    let _ = update(&mut model, input(AppKey::Esc));
    assert!(model.should_quit());
}

#[test]
fn open_path_mode_accepts_q_as_file_name_input() {
    let mut model = Model::empty(80, 20);

    let _ = update(&mut model, input(AppKey::Char('q')));
    let effects = update(&mut model, input(AppKey::Enter));

    assert_eq!(model.path_input(), "q");
    assert_eq!(effects, vec![Effect::OpenFile(PathBuf::from("q"))]);
    assert!(!model.should_quit());
}

#[test]
fn periodic_tick_rehydrates_visible_cached_review_states() {
    let mut model = model("alpha beta", 40);
    let word_id = WordId::new("alpha");
    let _ = update(
        &mut model,
        Message::CaptureFinished(Ok(CaptureFeedback {
            result: CaptureResult {
                word_created: true,
                contexts_created: 1,
            },
            details: WordDetails {
                surface: "alpha".to_string(),
                entry: Some(LexiconEntry {
                    lemma: "alpha".to_string(),
                    phonetic: "alpha".to_string(),
                    definitions: vec![Definition {
                        part_of_speech: "noun".to_string(),
                        text: "first".to_string(),
                    }],
                }),
                state: Some(initial_review_state(word_id, now())),
            },
            captured_at: now(),
        })),
    );

    let effects = update(&mut model, Message::Tick(now()));

    assert!(effects
        .iter()
        .any(|effect| matches!(effect, Effect::HydrateWords { surfaces, .. } if surfaces == &["alpha", "beta"])));
}

#[test]
fn review_conflict_keeps_cached_state_and_requests_rehydration() {
    let mut model = model("Reading immersion.", 40);
    let original = card("word-1");
    let _ = update(
        &mut model,
        Message::CaptureFinished(Ok(CaptureFeedback {
            result: CaptureResult {
                word_created: true,
                contexts_created: 1,
            },
            details: WordDetails {
                surface: "immersion".to_string(),
                entry: None,
                state: Some(original.state.clone()),
            },
            captured_at: now(),
        })),
    );
    let _ = update(&mut model, input(AppKey::Char('r')));
    let _ = update(
        &mut model,
        Message::DueReviewsLoaded(Ok(vec![original.clone()])),
    );
    let _ = update(&mut model, input(AppKey::Enter));
    let effects = update(&mut model, input(AppKey::Char('4')));
    let Effect::SaveReview { replacement, .. } = &effects[0] else {
        panic!("expected save review effect");
    };

    let effects = update(
        &mut model,
        Message::ReviewSaved {
            result: Ok(ReviewUpdate::Conflict),
            replacement: replacement.clone(),
            reviewed_at: now(),
        },
    );

    assert_eq!(
        model
            .word_details("immersion")
            .and_then(|details| details.state.as_ref()),
        Some(&original.state)
    );
    assert!(effects.iter().any(|effect| {
        matches!(effect, Effect::HydrateWords { surfaces, .. } if surfaces == &["immersion"])
    }));
    assert!(effects
        .iter()
        .any(|effect| matches!(effect, Effect::RefreshDueCount { .. })));
}

#[test]
fn cursor_at_document_end_stays_inside_rendered_reader_height() {
    let text = vec!["alpha"; 20].join("\n");
    let mut model = model(&text, 40);

    let _ = update(&mut model, input(AppKey::Char('G')));

    let cursor_line = model
        .active_token_index()
        .and_then(|index| model.layout().placement(index))
        .map(|placement| placement.line)
        .expect("active token must have a placement");
    assert!(cursor_line >= model.viewport_top());
    assert!(cursor_line < model.viewport_top().saturating_add(9));
}

#[test]
fn resize_preserves_hidden_sidebar_preference() {
    let mut model = model("alpha beta", 120);
    let _ = update(&mut model, input(AppKey::Tab));
    assert!(!model.panel_visible());

    let _ = update(
        &mut model,
        Message::Resize {
            width: 121,
            height: 20,
        },
    );

    assert!(!model.panel_visible());
}

#[test]
fn closing_narrow_overlay_after_wide_resize_reflows_for_sidebar() {
    let mut model = model("alpha beta gamma delta", 72);
    let _ = update(&mut model, input(AppKey::Tab));
    assert_eq!(model.mode(), Mode::Vocabulary);

    let _ = update(
        &mut model,
        Message::Resize {
            width: 120,
            height: 20,
        },
    );
    let _ = update(&mut model, input(AppKey::Esc));

    assert_eq!(model.mode(), Mode::Reader);
    assert!(model.panel_visible());
    assert_eq!(model.layout().width(), 83);
}
