use std::collections::{BTreeSet, HashMap};

use chrono::{DateTime, Utc};
use core_engine::{CaptureResult, ReviewState};
use language_engine::{Document, InlineStyle, LexiconEntry};

use crate::Page;

pub const READER_SPLIT_MIN_WIDTH: f32 = 750.0;

pub fn reader_uses_split_layout(width: f32) -> bool {
    width >= READER_SPLIT_MIN_WIDTH
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReaderWordDetails {
    pub surface: String,
    pub entry: Option<LexiconEntry>,
    pub state: Option<ReviewState>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReaderBadge {
    New,
    Captured,
    DueNow,
    DueInDays(i64),
}

impl ReaderBadge {
    pub fn label(self) -> String {
        match self {
            Self::New => "New".to_string(),
            Self::Captured => "Captured".to_string(),
            Self::DueNow => "Due now".to_string(),
            Self::DueInDays(days) => format!("Due in {days}d"),
        }
    }
}

pub fn reader_badge(state: Option<&ReviewState>, as_of: DateTime<Utc>) -> ReaderBadge {
    let Some(state) = state else {
        return ReaderBadge::New;
    };
    if state.last_rating.is_none() {
        return ReaderBadge::Captured;
    }
    if state.next_review_at <= as_of {
        return ReaderBadge::DueNow;
    }

    let seconds = (state.next_review_at - as_of).num_seconds().max(1);
    ReaderBadge::DueInDays(seconds.saturating_add(86_399) / 86_400)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReaderRun {
    pub text: String,
    pub token_index: Option<usize>,
    pub style: InlineStyle,
}

pub fn build_block_runs(document: &Document, block_index: usize) -> Vec<ReaderRun> {
    let Some(block) = document.blocks().get(block_index) else {
        return Vec::new();
    };

    let mut boundaries = BTreeSet::from([0, block.text.len()]);
    for range in &block.ranges {
        if valid_boundary(&block.text, range.start) && valid_boundary(&block.text, range.end) {
            boundaries.insert(range.start);
            boundaries.insert(range.end);
        }
    }
    for token in document
        .tokens()
        .iter()
        .filter(|token| token.block_index == block_index)
    {
        if valid_boundary(&block.text, token.start) && valid_boundary(&block.text, token.end) {
            boundaries.insert(token.start);
            boundaries.insert(token.end);
        }
    }

    let boundaries = boundaries.into_iter().collect::<Vec<_>>();
    let mut runs: Vec<ReaderRun> = Vec::new();
    for window in boundaries.windows(2) {
        let [start, end] = window else {
            continue;
        };
        if start == end {
            continue;
        }

        let style = block
            .ranges
            .iter()
            .filter(|range| range.start < *end && range.end > *start)
            .fold(InlineStyle::default(), |mut combined, range| {
                combined.bold |= range.style.bold;
                combined.italic |= range.style.italic;
                combined.underlined |= range.style.underlined;
                combined.dim |= range.style.dim;
                combined
            });
        let token_index = document
            .tokens()
            .iter()
            .enumerate()
            .find_map(|(index, token)| {
                (token.block_index == block_index && token.start <= *start && token.end >= *end)
                    .then_some(index)
            });
        let text = block.text[*start..*end].to_string();

        if token_index.is_none()
            && runs
                .last()
                .is_some_and(|previous| previous.token_index.is_none() && previous.style == style)
        {
            if let Some(previous) = runs.last_mut() {
                previous.text.push_str(&text);
            }
        } else {
            runs.push(ReaderRun {
                text,
                token_index,
                style,
            });
        }
    }
    runs
}

fn valid_boundary(text: &str, index: usize) -> bool {
    index <= text.len() && text.is_char_boundary(index)
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ReaderModel {
    pub document: Option<Document>,
    pub cursor: Option<usize>,
    pub details: Option<ReaderWordDetails>,
    pub details_cache: HashMap<String, ReaderWordDetails>,
    pub open_generation: u64,
    pub hydration_generation: u64,
    pub capture_generation: u64,
    pub opening: bool,
    pub capturing: bool,
    pub feedback: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReaderMessage {
    OpenRequested,
    DocumentOpened {
        generation: u64,
        result: Result<Option<Document>, String>,
    },
    MovePrevious,
    MoveNext,
    SelectToken(usize),
    WordHydrated {
        generation: u64,
        cache_key: String,
        result: Result<ReaderWordDetails, String>,
    },
    CaptureRequested {
        captured_at: DateTime<Utc>,
    },
    CaptureFinished {
        generation: u64,
        cache_key: String,
        lemma: String,
        result: Result<(CaptureResult, ReviewState), String>,
    },
    ClearError,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReaderEffect {
    PickDocument {
        generation: u64,
    },
    HydrateWord {
        generation: u64,
        cache_key: String,
        surface: String,
    },
    CaptureWord {
        generation: u64,
        cache_key: String,
        surface: String,
        sentence: String,
        source_path: std::path::PathBuf,
        entry: LexiconEntry,
        captured_at: DateTime<Utc>,
    },
}

pub fn update_reader(model: &mut ReaderModel, message: ReaderMessage) -> Vec<ReaderEffect> {
    match message {
        ReaderMessage::OpenRequested => {
            model.open_generation = model.open_generation.saturating_add(1);
            model.opening = true;
            model.error = None;
            vec![ReaderEffect::PickDocument {
                generation: model.open_generation,
            }]
        }
        ReaderMessage::DocumentOpened { generation, result } => {
            if generation != model.open_generation {
                return Vec::new();
            }
            model.opening = false;
            match result {
                Ok(Some(document)) => {
                    model.capture_generation = model.capture_generation.saturating_add(1);
                    model.capturing = false;
                    model.document = Some(document);
                    model.cursor = model
                        .document
                        .as_ref()
                        .and_then(|document| (!document.tokens().is_empty()).then_some(0));
                    model.details = None;
                    model.feedback = None;
                    focus_current_word(model)
                }
                Ok(None) => Vec::new(),
                Err(error) => {
                    model.error = Some(error);
                    Vec::new()
                }
            }
        }
        ReaderMessage::MovePrevious => move_cursor(model, false),
        ReaderMessage::MoveNext => move_cursor(model, true),
        ReaderMessage::SelectToken(index) => {
            let is_valid = model
                .document
                .as_ref()
                .is_some_and(|document| index < document.tokens().len());
            if is_valid && model.cursor != Some(index) {
                model.cursor = Some(index);
                focus_current_word(model)
            } else {
                Vec::new()
            }
        }
        ReaderMessage::WordHydrated {
            generation,
            cache_key,
            result,
        } => {
            match result {
                Ok(details) => {
                    model
                        .details_cache
                        .insert(cache_key.clone(), details.clone());
                    if generation == model.hydration_generation
                        && current_cache_key(model).as_deref() == Some(cache_key.as_str())
                    {
                        model.details = Some(details);
                    }
                }
                Err(error) if generation == model.hydration_generation => {
                    model.error = Some(error);
                }
                Err(_) => {}
            }
            Vec::new()
        }
        ReaderMessage::CaptureRequested { captured_at } => {
            if model.capturing {
                return Vec::new();
            }
            let Some(document) = model.document.as_ref() else {
                return Vec::new();
            };
            let Some(token) = model.cursor.and_then(|index| document.tokens().get(index)) else {
                return Vec::new();
            };
            let Some(details) = model.details.as_ref() else {
                return Vec::new();
            };
            let Some(entry) = details.entry.clone() else {
                return Vec::new();
            };

            model.capture_generation = model.capture_generation.saturating_add(1);
            model.capturing = true;
            model.feedback = None;
            model.error = None;
            vec![ReaderEffect::CaptureWord {
                generation: model.capture_generation,
                cache_key: cache_key(&token.surface),
                surface: token.surface.clone(),
                sentence: token.sentence.clone(),
                source_path: document.path().to_path_buf(),
                entry,
                captured_at,
            }]
        }
        ReaderMessage::CaptureFinished {
            generation,
            cache_key,
            lemma,
            result,
        } => {
            if generation != model.capture_generation {
                return Vec::new();
            }
            model.capturing = false;
            match result {
                Ok((capture, state)) => {
                    model.feedback = Some(capture_feedback(capture, &lemma));
                    if let Some(cached) = model.details_cache.get_mut(&cache_key) {
                        cached.state = Some(state.clone());
                    }
                    if current_cache_key(model).as_deref() == Some(cache_key.as_str()) {
                        if let Some(details) = model.details.as_mut() {
                            details.state = Some(state);
                        }
                    }
                }
                Err(error) => model.error = Some(error),
            }
            Vec::new()
        }
        ReaderMessage::ClearError => {
            model.error = None;
            Vec::new()
        }
    }
}

fn move_cursor(model: &mut ReaderModel, forward: bool) -> Vec<ReaderEffect> {
    let Some(document) = model.document.as_ref() else {
        return Vec::new();
    };
    let Some(cursor) = model.cursor else {
        return Vec::new();
    };
    let next = if forward {
        cursor
            .saturating_add(1)
            .min(document.tokens().len().saturating_sub(1))
    } else {
        cursor.saturating_sub(1)
    };
    if next == cursor {
        Vec::new()
    } else {
        model.cursor = Some(next);
        focus_current_word(model)
    }
}

fn focus_current_word(model: &mut ReaderModel) -> Vec<ReaderEffect> {
    let Some(token) = model
        .document
        .as_ref()
        .and_then(|document| model.cursor.and_then(|index| document.tokens().get(index)))
    else {
        model.details = None;
        return Vec::new();
    };
    let surface = token.surface.clone();
    let cache_key = cache_key(&surface);
    model.hydration_generation = model.hydration_generation.saturating_add(1);
    if let Some(details) = model.details_cache.get(&cache_key) {
        model.details = Some(details.clone());
        Vec::new()
    } else {
        model.details = None;
        vec![ReaderEffect::HydrateWord {
            generation: model.hydration_generation,
            cache_key,
            surface,
        }]
    }
}

fn current_cache_key(model: &ReaderModel) -> Option<String> {
    model.document.as_ref().and_then(|document| {
        model
            .cursor
            .and_then(|index| document.tokens().get(index))
            .map(|token| cache_key(&token.surface))
    })
}

fn cache_key(surface: &str) -> String {
    surface.trim().to_lowercase()
}

fn capture_feedback(result: CaptureResult, lemma: &str) -> String {
    if result.word_created {
        format!("Captured {lemma}")
    } else if result.contexts_created > 0 {
        format!("Added context for {lemma}")
    } else {
        format!("{lemma} already has this context")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReaderKey {
    Character(String),
    ModifiedCharacter(String),
    NamedLeft,
    NamedRight,
    NamedUp,
    NamedDown,
    Enter,
    Tab,
    ShiftTab,
    Escape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEventKind {
    Pressed,
    Repeated,
    Captured,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReaderShortcut {
    MovePrevious,
    MoveNext,
    ScrollUp,
    ScrollDown,
    Capture,
    Open,
}

impl ReaderShortcut {
    pub fn from_key(key: ReaderKey, kind: KeyEventKind) -> Option<Self> {
        if kind == KeyEventKind::Captured || matches!(key, ReaderKey::ModifiedCharacter(_)) {
            return None;
        }
        let shortcut = match key {
            ReaderKey::Character(value) => match value.to_lowercase().as_str() {
                "w" | "l" => Self::MoveNext,
                "b" | "h" => Self::MovePrevious,
                "j" => Self::ScrollDown,
                "k" => Self::ScrollUp,
                "c" => Self::Capture,
                "o" => Self::Open,
                _ => return None,
            },
            ReaderKey::NamedLeft => Self::MovePrevious,
            ReaderKey::NamedRight => Self::MoveNext,
            ReaderKey::NamedUp => Self::ScrollUp,
            ReaderKey::NamedDown => Self::ScrollDown,
            ReaderKey::Enter => Self::Capture,
            ReaderKey::Tab | ReaderKey::ShiftTab | ReaderKey::Escape => return None,
            ReaderKey::ModifiedCharacter(_) => return None,
        };
        if kind == KeyEventKind::Repeated
            && !matches!(
                shortcut,
                Self::MovePrevious | Self::MoveNext | Self::ScrollUp | Self::ScrollDown
            )
        {
            None
        } else {
            Some(shortcut)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobalShortcut {
    NextPage,
    PreviousPage,
    Navigate(Page),
    ReleaseFocus,
}

impl GlobalShortcut {
    pub fn from_key(key: ReaderKey, kind: KeyEventKind) -> Option<Self> {
        if kind != KeyEventKind::Pressed || matches!(key, ReaderKey::ModifiedCharacter(_)) {
            return None;
        }
        match key {
            ReaderKey::Character(value) => value
                .parse::<u8>()
                .ok()
                .and_then(Page::from_number)
                .map(Self::Navigate),
            ReaderKey::Tab => Some(Self::NextPage),
            ReaderKey::ShiftTab => Some(Self::PreviousPage),
            ReaderKey::Escape => Some(Self::ReleaseFocus),
            ReaderKey::ModifiedCharacter(_)
            | ReaderKey::NamedLeft
            | ReaderKey::NamedRight
            | ReaderKey::NamedUp
            | ReaderKey::NamedDown
            | ReaderKey::Enter => None,
        }
    }
}
