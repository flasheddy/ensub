use language_engine::{capture_from_entry, word_id_for_lemma, Definition, LexiconEntry};

fn entry(lemma: &str) -> LexiconEntry {
    LexiconEntry {
        lemma: lemma.to_string(),
        phonetic: "go\u{28a}".to_string(),
        definitions: vec![
            Definition {
                part_of_speech: "verb".to_string(),
                text: "move from one place to another".to_string(),
            },
            Definition {
                part_of_speech: "noun".to_string(),
                text: "an attempt".to_string(),
            },
        ],
    }
}

#[test]
fn capture_factory_preserves_deterministic_identity_and_formatting() {
    let captured_at = "2026-08-15T01:00:00Z"
        .parse()
        .expect("test timestamp must parse");

    let capture = capture_from_entry(
        "Went",
        Some("She went home."),
        "tui:/tmp/a",
        entry("go"),
        captured_at,
    );

    assert_eq!(
        capture.word.id.as_str(),
        "2ba4fdf2-12b7-5aa1-96a3-68c1954d14fa"
    );
    assert_eq!(capture.word.id, word_id_for_lemma("  GO "));
    assert_eq!(capture.word.term, "Went");
    assert_eq!(capture.word.lemma, "go");
    assert_eq!(
        capture.word.definition,
        "verb: move from one place to another\nnoun: an attempt"
    );
    assert_eq!(capture.contexts.len(), 1);
    assert_eq!(
        capture.contexts[0].id.as_str(),
        "2336ffb8-5807-56de-816e-77987bf673fe"
    );
    assert_eq!(capture.initial_review_state.word_id, capture.word.id);
}

#[test]
fn context_identity_is_idempotent_and_changes_with_source() {
    let captured_at = "2026-08-15T01:00:00Z"
        .parse()
        .expect("test timestamp must parse");
    let first = capture_from_entry(
        "went",
        Some("She went home."),
        "tui:/tmp/a",
        entry("go"),
        captured_at,
    );
    let repeated = capture_from_entry(
        "went",
        Some("She went home."),
        "tui:/tmp/a",
        entry("go"),
        captured_at,
    );
    let another_source = capture_from_entry(
        "went",
        Some("She went home."),
        "tui:/tmp/b",
        entry("go"),
        captured_at,
    );

    assert_eq!(first.contexts[0].id, repeated.contexts[0].id);
    assert_ne!(first.contexts[0].id, another_source.contexts[0].id);
}
