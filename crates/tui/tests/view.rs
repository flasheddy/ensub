use std::path::PathBuf;

use chrono::{DateTime, TimeZone, Utc};
use core_engine::{initial_review_state, CaptureResult, ReviewCard, WordId, WordRecord};
use ensub_theme::{Rgb, Theme};
use ensub_tui::{
    render, render_with_theme, update, AppKey, CaptureFeedback, ColorPolicy, Document,
    DocumentFormat, InputEvent, InputKind, Message, Model, WordDetails,
};
use language_engine::{Definition, LexiconEntry};
use ratatui::{
    backend::TestBackend,
    buffer::Buffer,
    style::{Color, Modifier},
    Terminal,
};

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

fn draw_themed(
    model: &Model,
    width: u16,
    height: u16,
    theme: &Theme,
    policy: ColorPolicy,
) -> Buffer {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal must create");
    terminal
        .draw(|frame| render_with_theme(frame, model, theme, policy))
        .expect("themed test view must render");
    terminal.backend().buffer().clone()
}

fn find_text(buffer: &Buffer, needle: &str) -> (u16, u16) {
    for y in 0..buffer.area.height {
        let row = (0..buffer.area.width)
            .map(|x| buffer[(x, y)].symbol())
            .collect::<String>();
        if let Some(x) = row.find(needle) {
            return (
                u16::try_from(x).expect("ASCII test label offset must fit"),
                y,
            );
        }
    }
    panic!("test label {needle:?} was not rendered");
}

fn sentinel_theme() -> Theme {
    Theme {
        background: Rgb::new(1, 2, 3),
        surface: Rgb::new(4, 5, 6),
        surface_raised: Rgb::new(7, 8, 9),
        surface_overlay: Rgb::new(10, 11, 12),
        border: Rgb::new(13, 14, 15),
        border_strong: Rgb::new(16, 17, 18),
        text: Rgb::new(19, 20, 21),
        text_muted: Rgb::new(22, 23, 24),
        text_subtle: Rgb::new(25, 26, 27),
        accent: Rgb::new(28, 29, 30),
        on_accent: Rgb::new(31, 32, 33),
        focus: Rgb::new(34, 35, 36),
        selection: Rgb::new(37, 38, 39),
        on_selection: Rgb::new(40, 41, 42),
        success: Rgb::new(43, 44, 45),
        warning: Rgb::new(46, 47, 48),
        danger: Rgb::new(49, 50, 51),
        info: Rgb::new(52, 53, 54),
        ..Theme::default()
    }
}

fn ratatui(color: Rgb) -> Color {
    Color::Rgb(color.red, color.green, color.blue)
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

#[test]
fn semantic_theme_colors_the_canvas_statuses_and_selection() {
    let mut model = reader(120, 26);
    let theme = sentinel_theme();
    let _ = update(
        &mut model,
        Message::DueCountLoaded(Err("theme error".to_string())),
    );

    let buffer = draw_themed(&model, 120, 26, &theme, ColorPolicy::Enabled);

    assert_eq!(buffer[(0, 0)].bg, ratatui(theme.background));
    let mode = find_text(&buffer, "NORMAL");
    assert_eq!(buffer[mode].fg, ratatui(theme.accent));
    let error = find_text(&buffer, "theme error");
    assert_eq!(buffer[error].fg, ratatui(theme.danger));
    let phonetic = find_text(&buffer, "ih-MER-zhuhn");
    assert_eq!(buffer[phonetic].fg, ratatui(theme.text_muted));
    let selected = find_text(&buffer, "Immersion");
    assert_eq!(buffer[selected].fg, ratatui(theme.on_selection));
    assert_eq!(buffer[selected].bg, ratatui(theme.selection));
    assert!(buffer[selected].modifier.contains(Modifier::BOLD));
    assert!(!buffer[selected].modifier.contains(Modifier::REVERSED));
}

#[test]
fn disabled_color_policy_resets_every_cell_but_keeps_modifiers() {
    let model = reader(120, 26);
    let buffer = draw_themed(&model, 120, 26, &sentinel_theme(), ColorPolicy::Disabled);

    assert!(buffer.content().iter().all(|cell| {
        cell.fg == Color::Reset && cell.bg == Color::Reset && cell.underline_color == Color::Reset
    }));
    let selected = find_text(&buffer, "Immersion");
    assert!(buffer[selected].modifier.contains(Modifier::BOLD));
    assert!(!buffer[selected].modifier.contains(Modifier::REVERSED));
}
