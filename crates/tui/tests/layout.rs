use std::path::PathBuf;

use ensub_tui::{Document, DocumentFormat, DocumentLayout};
use unicode_width::UnicodeWidthStr;

fn document(text: &str) -> Document {
    Document::parse(PathBuf::from("reader.txt"), DocumentFormat::PlainText, text)
}

#[test]
fn wraps_without_exceeding_unicode_display_width() {
    let document = document("Caf\u{e9} readers revisit meaningful context every day.");

    let layout = DocumentLayout::new(&document, 14);

    assert!(layout.lines().len() > 1);
    for line in layout.lines() {
        assert!(UnicodeWidthStr::width(line.text.as_str()) <= 14);
    }
    assert_eq!(layout.placements().len(), document.tokens().len());
}

#[test]
fn adjacent_line_selection_preserves_the_preferred_column() {
    let document = document("alpha beta gamma delta epsilon zeta eta theta");
    let layout = DocumentLayout::new(&document, 18);
    let beta = document
        .tokens()
        .iter()
        .position(|token| token.surface == "beta")
        .expect("beta token must exist");
    let beta_position = layout.placement(beta).expect("beta placement must exist");

    let below = layout
        .nearest_token_on_line(beta_position.line.saturating_add(1), beta_position.x)
        .expect("next line must contain a token");

    assert_eq!(document.tokens()[below].surface, "epsilon");
}

#[test]
fn zero_width_layout_is_empty_instead_of_panicking() {
    let document = document("alpha beta");

    let layout = DocumentLayout::new(&document, 0);

    assert!(layout.lines().is_empty());
    assert!(layout.placements().is_empty());
}
