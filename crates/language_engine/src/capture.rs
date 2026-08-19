use chrono::{DateTime, Utc};
use core_engine::{
    initial_review_state, Capture, ContextId, ContextRecord, MediaDomainError, PodcastCapture,
    PodcastContext, PodcastContextDraft, PodcastContextRecord, WordId, WordRecord,
};
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

pub fn podcast_capture_from_entry(
    draft: PodcastContextDraft,
    entry: LexiconEntry,
    captured_at: DateTime<Utc>,
) -> Result<PodcastCapture, MediaDomainError> {
    let surface = draft.selected_token.surface().to_string();
    let episode_id = draft.episode.internal_id.clone();
    let transcript_url = draft.transcript.url.clone();
    let selected_cue_id = draft.selected_cue_id.clone();
    let token_start = draft.selected_token.start_byte();
    let token_end = draft.selected_token.end_byte();
    let sentence = draft.sentence.trim().to_string();
    let normalized_lemma = entry.lemma.trim().to_lowercase();
    let word_id = word_id_for_lemma(&normalized_lemma);
    let context_key = format!(
        "ensub:podcast-context:{}:{episode_id}:{transcript_url}:{selected_cue_id}:{token_start}:{token_end}",
        word_id.as_str()
    );
    let context_id =
        ContextId::new(Uuid::new_v5(&Uuid::NAMESPACE_URL, context_key.as_bytes()).to_string());
    let podcast_context = PodcastContext::try_from_draft(draft, normalized_lemma, captured_at)?;
    let source = format!("podcast:{episode_id}");
    let mut capture = capture_from_entry(&surface, None, &source, entry, captured_at);
    capture.contexts.push(ContextRecord {
        id: context_id.clone(),
        word_id: word_id.clone(),
        sentence,
        source,
        captured_at,
    });
    PodcastCapture::try_new(
        capture,
        PodcastContextRecord {
            context_id,
            word_id,
            context: podcast_context,
        },
    )
}
