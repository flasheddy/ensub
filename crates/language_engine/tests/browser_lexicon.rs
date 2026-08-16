use language_engine::{
    BrowserLexicon, BrowserLexiconAsset, BrowserLexiconError, BrowserLexiconForm, Definition,
    Lexicon, LexiconEntry, BROWSER_LEXICON_SCHEMA_VERSION,
};

fn entries() -> Vec<LexiconEntry> {
    vec![
        LexiconEntry {
            lemma: "go".to_string(),
            phonetic: "goʊ".to_string(),
            definitions: vec![Definition {
                part_of_speech: "verb".to_string(),
                text: "move from one place to another".to_string(),
            }],
        },
        LexiconEntry {
            lemma: "immersion".to_string(),
            phonetic: "ɪˈmɝʒən".to_string(),
            definitions: vec![Definition {
                part_of_speech: "noun".to_string(),
                text: "deep involvement in an activity".to_string(),
            }],
        },
    ]
}

fn asset() -> BrowserLexiconAsset {
    BrowserLexiconAsset {
        schema_version: BROWSER_LEXICON_SCHEMA_VERSION,
        definition_source: "Open English WordNet 2025".to_string(),
        pronunciation_source: "CMUdict 0.7b".to_string(),
        entries: entries(),
        forms: vec![
            BrowserLexiconForm {
                surface: "go".to_string(),
                entry_index: 0,
                priority: 0,
            },
            BrowserLexiconForm {
                surface: "immersion".to_string(),
                entry_index: 1,
                priority: 0,
            },
            BrowserLexiconForm {
                surface: "went".to_string(),
                entry_index: 0,
                priority: 0,
            },
        ],
    }
}

#[test]
fn duplicate_surfaces_preserve_priority_and_resolve_the_first_entry() {
    let mut ambiguous = asset();
    ambiguous.forms.push(BrowserLexiconForm {
        surface: "went".to_string(),
        entry_index: 1,
        priority: 1,
    });
    let bytes = ambiguous.encode().expect("ambiguous asset must encode");
    let lexicon = BrowserLexicon::decode(&bytes).expect("ambiguous asset must decode");

    let resolved = lexicon
        .lookup("went")
        .expect("lookup must execute")
        .expect("ambiguous form must resolve");

    assert_eq!(lexicon.form_count(), 4);
    assert_eq!(resolved.lemma, "go");
}

#[test]
fn binary_asset_round_trip_preserves_lookup_semantics() {
    let bytes = asset().encode().expect("fixture must encode");
    let lexicon = BrowserLexicon::decode(&bytes).expect("fixture must decode");

    let went = lexicon
        .lookup(" WENT ")
        .expect("lookup must execute")
        .expect("inflected form must resolve");

    assert!(bytes.starts_with(b"ESBLX\0\r\n"));
    assert_eq!(went.lemma, "go");
    assert_eq!(went.phonetic, "goʊ");
    assert_eq!(lexicon.entry_count(), 2);
    assert_eq!(lexicon.form_count(), 3);
    assert_eq!(lexicon.definition_source(), "Open English WordNet 2025");
}

#[test]
fn decoder_rejects_unsupported_schema_without_guessing() {
    let mut unsupported = asset();
    unsupported.schema_version = BROWSER_LEXICON_SCHEMA_VERSION + 1;
    let bytes = unsupported
        .encode_unchecked()
        .expect("unsupported fixture must serialize");

    let error = BrowserLexicon::decode(&bytes).expect_err("newer schema must fail");

    assert!(matches!(
        error,
        BrowserLexiconError::UnsupportedSchema { .. }
    ));
}

#[test]
fn encoder_rejects_unsorted_forms_and_invalid_entry_links() {
    let mut unsorted = asset();
    unsorted.forms.swap(0, 1);
    assert!(matches!(
        unsorted.encode(),
        Err(BrowserLexiconError::FormsNotSorted)
    ));

    let mut unlinked = asset();
    unlinked.forms[0].entry_index = 99;
    assert!(matches!(
        unlinked.encode(),
        Err(BrowserLexiconError::InvalidEntryIndex { .. })
    ));
}

#[test]
fn decoder_rejects_bad_magic_and_truncated_payloads() {
    assert!(matches!(
        BrowserLexicon::decode(b"not-an-ensub-lexicon"),
        Err(BrowserLexiconError::InvalidMagic)
    ));

    let bytes = asset().encode().expect("fixture must encode");
    let truncated = &bytes[..bytes.len() - 1];
    assert!(matches!(
        BrowserLexicon::decode(truncated),
        Err(BrowserLexiconError::Decode(_))
    ));
}
