use chrono::{DateTime, Utc};
use core_engine::{schedule_review, ReviewCard, ReviewRating, ReviewState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewPhase {
    Empty,
    Prompt,
    Revealed,
    Saving,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    pub as_of: DateTime<Utc>,
    pub due_count: u64,
    pub popup_open: bool,
    pub card: Option<ReviewCard>,
    pub review_phase: ReviewPhase,
    pub error: Option<String>,
}

impl Model {
    pub fn new(as_of: DateTime<Utc>) -> Self {
        Self {
            as_of,
            due_count: 0,
            popup_open: false,
            card: None,
            review_phase: ReviewPhase::Empty,
            error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    Tick(DateTime<Utc>),
    PopupOpened(DateTime<Utc>),
    PopupClosed,
    DueCountLoaded(Result<u64, String>),
    DueCardLoaded(Result<Option<ReviewCard>, String>),
    Reveal,
    Rate(ReviewRating, DateTime<Utc>),
    ReviewCommitted(Result<bool, String>, DateTime<Utc>),
    LaunchCapture,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    RefreshDueCount {
        as_of: DateTime<Utc>,
    },
    LoadDueCard {
        as_of: DateTime<Utc>,
    },
    CommitReview {
        expected: ReviewState,
        replacement: ReviewState,
        reviewed_at: DateTime<Utc>,
    },
    LaunchCaptureHud,
}

pub fn badge_text(count: u64) -> String {
    if count > 99 {
        "99+".to_string()
    } else {
        count.to_string()
    }
}

pub fn update(model: &mut Model, message: Message) -> Vec<Effect> {
    match message {
        Message::Tick(as_of) => {
            model.as_of = as_of;
            vec![Effect::RefreshDueCount { as_of }]
        }
        Message::PopupOpened(as_of) => {
            model.as_of = as_of;
            model.popup_open = true;
            vec![
                Effect::RefreshDueCount { as_of },
                Effect::LoadDueCard { as_of },
            ]
        }
        Message::PopupClosed => {
            model.popup_open = false;
            Vec::new()
        }
        Message::DueCountLoaded(result) => {
            match result {
                Ok(count) => model.due_count = count,
                Err(error) => model.error = Some(error),
            }
            Vec::new()
        }
        Message::DueCardLoaded(result) => {
            match result {
                Ok(card) => {
                    model.card = card;
                    model.review_phase = if model.card.is_some() {
                        ReviewPhase::Prompt
                    } else {
                        ReviewPhase::Empty
                    };
                }
                Err(error) => model.error = Some(error),
            }
            Vec::new()
        }
        Message::Reveal => {
            if model.review_phase == ReviewPhase::Prompt {
                model.review_phase = ReviewPhase::Revealed;
            }
            Vec::new()
        }
        Message::Rate(rating, reviewed_at) => {
            if model.review_phase != ReviewPhase::Revealed {
                return Vec::new();
            }
            let Some(card) = model.card.as_ref() else {
                return Vec::new();
            };
            match schedule_review(&card.state, rating, reviewed_at) {
                Ok(replacement) => {
                    model.review_phase = ReviewPhase::Saving;
                    vec![Effect::CommitReview {
                        expected: card.state.clone(),
                        replacement,
                        reviewed_at,
                    }]
                }
                Err(error) => {
                    model.error = Some(error.to_string());
                    Vec::new()
                }
            }
        }
        Message::ReviewCommitted(result, as_of) => match result {
            Ok(_) => {
                model.card = None;
                model.review_phase = ReviewPhase::Empty;
                vec![
                    Effect::RefreshDueCount { as_of },
                    Effect::LoadDueCard { as_of },
                ]
            }
            Err(error) => {
                model.error = Some(error);
                model.review_phase = ReviewPhase::Revealed;
                Vec::new()
            }
        },
        Message::LaunchCapture => vec![Effect::LaunchCaptureHud],
    }
}
