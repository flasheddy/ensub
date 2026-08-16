use std::path::PathBuf;

use ensub_tui::{BlockKind, Document, DocumentFormat};

#[test]
fn markdown_renders_structure_and_only_prose_is_capturable() {
    let source = r#"---
title: Hidden metadata
---
# Immersion Guide

Reading **useful** [guides](https://example.com) with `cargo` builds fluency.

> Repeated context helps.

```rust
println!("not vocabulary");
```
"#;

    let document = Document::parse(
        PathBuf::from("article.md"),
        DocumentFormat::Markdown,
        source,
    );

    let rendered = document.rendered_text();
    assert!(rendered.contains("Immersion Guide"));
    assert!(rendered.contains("Reading useful guides with cargo builds fluency."));
    assert!(!rendered.contains("Hidden metadata"));
    assert!(!rendered.contains("**"));
    assert!(!rendered.contains("https://"));
    assert!(document
        .blocks()
        .iter()
        .any(|block| block.kind == BlockKind::Heading));
    assert!(document
        .blocks()
        .iter()
        .any(|block| block.kind == BlockKind::Code));

    let surfaces: Vec<&str> = document
        .tokens()
        .iter()
        .map(|token| token.surface.as_str())
        .collect();
    assert!(surfaces.contains(&"Immersion"));
    assert!(surfaces.contains(&"useful"));
    assert!(surfaces.contains(&"guides"));
    assert!(surfaces.contains(&"Repeated"));
    assert!(!surfaces.contains(&"cargo"));
    assert!(!surfaces.contains(&"println"));
    let useful = document
        .tokens()
        .iter()
        .find(|token| token.surface == "useful")
        .expect("useful token must exist");
    assert_eq!(
        useful.sentence,
        "Reading useful guides with cargo builds fluency."
    );
}

#[test]
fn plain_text_normalizes_controls_and_keeps_repeated_words() {
    let document = Document::parse(
        PathBuf::from("notes.txt"),
        DocumentFormat::PlainText,
        "Read\r\nread\tcarefully.\u{1b}[31m",
    );

    assert_eq!(document.rendered_text(), "Read\nread    carefully.");
    let surfaces: Vec<&str> = document
        .tokens()
        .iter()
        .map(|token| token.surface.as_str())
        .collect();
    assert_eq!(surfaces, vec!["Read", "read", "carefully"]);
}

#[test]
fn format_detection_uses_markdown_extensions_case_insensitively() {
    assert_eq!(
        DocumentFormat::from_path(&PathBuf::from("README.MD")),
        DocumentFormat::Markdown
    );
    assert_eq!(
        DocumentFormat::from_path(&PathBuf::from("article.markdown")),
        DocumentFormat::Markdown
    );
    assert_eq!(
        DocumentFormat::from_path(&PathBuf::from("article.txt")),
        DocumentFormat::PlainText
    );
}
