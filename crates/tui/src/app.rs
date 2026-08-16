use std::collections::HashMap;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use core_engine::{
    schedule_review, CaptureResult, ReviewCard, ReviewRating, ReviewState, ReviewUpdate,
};
use language_engine::LexiconEntry;

use crate::{Document, DocumentLayout, DocumentToken};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Reader,
    OpenPath,
    Vocabulary,
    Review,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputKind {
    Press,
    Repeat,
    Release,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppKey {
    Char(char),
    Ctrl(char),
    Enter,
    Space,
    Esc,
    Tab,
    Backspace,
    Delete,
    Left,
    Right,
    Home,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputEvent {
    pub key: AppKey,
    pub kind: InputKind,
    pub at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WordDetails {
    pub surface: String,
    pub entry: Option<LexiconEntry>,
    pub state: Option<ReviewState>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CaptureFeedback {
    pub result: CaptureResult,
    pub details: WordDetails,
    pub captured_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub message: String,
    pub is_error: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    OpenFile(PathBuf),
    HydrateWords {
        generation: u64,
        surfaces: Vec<String>,
    },
    Capture {
        surface: String,
        context: String,
        source: String,
        captured_at: DateTime<Utc>,
    },
    LoadDueReviews {
        as_of: DateTime<Utc>,
    },
    SaveReview {
        expected: ReviewState,
        replacement: ReviewState,
        reviewed_at: DateTime<Utc>,
    },
    RefreshDueCount {
        as_of: DateTime<Utc>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    Input(InputEvent),
    Resize {
        width: u16,
        height: u16,
    },
    Tick(DateTime<Utc>),
    FileLoaded(Result<Document, String>),
    WordsHydrated {
        generation: u64,
        result: Result<Vec<WordDetails>, String>,
    },
    CaptureFinished(Result<CaptureFeedback, String>),
    DueReviewsLoaded(Result<Vec<ReviewCard>, String>),
    ReviewSaved {
        result: Result<ReviewUpdate, String>,
        replacement: ReviewState,
        reviewed_at: DateTime<Utc>,
    },
    DueCountLoaded(Result<u64, String>),
}

#[derive(Debug, Clone)]
struct ReviewSession {
    cards: Vec<ReviewCard>,
    index: usize,
    revealed: bool,
    saving: bool,
}

#[derive(Debug, Clone)]
pub struct Model {
    document: Option<Document>,
    layout: DocumentLayout,
    width: u16,
    height: u16,
    cursor: Option<usize>,
    preferred_x: u16,
    viewport_top: usize,
    mode: Mode,
    panel_visible: bool,
    panel_preferred: bool,
    should_quit: bool,
    pending_g: bool,
    path_input: String,
    path_cursor: usize,
    due_count: Option<u64>,
    generation: u64,
    word_details: HashMap<String, WordDetails>,
    review: Option<ReviewSession>,
    notice: Option<Notice>,
    as_of: Option<DateTime<Utc>>,
}

impl Model {
    pub fn empty(width: u16, height: u16) -> Self {
        Self {
            document: None,
            layout: DocumentLayout::default(),
            width,
            height,
            cursor: None,
            preferred_x: 0,
            viewport_top: 0,
            mode: Mode::OpenPath,
            panel_visible: width >= 100,
            panel_preferred: true,
            should_quit: false,
            pending_g: false,
            path_input: String::new(),
            path_cursor: 0,
            due_count: None,
            generation: 0,
            word_details: HashMap::new(),
            review: None,
            notice: None,
            as_of: None,
        }
    }

    pub fn with_document(document: Document, width: u16, height: u16) -> Self {
        let mut model = Self::empty(width, height);
        model.mode = Mode::Reader;
        model.set_document(document);
        model
    }

    pub fn document(&self) -> Option<&Document> {
        self.document.as_ref()
    }

    pub fn layout(&self) -> &DocumentLayout {
        &self.layout
    }

    pub fn active_token(&self) -> Option<&DocumentToken> {
        self.cursor
            .and_then(|index| self.document.as_ref()?.tokens().get(index))
    }

    pub fn active_token_index(&self) -> Option<usize> {
        self.cursor
    }

    pub fn active_word_details(&self) -> Option<&WordDetails> {
        let key = self.active_token()?.surface.to_lowercase();
        self.word_details.get(&key)
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn review_revealed(&self) -> bool {
        self.review.as_ref().is_some_and(|review| review.revealed)
    }

    pub fn review_card(&self) -> Option<&ReviewCard> {
        let review = self.review.as_ref()?;
        review.cards.get(review.index)
    }

    pub fn review_progress(&self) -> Option<(usize, usize)> {
        let review = self.review.as_ref()?;
        Some((review.index.saturating_add(1), review.cards.len()))
    }

    pub fn due_count(&self) -> Option<u64> {
        self.due_count
    }

    pub fn notice(&self) -> Option<&Notice> {
        self.notice.as_ref()
    }

    pub fn path_input(&self) -> &str {
        &self.path_input
    }

    pub fn path_cursor(&self) -> usize {
        self.path_cursor
    }

    pub fn as_of(&self) -> Option<DateTime<Utc>> {
        self.as_of
    }

    pub fn viewport_top(&self) -> usize {
        self.viewport_top
    }

    pub fn panel_visible(&self) -> bool {
        self.panel_visible
    }

    pub fn terminal_size(&self) -> (u16, u16) {
        (self.width, self.height)
    }

    pub fn progress_percent(&self) -> u8 {
        let Some(document) = self.document.as_ref() else {
            return 0;
        };
        let Some(cursor) = self.cursor else {
            return 0;
        };
        let last = document.tokens().len().saturating_sub(1);
        if last == 0 {
            return 100;
        }
        u8::try_from(cursor.saturating_mul(100) / last).unwrap_or(100)
    }

    pub fn word_details(&self, surface: &str) -> Option<&WordDetails> {
        self.word_details.get(&surface.to_lowercase())
    }

    fn set_document(&mut self, document: Document) {
        self.generation = self.generation.saturating_add(1);
        self.document = Some(document);
        self.reflow();
        self.cursor = self
            .document
            .as_ref()
            .and_then(|document| (!document.tokens().is_empty()).then_some(0));
        self.preferred_x = self
            .cursor
            .and_then(|cursor| self.layout.placement(cursor).map(|placement| placement.x))
            .unwrap_or(0);
        self.viewport_top = 0;
        self.notice = None;
    }

    fn reflow(&mut self) {
        let width = reader_width(self.width, self.panel_visible);
        self.layout = self
            .document
            .as_ref()
            .map(|document| DocumentLayout::new(document, width))
            .unwrap_or_default();
    }

    fn reader_height(&self) -> usize {
        usize::from(self.height.saturating_sub(3).max(1))
    }

    fn ensure_cursor_visible(&mut self) {
        let Some(line) = self.cursor.and_then(|cursor| {
            self.layout
                .placement(cursor)
                .map(|placement| placement.line)
        }) else {
            return;
        };
        if line < self.viewport_top {
            self.viewport_top = line;
        } else {
            let height = self.reader_height();
            if line >= self.viewport_top.saturating_add(height) {
                self.viewport_top = line.saturating_add(1).saturating_sub(height);
            }
        }
    }
}

pub fn update(model: &mut Model, message: Message) -> Vec<Effect> {
    match message {
        Message::Input(input) => update_input(model, input),
        Message::Resize { width, height } => {
            model.width = width;
            model.height = height;
            model.panel_visible =
                width >= 100 && model.panel_preferred && model.mode != Mode::Vocabulary;
            model.reflow();
            model.ensure_cursor_visible();
            hydrate_visible(model, false)
        }
        Message::Tick(now) => {
            model.as_of = Some(now);
            let mut effects = vec![Effect::RefreshDueCount { as_of: now }];
            effects.extend(hydrate_visible(model, true));
            effects
        }
        Message::FileLoaded(result) => {
            match result {
                Ok(document) => {
                    model.set_document(document);
                    model.mode = Mode::Reader;
                }
                Err(error) => set_error(model, error),
            }
            hydrate_visible(model, false)
        }
        Message::WordsHydrated { generation, result } => {
            if generation == model.generation {
                match result {
                    Ok(details) => {
                        for detail in details {
                            model
                                .word_details
                                .insert(detail.surface.to_lowercase(), detail);
                        }
                    }
                    Err(error) => set_error(model, error),
                }
            }
            Vec::new()
        }
        Message::CaptureFinished(result) => {
            let mut effects = Vec::new();
            match result {
                Ok(feedback) => {
                    let message = if feedback.result.word_created {
                        format!("Captured {}", feedback.details.surface)
                    } else if feedback.result.contexts_created > 0 {
                        format!("Added context for {}", feedback.details.surface)
                    } else {
                        format!("{} is already captured", feedback.details.surface)
                    };
                    model
                        .word_details
                        .insert(feedback.details.surface.to_lowercase(), feedback.details);
                    model.notice = Some(Notice {
                        message,
                        is_error: false,
                    });
                    effects.push(Effect::RefreshDueCount {
                        as_of: feedback.captured_at,
                    });
                }
                Err(error) => set_error(model, error),
            }
            effects
        }
        Message::DueReviewsLoaded(result) => {
            match result {
                Ok(cards) if cards.is_empty() => {
                    model.mode = Mode::Reader;
                    model.review = None;
                    model.notice = Some(Notice {
                        message: "No reviews due".to_string(),
                        is_error: false,
                    });
                }
                Ok(cards) => {
                    model.review = Some(ReviewSession {
                        cards,
                        index: 0,
                        revealed: false,
                        saving: false,
                    });
                }
                Err(error) => {
                    model.mode = Mode::Reader;
                    model.review = None;
                    set_error(model, error);
                }
            }
            Vec::new()
        }
        Message::ReviewSaved {
            result,
            replacement,
            reviewed_at,
        } => review_saved(model, result, replacement, reviewed_at),
        Message::DueCountLoaded(result) => {
            match result {
                Ok(count) => model.due_count = Some(count),
                Err(error) => set_error(model, error),
            }
            Vec::new()
        }
    }
}

fn update_input(model: &mut Model, input: InputEvent) -> Vec<Effect> {
    model.as_of = Some(input.at);
    if input.kind == InputKind::Release {
        return Vec::new();
    }
    if input.key == AppKey::Ctrl('c') {
        model.should_quit = true;
        return Vec::new();
    }
    match model.mode {
        Mode::Reader => reader_input(model, input),
        Mode::OpenPath => path_input(model, input),
        Mode::Vocabulary => {
            if matches!(input.key, AppKey::Esc | AppKey::Char('q') | AppKey::Tab) {
                model.mode = Mode::Reader;
                model.panel_visible = model.width >= 100 && model.panel_preferred;
                model.reflow();
                model.ensure_cursor_visible();
            }
            Vec::new()
        }
        Mode::Review => review_input(model, input),
    }
}

fn reader_input(model: &mut Model, input: InputEvent) -> Vec<Effect> {
    let navigation = matches!(
        input.key,
        AppKey::Char('h' | 'j' | 'k' | 'l' | 'g' | 'G') | AppKey::Ctrl('u' | 'd')
    );
    if input.kind == InputKind::Repeat && !navigation {
        return Vec::new();
    }

    if model.pending_g && input.key != AppKey::Char('g') {
        model.pending_g = false;
    }
    match input.key {
        AppKey::Esc | AppKey::Char('q') => model.should_quit = true,
        AppKey::Char('h') => move_horizontal(model, -1),
        AppKey::Char('l') => move_horizontal(model, 1),
        AppKey::Char('j') => move_vertical(model, 1),
        AppKey::Char('k') => move_vertical(model, -1),
        AppKey::Ctrl('d') => move_page(model, 1),
        AppKey::Ctrl('u') => move_page(model, -1),
        AppKey::Char('G') => move_to_end(model),
        AppKey::Char('g') if model.pending_g => {
            model.pending_g = false;
            move_to_start(model);
        }
        AppKey::Char('g') => model.pending_g = true,
        AppKey::Char('c') if input.kind == InputKind::Press => {
            return capture_effect(model, input.at).into_iter().collect()
        }
        AppKey::Char('r') if input.kind == InputKind::Press => {
            model.mode = Mode::Review;
            model.review = None;
            return vec![Effect::LoadDueReviews { as_of: input.at }];
        }
        AppKey::Char('o') if input.kind == InputKind::Press => {
            model.mode = Mode::OpenPath;
            model.path_input = model
                .document
                .as_ref()
                .map(|document| document.path().display().to_string())
                .unwrap_or_default();
            model.path_cursor = model.path_input.len();
        }
        AppKey::Tab => {
            if model.width < 100 {
                model.mode = Mode::Vocabulary;
            } else {
                model.panel_preferred = !model.panel_preferred;
                model.panel_visible = model.panel_preferred;
                model.reflow();
                model.ensure_cursor_visible();
            }
        }
        _ => {}
    }
    hydrate_active(model)
}

fn path_input(model: &mut Model, input: InputEvent) -> Vec<Effect> {
    match input.key {
        AppKey::Esc => {
            if model.document.is_some() {
                model.mode = Mode::Reader;
            } else {
                model.should_quit = true;
            }
        }
        AppKey::Enter if !model.path_input.trim().is_empty() => {
            return vec![Effect::OpenFile(PathBuf::from(model.path_input.trim()))]
        }
        AppKey::Char(character) if !character.is_control() => {
            model.path_input.insert(model.path_cursor, character);
            model.path_cursor = model.path_cursor.saturating_add(character.len_utf8());
        }
        AppKey::Backspace if model.path_cursor > 0 => {
            let previous = model.path_input[..model.path_cursor]
                .char_indices()
                .next_back()
                .map(|(index, _)| index)
                .unwrap_or(0);
            model.path_input.drain(previous..model.path_cursor);
            model.path_cursor = previous;
        }
        AppKey::Delete if model.path_cursor < model.path_input.len() => {
            let next = model.path_input[model.path_cursor..]
                .chars()
                .next()
                .map(char::len_utf8)
                .unwrap_or(0);
            model
                .path_input
                .drain(model.path_cursor..model.path_cursor.saturating_add(next));
        }
        AppKey::Left => {
            model.path_cursor = model.path_input[..model.path_cursor]
                .char_indices()
                .next_back()
                .map(|(index, _)| index)
                .unwrap_or(0);
        }
        AppKey::Right if model.path_cursor < model.path_input.len() => {
            model.path_cursor = model.path_cursor.saturating_add(
                model.path_input[model.path_cursor..]
                    .chars()
                    .next()
                    .map(char::len_utf8)
                    .unwrap_or(0),
            );
        }
        AppKey::Home => model.path_cursor = 0,
        AppKey::End => model.path_cursor = model.path_input.len(),
        _ => {}
    }
    Vec::new()
}

fn review_input(model: &mut Model, input: InputEvent) -> Vec<Effect> {
    if matches!(input.key, AppKey::Esc | AppKey::Char('q')) {
        model.mode = Mode::Reader;
        model.review = None;
        return Vec::new();
    }
    let Some(review) = model.review.as_mut() else {
        return Vec::new();
    };
    if review.saving || input.kind != InputKind::Press {
        return Vec::new();
    }
    if matches!(input.key, AppKey::Enter | AppKey::Space) {
        review.revealed = true;
        return Vec::new();
    }
    let AppKey::Char(value @ '0'..='5') = input.key else {
        return Vec::new();
    };
    if !review.revealed {
        return Vec::new();
    }
    let Some(card) = review.cards.get(review.index) else {
        return Vec::new();
    };
    let rating_value = value
        .to_digit(10)
        .and_then(|value| u8::try_from(value).ok());
    let Some(rating) = rating_value.and_then(|value| ReviewRating::try_from(value).ok()) else {
        return Vec::new();
    };
    match schedule_review(&card.state, rating, input.at) {
        Ok(replacement) => {
            review.saving = true;
            vec![Effect::SaveReview {
                expected: card.state.clone(),
                replacement,
                reviewed_at: input.at,
            }]
        }
        Err(error) => {
            set_error(model, format!("failed to schedule review: {error}"));
            Vec::new()
        }
    }
}

fn review_saved(
    model: &mut Model,
    result: Result<ReviewUpdate, String>,
    replacement: ReviewState,
    reviewed_at: DateTime<Utc>,
) -> Vec<Effect> {
    match result {
        Ok(ReviewUpdate::Updated) => {
            for details in model.word_details.values_mut() {
                if details
                    .state
                    .as_ref()
                    .is_some_and(|state| state.word_id == replacement.word_id)
                {
                    details.state = Some(replacement.clone());
                }
            }
            advance_review(model);
            vec![Effect::RefreshDueCount { as_of: reviewed_at }]
        }
        Ok(ReviewUpdate::Conflict) => {
            let surface = model
                .review
                .as_ref()
                .and_then(|review| review.cards.get(review.index))
                .map(|card| card.word.term.clone());
            model.notice = Some(Notice {
                message: "Card changed in another process; skipped".to_string(),
                is_error: true,
            });
            advance_review(model);
            let mut effects = vec![Effect::RefreshDueCount { as_of: reviewed_at }];
            if let Some(surface) = surface {
                effects.push(Effect::HydrateWords {
                    generation: model.generation,
                    surfaces: vec![surface],
                });
            }
            effects
        }
        Err(error) => {
            if let Some(review) = model.review.as_mut() {
                review.saving = false;
            }
            set_error(model, error);
            Vec::new()
        }
    }
}

fn advance_review(model: &mut Model) {
    if let Some(review) = model.review.as_mut() {
        review.index = review.index.saturating_add(1);
        review.revealed = false;
        review.saving = false;
        if review.index >= review.cards.len() {
            model.review = None;
            model.mode = Mode::Reader;
        }
    }
}

fn capture_effect(model: &Model, captured_at: DateTime<Utc>) -> Option<Effect> {
    let token = model.active_token()?;
    let document = model.document.as_ref()?;
    Some(Effect::Capture {
        surface: token.surface.clone(),
        context: token.sentence.clone(),
        source: format!("tui:{}", document.path().display()),
        captured_at,
    })
}

fn move_horizontal(model: &mut Model, direction: i8) {
    let Some(document) = model.document.as_ref() else {
        return;
    };
    let count = document.tokens().len();
    if count == 0 {
        return;
    }
    let current = model.cursor.unwrap_or(0);
    model.cursor = Some(if direction < 0 {
        current.saturating_sub(1)
    } else {
        current.saturating_add(1).min(count.saturating_sub(1))
    });
    if let Some(placement) = model
        .cursor
        .and_then(|cursor| model.layout.placement(cursor))
    {
        model.preferred_x = placement.x;
    }
    model.ensure_cursor_visible();
}

fn move_vertical(model: &mut Model, direction: i8) {
    let Some(current) = model
        .cursor
        .and_then(|cursor| model.layout.placement(cursor))
    else {
        return;
    };
    let mut line = current.line;
    loop {
        line = if direction < 0 {
            let next = line.saturating_sub(1);
            if next == line {
                return;
            }
            next
        } else {
            let next = line.saturating_add(1);
            if next >= model.layout.lines().len() {
                return;
            }
            next
        };
        if let Some(token) = model.layout.nearest_token_on_line(line, model.preferred_x) {
            model.cursor = Some(token);
            model.ensure_cursor_visible();
            return;
        }
    }
}

fn move_page(model: &mut Model, direction: i8) {
    let Some(current) = model
        .cursor
        .and_then(|cursor| model.layout.placement(cursor))
    else {
        return;
    };
    let delta = model.reader_height().saturating_div(2).max(1);
    let target = if direction < 0 {
        current.line.saturating_sub(delta)
    } else {
        current
            .line
            .saturating_add(delta)
            .min(model.layout.lines().len().saturating_sub(1))
    };
    if let Some(token) = nearest_token_around(&model.layout, target, model.preferred_x, direction) {
        model.cursor = Some(token);
        model.ensure_cursor_visible();
    }
}

fn nearest_token_around(
    layout: &DocumentLayout,
    target: usize,
    preferred_x: u16,
    direction: i8,
) -> Option<usize> {
    let mut line = target;
    loop {
        if let Some(token) = layout.nearest_token_on_line(line, preferred_x) {
            return Some(token);
        }
        if direction < 0 {
            if line == 0 {
                return None;
            }
            line = line.saturating_sub(1);
        } else {
            line = line.saturating_add(1);
            if line >= layout.lines().len() {
                return None;
            }
        }
    }
}

fn move_to_start(model: &mut Model) {
    if model
        .document
        .as_ref()
        .is_some_and(|document| !document.tokens().is_empty())
    {
        model.cursor = Some(0);
        model.preferred_x = 0;
        model.ensure_cursor_visible();
    }
}

fn move_to_end(model: &mut Model) {
    if let Some(last) = model
        .document
        .as_ref()
        .and_then(|document| document.tokens().len().checked_sub(1))
    {
        model.cursor = Some(last);
        if let Some(placement) = model.layout.placement(last) {
            model.preferred_x = placement.x;
        }
        model.ensure_cursor_visible();
    }
}

fn hydrate_active(model: &Model) -> Vec<Effect> {
    let Some(token) = model.active_token() else {
        return Vec::new();
    };
    let key = token.surface.to_lowercase();
    if model.word_details.contains_key(&key) {
        return Vec::new();
    }
    vec![Effect::HydrateWords {
        generation: model.generation,
        surfaces: vec![token.surface.clone()],
    }]
}

fn hydrate_visible(model: &Model, force: bool) -> Vec<Effect> {
    let Some(document) = model.document.as_ref() else {
        return Vec::new();
    };
    let bottom = model.viewport_top.saturating_add(model.reader_height());
    let mut surfaces = Vec::new();
    for placement in model.layout.placements() {
        if placement.line < model.viewport_top || placement.line >= bottom {
            continue;
        }
        let Some(token) = document.tokens().get(placement.token_index) else {
            continue;
        };
        let key = token.surface.to_lowercase();
        if (force || !model.word_details.contains_key(&key))
            && !surfaces
                .iter()
                .any(|surface: &String| surface.to_lowercase() == key)
        {
            surfaces.push(token.surface.clone());
        }
        if surfaces.len() >= 64 {
            break;
        }
    }
    if surfaces.is_empty() {
        Vec::new()
    } else {
        vec![Effect::HydrateWords {
            generation: model.generation,
            surfaces,
        }]
    }
}

fn reader_width(width: u16, panel_visible: bool) -> u16 {
    let pane_width = if width >= 100 && panel_visible {
        width.saturating_sub(35)
    } else {
        width
    };
    pane_width.saturating_sub(2)
}

fn set_error(model: &mut Model, message: String) {
    model.notice = Some(Notice {
        message,
        is_error: true,
    });
}
