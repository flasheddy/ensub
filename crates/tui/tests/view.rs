use std::path::PathBuf;

use chrono::{DateTime, TimeZone, Utc};
use core_engine::{initial_review_state, CaptureResult, ReviewCard, WordId, WordRecord};
use ensub_tui::{
    render, update, AppKey, CaptureFeedback, Document, DocumentFormat, InputEvent, InputKind,
    Message, Model, WordDetails,
};
use language_engine::{Definition, LexiconEntry};
use ratatui::{backend::TestBackend, Terminal};

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

fn reader(width: u16, height: u16) -> Model {
    let mut model = Model::with_document(
        Document::parse(
            PathBuf::from("/docs/article.md"),
            DocumentFormat::Markdown,
            "# Immersion\n\nReading immersion builds fluency.",
        ),
        width,
        height,
    );
    let word_id = WordId::new("word-immersion");
    let _ = update(
        &mut model,
        Message::CaptureFinished(Ok(CaptureFeedback {
            result: CaptureResult {
                word_created: true,
                contexts_created: 1,
            },
            details: WordDetails {
                surface: "immersion".to_string(),
                entry: Some(LexiconEntry {
                    lemma: "immersion".to_string(),
                    phonetic: "ih-MER-zhuhn".to_string(),
                    definitions: vec![Definition {
                        part_of_speech: "noun".to_string(),
                        text: "deep involvement in an activity".to_string(),
                    }],
                }),
                state: Some(initial_review_state(word_id, now())),
            },
            captured_at: now(),
        })),
    );
    let _ = update(&mut model, Message::DueCountLoaded(Ok(7)));
    model
}

fn draw(model: &Model, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal must create");
    terminal
        .draw(|frame| render(frame, model))
        .expect("test view must render");
    let buffer = terminal.backend().buffer();
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn wide_reader_shows_vocabulary_progress_and_due_count() {
    let model = reader(120, 26);

    let output = draw(&model, 120, 26);

    assert!(output.contains("Immersion"));
    assert!(output.contains("Vocabulary"));
    assert!(output.contains("ih-MER-zhuhn"));
    assert!(output.contains("Interval: 0 days"));
    assert!(output.contains("Due: 7"));
}

#[test]
fn narrow_reader_opens_vocabulary_as_an_overlay() {
    let mut model = reader(72, 22);

    let _ = update(&mut model, input(AppKey::Tab));
    let output = draw(&model, 72, 22);

    assert!(output.contains("Vocabulary"));
    assert!(output.contains("deep involvement in an activity"));
}

#[test]
fn review_definition_is_hidden_until_reveal() {
    let mut model = reader(90, 24);
    let word_id = WordId::new("review");
    let card = ReviewCard {
        word: WordRecord {
            id: word_id.clone(),
            term: "immersion".to_string(),
            lemma: "immersion".to_string(),
            phonetic: "ih-MER-zhuhn".to_string(),
            definition: "noun: deep involvement".to_string(),
            created_at: now(),
        },
        contexts: Vec::new(),
        state: initial_review_state(word_id, now()),
    };
    let _ = update(&mut model, input(AppKey::Char('r')));
    let _ = update(&mut model, Message::DueReviewsLoaded(Ok(vec![card])));

    let front = draw(&model, 90, 24);
    assert!(!front.contains("deep involvement"));
    let _ = update(&mut model, input(AppKey::Enter));
    let back = draw(&model, 90, 24);
    assert!(back.contains("deep involvement"));
    assert!(back.contains("0 Blackout"));
}

#[test]
fn tiny_terminal_uses_a_non_overlapping_minimum_size_view() {
    let model = reader(40, 8);

    let output = draw(&model, 40, 8);

    assert!(output.contains("Terminal too small"));
    assert!(!output.contains("Vocabulary"));
}

#[test]
fn path_cursor_uses_terminal_display_width() {
    let mut model = Model::empty(60, 20);
    let _ = update(&mut model, input(AppKey::Char('\u{754c}')));
    let backend = TestBackend::new(60, 20);
    let mut terminal = Terminal::new(backend).expect("test terminal must create");

    terminal
        .draw(|frame| render(frame, &model))
        .expect("test view must render");
    let cursor = terminal
        .get_cursor_position()
        .expect("test cursor position must be available");

    assert_eq!(cursor.x, 6);
}
