use std::collections::HashMap;
use std::convert::Infallible;

use language_engine::{
    extract_candidates, segment_text, Definition, Lexicon, LexiconEntry, ParseOptions, ParseReport,
};

struct FakeLexicon {
    entries: HashMap<String, LexiconEntry>,
}

impl FakeLexicon {
    fn new(entries: &[(&str, &str)]) -> Self {
        let entries = entries
            .iter()
            .map(|(surface, lemma)| {
                (
                    (*surface).to_string(),
                    LexiconEntry {
                        lemma: (*lemma).to_string(),
                        phonetic: format!("/{lemma}/"),
                        definitions: vec![Definition {
                            part_of_speech: "noun".to_string(),
                            text: format!("definition of {lemma}"),
                        }],
                    },
                )
            })
            .collect();
        Self { entries }
    }
}

impl Lexicon for FakeLexicon {
    type Error = Infallible;

    fn lookup(&self, surface: &str) -> Result<Option<LexiconEntry>, Self::Error> {
        Ok(self.entries.get(&surface.to_lowercase()).cloned())
    }
}

fn parse(text: &str, lexicon: &FakeLexicon) -> ParseReport {
    extract_candidates(text, lexicon, ParseOptions::default())
        .expect("fake lexicon lookup must succeed")
}

#[test]
fn segments_every_word_occurrence_with_sentence_ownership() {
    let text = "The mice went home. The mice re-read https://example.com/docs.";

    let segmentation = segment_text(text);

    assert_eq!(segmentation.sentences.len(), 2);
    let words: Vec<&str> = segmentation
        .sentences
        .iter()
        .flat_map(|sentence| &sentence.words)
        .map(|word| &text[word.start..word.end])
        .collect();
    assert_eq!(
        words,
        vec!["The", "mice", "went", "home", "The", "mice", "re-read"]
    );
    for sentence in &segmentation.sentences {
        let sentence_text = &text[sentence.start..sentence.end];
        for word in &sentence.words {
            assert!(word.start >= sentence.start);
            assert!(word.end <= sentence.end);
            assert!(sentence_text.contains(&text[word.start..word.end]));
        }
    }
}

#[test]
fn extracts_owned_candidates_with_exact_sentence_and_token_offsets() {
    let text = "The mice went home. The mice waited.";
    let lexicon = FakeLexicon::new(&[
        ("mice", "mouse"),
        ("went", "go"),
        ("home", "home"),
        ("waited", "wait"),
    ]);

    let report = parse(text, &lexicon);

    let lemmas: Vec<&str> = report
        .candidates
        .iter()
        .map(|candidate| candidate.entry.lemma.as_str())
        .collect();
    assert_eq!(lemmas, vec!["mouse", "go", "home", "wait"]);
    assert_eq!(report.candidates[0].surface, "mice");
    assert_eq!(report.candidates[0].sentence, "The mice went home.");
    assert_eq!(
        &text[report.candidates[0].sentence_start..report.candidates[0].sentence_end],
        report.candidates[0].sentence
    );
    assert_eq!(
        &text[report.candidates[0].token_start..report.candidates[0].token_end],
        report.candidates[0].surface
    );
    assert_eq!(report.filtered_stopwords, 2);
    assert_eq!(report.lookup_misses, 0);
}

#[test]
fn english_abbreviations_do_not_break_surrounding_context_sentences() {
    let text = "Mr. Smith went home. Dr. Jones stayed.";
    let lexicon = FakeLexicon::new(&[("went", "go"), ("stayed", "stay")]);

    let report = parse(text, &lexicon);

    assert_eq!(report.candidates[0].sentence, "Mr. Smith went home.");
    assert_eq!(report.candidates[1].sentence, "Dr. Jones stayed.");
}

#[test]
fn deduplicates_by_lemma_and_preserves_first_occurrence() {
    let text = "Went first. Go later.";
    let lexicon = FakeLexicon::new(&[("went", "go"), ("go", "go")]);

    let report = parse(text, &lexicon);

    assert_eq!(report.candidates.len(), 1);
    assert_eq!(report.candidates[0].surface, "Went");
    assert_eq!(report.candidates[0].sentence, "Went first.");
}

#[test]
fn reports_lookup_misses_and_candidate_truncation() {
    let text = "Alpha mystery beta gamma.";
    let lexicon = FakeLexicon::new(&[("alpha", "alpha"), ("beta", "beta"), ("gamma", "gamma")]);
    let options = ParseOptions {
        include_stopwords: false,
        max_candidates: 2,
    };

    let report =
        extract_candidates(text, &lexicon, options).expect("fake lexicon lookup must succeed");

    assert_eq!(report.lookup_misses, 1);
    assert_eq!(report.candidates.len(), 2);
    assert_eq!(report.truncated_candidates, 1);
}

#[test]
fn stopword_filter_can_be_disabled() {
    let text = "The cat and dog.";
    let lexicon = FakeLexicon::new(&[
        ("the", "the"),
        ("cat", "cat"),
        ("and", "and"),
        ("dog", "dog"),
    ]);

    let filtered = parse(text, &lexicon);
    let included = extract_candidates(
        text,
        &lexicon,
        ParseOptions {
            include_stopwords: true,
            max_candidates: 100,
        },
    )
    .expect("fake lexicon lookup must succeed");

    assert_eq!(filtered.candidates.len(), 2);
    assert_eq!(included.candidates.len(), 4);
}

#[test]
fn preserves_apostrophe_and_hyphen_compounds() {
    let text = "Don't re-read a well-known line. Don’t panic.";
    let lexicon = FakeLexicon::new(&[
        ("don't", "do"),
        ("re-read", "reread"),
        ("well-known", "well-known"),
        ("don’t", "do"),
        ("panic", "panic"),
    ]);

    let report = parse(text, &lexicon);
    let surfaces: Vec<&str> = report
        .candidates
        .iter()
        .map(|candidate| candidate.surface.as_str())
        .collect();

    assert_eq!(surfaces, vec!["Don't", "re-read", "well-known", "panic"]);
    for candidate in &report.candidates {
        assert_eq!(
            &text[candidate.token_start..candidate.token_end],
            candidate.surface
        );
    }
}

#[test]
fn ignores_words_inside_urls() {
    let text = "Learn at https://example.com/interesting-words today.";
    let lexicon = FakeLexicon::new(&[
        ("learn", "learn"),
        ("https", "https"),
        ("example", "example"),
        ("com", "com"),
        ("interesting-words", "interesting-words"),
        ("today", "today"),
    ]);

    let report = parse(text, &lexicon);
    let lemmas: Vec<&str> = report
        .candidates
        .iter()
        .map(|candidate| candidate.entry.lemma.as_str())
        .collect();

    assert_eq!(lemmas, vec!["learn", "today"]);
}

#[test]
fn handles_quotes_ellipses_and_final_unpunctuated_sentence() {
    let text = "“Wait...” she whispered. Then continued without punctuation";
    let lexicon = FakeLexicon::new(&[
        ("wait", "wait"),
        ("whispered", "whisper"),
        ("continued", "continue"),
        ("without", "without"),
        ("punctuation", "punctuation"),
    ]);

    let report = parse(text, &lexicon);

    assert_eq!(report.candidates[0].sentence, "“Wait...” she whispered.");
    assert_eq!(
        report
            .candidates
            .last()
            .map(|candidate| candidate.sentence.as_str()),
        Some("Then continued without punctuation")
    );
}

#[test]
fn arbitrary_unicode_never_produces_invalid_offsets() {
    let text = "你好 👩🏽‍💻 café — naïve Ελληνικά. अंतिम वाक्य";
    let lexicon = FakeLexicon::new(&[("café", "café"), ("naïve", "naive")]);

    let report = parse(text, &lexicon);

    for candidate in &report.candidates {
        assert_eq!(
            text.get(candidate.token_start..candidate.token_end),
            Some(candidate.surface.as_str())
        );
        assert_eq!(
            text.get(candidate.sentence_start..candidate.sentence_end),
            Some(candidate.sentence.as_str())
        );
    }
}
