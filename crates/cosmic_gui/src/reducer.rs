use chrono::{DateTime, Utc};
use core_engine::{
    schedule_review, LibraryPage, ReviewActivity, ReviewCard, ReviewHistoryPage, ReviewRating,
    ReviewState, ReviewStatistics,
};

use crate::{update_reader, ReaderEffect, ReaderMessage, ReaderModel};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Dashboard,
    Library,
    Reader,
    ParseText,
    Review,
}

impl Page {
    pub const fn next(self) -> Self {
        match self {
            Self::Dashboard => Self::Library,
            Self::Library => Self::Reader,
            Self::Reader => Self::ParseText,
            Self::ParseText => Self::Review,
            Self::Review => Self::Dashboard,
        }
    }

    pub const fn previous(self) -> Self {
        match self {
            Self::Dashboard => Self::Review,
            Self::Library => Self::Dashboard,
            Self::Reader => Self::Library,
            Self::ParseText => Self::Reader,
            Self::Review => Self::ParseText,
        }
    }

    pub const fn from_number(number: u8) -> Option<Self> {
        match number {
            1 => Some(Self::Dashboard),
            2 => Some(Self::Library),
            3 => Some(Self::Reader),
            4 => Some(Self::ParseText),
            5 => Some(Self::Review),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewPhase {
    Empty,
    Prompt,
    Revealed,
    Saving,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReviewModel {
    pub cards: Vec<ReviewCard>,
    pub active_index: usize,
    pub phase: ReviewPhase,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DashboardData {
    pub statistics: ReviewStatistics,
    pub activity: ReviewActivity,
    pub history: ReviewHistoryPage,
}

impl Default for ReviewModel {
    fn default() -> Self {
        Self {
            cards: Vec::new(),
            active_index: 0,
            phase: ReviewPhase::Empty,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    pub page: Page,
    pub as_of: DateTime<Utc>,
    pub library_generation: u64,
    pub library: Option<LibraryPage>,
    pub dashboard: Option<DashboardData>,
    pub review: ReviewModel,
    pub reader: ReaderModel,
    pub error: Option<String>,
}

impl Model {
    pub fn new(as_of: DateTime<Utc>) -> Self {
        Self {
            page: Page::Dashboard,
            as_of,
            library_generation: 0,
            library: None,
            dashboard: None,
            review: ReviewModel::default(),
            reader: ReaderModel::default(),
            error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    Navigate {
        page: Page,
        as_of: DateTime<Utc>,
    },
    LibraryLoaded {
        generation: u64,
        result: Result<LibraryPage, String>,
    },
    DashboardLoaded(Result<DashboardData, String>),
    ReviewBatchLoaded(Result<Vec<ReviewCard>, String>),
    RevealReview,
    SkipReview,
    Rate(ReviewRating, DateTime<Utc>),
    ReviewCommitted(Result<bool, String>, DateTime<Utc>),
    Reader(ReaderMessage),
    ClearError,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    LoadDashboard {
        as_of: DateTime<Utc>,
    },
    LoadLibrary {
        generation: u64,
    },
    LoadReviewBatch {
        as_of: DateTime<Utc>,
    },
    CommitReview {
        expected: ReviewState,
        replacement: ReviewState,
        reviewed_at: DateTime<Utc>,
    },
    Reader(ReaderEffect),
}

pub fn update(model: &mut Model, message: Message) -> Vec<Effect> {
    match message {
        Message::Navigate { page, as_of } => {
            model.page = page;
            model.as_of = as_of;
            model.error = None;
            match page {
                Page::Dashboard => vec![Effect::LoadDashboard { as_of: model.as_of }],
                Page::Library => {
                    model.library_generation = model.library_generation.saturating_add(1);
                    vec![Effect::LoadLibrary {
                        generation: model.library_generation,
                    }]
                }
                Page::Review => vec![Effect::LoadReviewBatch { as_of: model.as_of }],
                Page::Reader | Page::ParseText => Vec::new(),
            }
        }
        Message::LibraryLoaded { generation, result } => {
            if generation == model.library_generation {
                match result {
                    Ok(page) => model.library = Some(page),
                    Err(error) => model.error = Some(error),
                }
            }
            Vec::new()
        }
        Message::DashboardLoaded(result) => {
            match result {
                Ok(dashboard) => model.dashboard = Some(dashboard),
                Err(error) => model.error = Some(error),
            }
            Vec::new()
        }
        Message::ReviewBatchLoaded(result) => {
            match result {
                Ok(cards) => {
                    model.review.cards = cards;
                    model.review.active_index = 0;
                    model.review.phase = if model.review.cards.is_empty() {
                        ReviewPhase::Empty
                    } else {
                        ReviewPhase::Prompt
                    };
                }
                Err(error) => model.error = Some(error),
            }
            Vec::new()
        }
        Message::RevealReview => {
            if model.review.phase == ReviewPhase::Prompt {
                model.review.phase = ReviewPhase::Revealed;
            }
            Vec::new()
        }
        Message::SkipReview => {
            if model.review.active_index < model.review.cards.len() {
                model.review.active_index = model.review.active_index.saturating_add(1);
            }
            model.review.phase = if model.review.active_index < model.review.cards.len() {
                ReviewPhase::Prompt
            } else {
                ReviewPhase::Empty
            };
            Vec::new()
        }
        Message::Rate(rating, reviewed_at) => {
            if model.review.phase != ReviewPhase::Revealed {
                return Vec::new();
            }
            let Some(card) = model.review.cards.get(model.review.active_index) else {
                return Vec::new();
            };
            match schedule_review(&card.state, rating, reviewed_at) {
                Ok(replacement) => {
                    model.review.phase = ReviewPhase::Saving;
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
            Ok(true) => {
                model.as_of = as_of;
                model.review.active_index = model.review.active_index.saturating_add(1);
                model.review.phase = if model.review.active_index < model.review.cards.len() {
                    ReviewPhase::Prompt
                } else {
                    ReviewPhase::Empty
                };
                Vec::new()
            }
            Ok(false) => {
                model.as_of = as_of;
                model.review = ReviewModel::default();
                vec![Effect::LoadReviewBatch { as_of }]
            }
            Err(error) => {
                model.error = Some(error);
                model.review.phase = ReviewPhase::Revealed;
                Vec::new()
            }
        },
        Message::Reader(message) => update_reader(&mut model.reader, message)
            .into_iter()
            .map(Effect::Reader)
            .collect(),
        Message::ClearError => {
            model.error = None;
            Vec::new()
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HudModel {
    pub text: String,
    pub clipboard_requested: bool,
    pub parsing: bool,
    pub capturing: bool,
    pub captured: u64,
    pub error: Option<String>,
    pub closed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HudMessage {
    Opened,
    ClipboardLoaded(Result<String, String>),
    TextChanged(String),
    Parse,
    Capture,
    CaptureFinished(Result<u64, String>),
    Cancel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HudEffect {
    ReadClipboard,
    ParseText(String),
    CaptureSelected,
}

pub fn update_hud(model: &mut HudModel, message: HudMessage) -> Vec<HudEffect> {
    match message {
        HudMessage::Opened if !model.clipboard_requested => {
            model.clipboard_requested = true;
            vec![HudEffect::ReadClipboard]
        }
        HudMessage::Opened => Vec::new(),
        HudMessage::ClipboardLoaded(result) => {
            match result {
                Ok(text) => model.text = text,
                Err(error) => model.error = Some(error),
            }
            Vec::new()
        }
        HudMessage::TextChanged(text) => {
            model.text = text;
            model.error = None;
            Vec::new()
        }
        HudMessage::Parse => {
            model.parsing = true;
            vec![HudEffect::ParseText(model.text.clone())]
        }
        HudMessage::Capture => {
            model.capturing = true;
            vec![HudEffect::CaptureSelected]
        }
        HudMessage::CaptureFinished(result) => {
            model.capturing = false;
            match result {
                Ok(count) => {
                    model.captured = count;
                    model.closed = true;
                }
                Err(error) => model.error = Some(error),
            }
            Vec::new()
        }
        HudMessage::Cancel => {
            model.closed = true;
            Vec::new()
        }
    }
}
