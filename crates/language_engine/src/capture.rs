use chrono::{DateTime, Utc};
use core_engine::{initial_review_state, Capture, ContextId, ContextRecord, WordId, WordRecord};
use uuid::Uuid;

use crate::{Candidate, LexiconEntry};

pub fn word_id_for_lemma(lemma: &str) -> WordId {
    let lemma = lemma.trim().to_lowercase();
    WordId::new(
        Uuid::new_v5(
            &Uuid::NAMESPACE_URL,
            format!("ensub:word:{lemma}").as_bytes(),
        )
        .to_string(),
    )
}

pub fn capture_from_candidate(
    candidate: &Candidate,
    source: &str,
    captured_at: DateTime<Utc>,
) -> Capture {
    capture_from_entry(
        &candidate.surface,
        Some(&candidate.sentence),
        source,
        candidate.entry.clone(),
        captured_at,
    )
}

pub fn capture_from_entry(
    surface: &str,
    context: Option<&str>,
    source: &str,
    entry: LexiconEntry,
    captured_at: DateTime<Utc>,
) -> Capture {
    let lemma = entry.lemma.trim().to_lowercase();
    let word_id = word_id_for_lemma(&lemma);
    let definition = entry
        .definitions
        .iter()
        .map(|definition| format!("{}: {}", definition.part_of_speech, definition.text))
        .collect::<Vec<_>>()
        .join("\n");
    let word = WordRecord {
        id: word_id.clone(),
        term: surface.trim().to_string(),
        lemma,
        phonetic: entry.phonetic,
        definition,
        created_at: captured_at,
    };
    let contexts = context
        .map(str::trim)
        .filter(|sentence| !sentence.is_empty())
        .map(|sentence| {
            let key = format!("ensub:context:{}:{source}:{sentence}", word_id.as_str());
            ContextRecord {
                id: ContextId::new(Uuid::new_v5(&Uuid::NAMESPACE_URL, key.as_bytes()).to_string()),
                word_id: word_id.clone(),
                sentence: sentence.to_string(),
                source: source.to_string(),
                captured_at,
            }
        })
        .into_iter()
        .collect();

    Capture {
        word,
        contexts,
        initial_review_state: initial_review_state(word_id, captured_at),
    }
}
