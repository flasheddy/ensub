use chrono::{TimeZone, Utc};
use core_engine::{initial_review_state, ReviewCard, ReviewRating, WordId, WordRecord};
use ensub_applet::{badge_text, update, Effect, Message, Model, ReviewPhase};

fn now() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 8, 15, 12, 0, 0)
        .single()
        .expect("test timestamp must be valid")
}

fn card() -> ReviewCard {
    let word_id = WordId::new("word-panel");
    ReviewCard {
        word: WordRecord {
            id: word_id.clone(),
            term: "panel".to_string(),
            lemma: "panel".to_string(),
            phonetic: "pænəl".to_string(),
            definition: "a small control surface".to_string(),
            created_at: now(),
        },
        contexts: Vec::new(),
        state: initial_review_state(word_id, now()),
    }
}

#[test]
fn badge_caps_visual_text_but_preserves_exact_count() {
    let mut model = Model::new(now());
    let _ = update(&mut model, Message::DueCountLoaded(Ok(142)));

    assert_eq!(model.due_count, 142);
    assert_eq!(badge_text(model.due_count), "99+");
}

#[test]
fn opening_popup_refreshes_count_and_loads_one_card() {
    let mut model = Model::new(now());
    let effects = update(&mut model, Message::PopupOpened(now()));

    assert!(model.popup_open);
    assert_eq!(
        effects,
        vec![
            Effect::RefreshDueCount { as_of: now() },
            Effect::LoadDueCard { as_of: now() }
        ]
    );
}

#[test]
fn micro_review_reveals_then_commits_and_refreshes() {
    let mut model = Model::new(now());
    let review_card = card();
    let _ = update(
        &mut model,
        Message::DueCardLoaded(Ok(Some(review_card.clone()))),
    );
    let _ = update(&mut model, Message::Reveal);
    assert_eq!(model.review_phase, ReviewPhase::Revealed);

    let effects = update(
        &mut model,
        Message::Rate(
            ReviewRating::try_from(5).expect("rating must be valid"),
            now(),
        ),
    );
    assert!(
        matches!(effects.as_slice(), [Effect::CommitReview { expected, .. }] if expected == &review_card.state)
    );

    let effects = update(&mut model, Message::ReviewCommitted(Ok(true), now()));
    assert_eq!(
        effects,
        vec![
            Effect::RefreshDueCount { as_of: now() },
            Effect::LoadDueCard { as_of: now() }
        ]
    );
}
