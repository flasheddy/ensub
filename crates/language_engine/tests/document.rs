use std::path::PathBuf;

use language_engine::{BlockKind, Document, DocumentFormat};

#[test]
fn portable_document_model_parses_markdown_without_terminal_types() {
    let document = Document::parse(
        PathBuf::from("guide.md"),
        DocumentFormat::Markdown,
        "# Guide\n\nRead **deeply** with `code` nearby.",
    );

    assert!(document
        .blocks()
        .iter()
        .any(|block| block.kind == BlockKind::Heading));
    assert!(document
        .tokens()
        .iter()
        .any(|token| token.surface == "deeply"));
    assert!(!document
        .tokens()
        .iter()
        .any(|token| token.surface == "code"));
}

#[test]
fn plain_text_splits_blank_line_separated_paragraphs() {
    let document = Document::parse(
        PathBuf::from("essay.txt"),
        DocumentFormat::PlainText,
        "First paragraph.\n\nSecond paragraph.",
    );

    let texts = document
        .blocks()
        .iter()
        .map(|block| block.text.as_str())
        .collect::<Vec<_>>();
    assert_eq!(texts, vec!["First paragraph.", "Second paragraph."]);
}

#[test]
fn plain_text_treats_whitespace_only_lines_as_paragraph_separators() {
    let document = Document::parse(
        PathBuf::from("essay.txt"),
        DocumentFormat::PlainText,
        "First paragraph.\n  \t \nSecond paragraph.",
    );

    assert_eq!(document.blocks().len(), 2);
    assert_eq!(document.blocks()[0].text, "First paragraph.");
    assert_eq!(document.blocks()[1].text, "Second paragraph.");
}

#[test]
fn plain_text_normalizes_crlf_before_splitting_paragraphs() {
    let document = Document::parse(
        PathBuf::from("essay.txt"),
        DocumentFormat::PlainText,
        "First line.\r\nStill first.\r\n\r\nSecond paragraph.",
    );

    assert_eq!(document.blocks().len(), 2);
    assert_eq!(document.blocks()[0].text, "First line.\nStill first.");
    assert_eq!(document.blocks()[1].text, "Second paragraph.");
}

#[test]
fn plain_text_preserves_single_newlines_inside_paragraphs() {
    let document = Document::parse(
        PathBuf::from("essay.txt"),
        DocumentFormat::PlainText,
        "First line.\nSecond line.",
    );

    assert_eq!(document.blocks().len(), 1);
    assert_eq!(document.blocks()[0].text, "First line.\nSecond line.");
}

#[test]
fn empty_plain_text_has_no_blocks_or_tokens() {
    let document = Document::parse(
        PathBuf::from("empty.txt"),
        DocumentFormat::PlainText,
        " \n\t\n ",
    );

    assert!(document.blocks().is_empty());
    assert!(document.tokens().is_empty());
}

#[test]
fn plain_text_tokens_and_sentences_do_not_cross_paragraphs() {
    let document = Document::parse(
        PathBuf::from("essay.txt"),
        DocumentFormat::PlainText,
        "A fragment without punctuation\n\nAnother paragraph follows.",
    );

    assert!(document
        .tokens()
        .iter()
        .filter(|token| token.block_index == 0)
        .all(|token| token.sentence == "A fragment without punctuation"));
    assert!(document
        .tokens()
        .iter()
        .filter(|token| token.block_index == 1)
        .all(|token| token.sentence == "Another paragraph follows."));
}
