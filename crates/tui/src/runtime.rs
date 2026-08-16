use std::collections::VecDeque;
use std::io::{self, IsTerminal, Stdout};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use core_engine::StorageAdapter;
use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use language_engine::{capture_from_entry, word_id_for_lemma, Lexicon};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use thiserror::Error;

use crate::{
    render, update, AppKey, CaptureFeedback, Document, DocumentFormat, Effect, InputEvent,
    InputKind, Message, Model, WordDetails,
};

const EVENT_POLL_INTERVAL: Duration = Duration::from_millis(100);
const DUE_REFRESH_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TuiConfig {
    pub initial_file: Option<PathBuf>,
    pub working_directory: PathBuf,
}

#[derive(Debug, Error)]
pub enum TuiError {
    #[error("the TUI requires interactive standard input and standard output")]
    NotTerminal,
    #[error("failed to load the initial reader file: {0}")]
    InitialFile(String),
    #[error("terminal {operation} failed")]
    Terminal {
        operation: &'static str,
        #[source]
        source: io::Error,
    },
}

pub trait TerminalControl {
    type Error;

    fn enable_raw(&mut self) -> Result<(), Self::Error>;
    fn enter_alternate(&mut self) -> Result<(), Self::Error>;
    fn hide_cursor(&mut self) -> Result<(), Self::Error>;
    fn show_cursor(&mut self) -> Result<(), Self::Error>;
    fn leave_alternate(&mut self) -> Result<(), Self::Error>;
    fn disable_raw(&mut self) -> Result<(), Self::Error>;
}

pub struct TerminalGuard<C: TerminalControl> {
    control: C,
    raw_enabled: bool,
    alternate_entered: bool,
    cursor_hidden: bool,
}

impl<C: TerminalControl> TerminalGuard<C> {
    pub fn enter(control: C) -> Result<Self, C::Error> {
        let mut guard = Self {
            control,
            raw_enabled: false,
            alternate_entered: false,
            cursor_hidden: false,
        };
        guard.control.enable_raw()?;
        guard.raw_enabled = true;
        if let Err(error) = guard.control.enter_alternate() {
            guard.restore_best_effort();
            return Err(error);
        }
        guard.alternate_entered = true;
        if let Err(error) = guard.control.hide_cursor() {
            guard.restore_best_effort();
            return Err(error);
        }
        guard.cursor_hidden = true;
        Ok(guard)
    }

    pub fn finish(mut self) -> Result<(), C::Error> {
        self.restore()
    }

    fn restore(&mut self) -> Result<(), C::Error> {
        let mut first_error = None;
        if self.cursor_hidden {
            if let Err(error) = self.control.show_cursor() {
                first_error = Some(error);
            }
            self.cursor_hidden = false;
        }
        if self.alternate_entered {
            if let Err(error) = self.control.leave_alternate() {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
            self.alternate_entered = false;
        }
        if self.raw_enabled {
            if let Err(error) = self.control.disable_raw() {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
            self.raw_enabled = false;
        }
        first_error.map_or(Ok(()), Err)
    }

    fn restore_best_effort(&mut self) {
        let _ = self.restore();
    }
}

impl<C: TerminalControl> Drop for TerminalGuard<C> {
    fn drop(&mut self) {
        self.restore_best_effort();
    }
}

struct CrosstermControl {
    output: Stdout,
}

impl CrosstermControl {
    fn new() -> Self {
        Self {
            output: io::stdout(),
        }
    }
}

impl TerminalControl for CrosstermControl {
    type Error = io::Error;

    fn enable_raw(&mut self) -> Result<(), Self::Error> {
        enable_raw_mode()
    }

    fn enter_alternate(&mut self) -> Result<(), Self::Error> {
        execute!(self.output, EnterAlternateScreen)
    }

    fn hide_cursor(&mut self) -> Result<(), Self::Error> {
        execute!(self.output, Hide)
    }

    fn show_cursor(&mut self) -> Result<(), Self::Error> {
        execute!(self.output, Show)
    }

    fn leave_alternate(&mut self) -> Result<(), Self::Error> {
        execute!(self.output, LeaveAlternateScreen)
    }

    fn disable_raw(&mut self) -> Result<(), Self::Error> {
        disable_raw_mode()
    }
}

pub fn run<S, L, C>(
    config: TuiConfig,
    storage: &mut S,
    lexicon: &L,
    clock: C,
) -> Result<(), TuiError>
where
    S: StorageAdapter,
    L: Lexicon,
    C: Fn() -> DateTime<Utc>,
{
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(TuiError::NotTerminal);
    }
    let initial_document = config
        .initial_file
        .as_ref()
        .map(|path| load_document(path, &config.working_directory))
        .transpose()
        .map_err(TuiError::InitialFile)?;

    let _panic_hook = PanicHookGuard::install();
    let session =
        TerminalGuard::enter(CrosstermControl::new()).map_err(|source| TuiError::Terminal {
            operation: "setup",
            source,
        })?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend).map_err(|source| TuiError::Terminal {
        operation: "initialization",
        source,
    })?;
    let area = terminal.size().map_err(|source| TuiError::Terminal {
        operation: "size query",
        source,
    })?;
    let mut model = initial_document.map_or_else(
        || Model::empty(area.width, area.height),
        |document| Model::with_document(document, area.width, area.height),
    );
    let mut effects = update(&mut model, Message::Tick(clock()));
    process_effects(
        &mut model,
        &mut effects,
        storage,
        lexicon,
        &config.working_directory,
    );
    let mut last_due_refresh = Instant::now();

    while !model.should_quit() {
        terminal
            .draw(|frame| render(frame, &model))
            .map_err(|source| TuiError::Terminal {
                operation: "render",
                source,
            })?;
        if event::poll(EVENT_POLL_INTERVAL).map_err(|source| TuiError::Terminal {
            operation: "event poll",
            source,
        })? {
            let event = event::read().map_err(|source| TuiError::Terminal {
                operation: "event read",
                source,
            })?;
            if let Some(message) = event_message(event, clock()) {
                effects.extend(update(&mut model, message));
            }
        }
        if last_due_refresh.elapsed() >= DUE_REFRESH_INTERVAL {
            effects.extend(update(&mut model, Message::Tick(clock())));
            last_due_refresh = Instant::now();
        }
        process_effects(
            &mut model,
            &mut effects,
            storage,
            lexicon,
            &config.working_directory,
        );
    }

    drop(terminal);
    session.finish().map_err(|source| TuiError::Terminal {
        operation: "restoration",
        source,
    })
}

fn process_effects<S, L>(
    model: &mut Model,
    effects: &mut Vec<Effect>,
    storage: &mut S,
    lexicon: &L,
    working_directory: &Path,
) where
    S: StorageAdapter,
    L: Lexicon,
{
    let mut pending: VecDeque<Effect> = effects.drain(..).collect();
    while let Some(effect) = pending.pop_front() {
        let message = execute_effect(effect, storage, lexicon, working_directory);
        pending.extend(update(model, message));
    }
}

pub fn execute_effect<S, L>(
    effect: Effect,
    storage: &mut S,
    lexicon: &L,
    working_directory: &Path,
) -> Message
where
    S: StorageAdapter,
    L: Lexicon,
{
    match effect {
        Effect::OpenFile(path) => Message::FileLoaded(load_document(&path, working_directory)),
        Effect::HydrateWords {
            generation,
            surfaces,
        } => Message::WordsHydrated {
            generation,
            result: hydrate_words(&surfaces, storage, lexicon),
        },
        Effect::Capture {
            surface,
            context,
            source,
            captured_at,
        } => Message::CaptureFinished(capture_word(
            &surface,
            &context,
            &source,
            captured_at,
            storage,
            lexicon,
        )),
        Effect::LoadDueReviews { as_of } => Message::DueReviewsLoaded(
            storage
                .due_reviews(as_of)
                .map_err(|error| format!("failed to query due reviews: {error}")),
        ),
        Effect::SaveReview {
            expected,
            replacement,
            reviewed_at,
        } => Message::ReviewSaved {
            result: storage
                .commit_review(&expected, &replacement, reviewed_at)
                .map_err(|error| format!("failed to save review: {error}")),
            replacement,
            reviewed_at,
        },
        Effect::RefreshDueCount { as_of } => Message::DueCountLoaded(
            storage
                .due_count(as_of)
                .map_err(|error| format!("failed to refresh due count: {error}")),
        ),
    }
}

fn hydrate_words<S, L>(
    surfaces: &[String],
    storage: &S,
    lexicon: &L,
) -> Result<Vec<WordDetails>, String>
where
    S: StorageAdapter,
    L: Lexicon,
{
    surfaces
        .iter()
        .map(|surface| {
            let entry = lexicon
                .lookup(surface)
                .map_err(|error| format!("offline dictionary lookup failed: {error}"))?;
            let state = match entry.as_ref() {
                Some(entry) => storage
                    .review_state(&word_id_for_lemma(&entry.lemma))
                    .map_err(|error| format!("failed to query review state: {error}"))?,
                None => None,
            };
            Ok(WordDetails {
                surface: surface.clone(),
                entry,
                state,
            })
        })
        .collect()
}

fn capture_word<S, L>(
    surface: &str,
    context: &str,
    source: &str,
    captured_at: DateTime<Utc>,
    storage: &mut S,
    lexicon: &L,
) -> Result<CaptureFeedback, String>
where
    S: StorageAdapter,
    L: Lexicon,
{
    let entry = lexicon
        .lookup(surface)
        .map_err(|error| format!("offline dictionary lookup failed: {error}"))?
        .ok_or_else(|| format!("{surface:?} was not found in the offline dictionary"))?;
    let capture = capture_from_entry(surface, Some(context), source, entry.clone(), captured_at);
    let result = storage
        .save_capture(&capture)
        .map_err(|error| format!("failed to save capture: {error}"))?;
    let state = storage
        .review_state(&capture.word.id)
        .map_err(|error| format!("failed to refresh captured review state: {error}"))?;
    Ok(CaptureFeedback {
        result,
        details: WordDetails {
            surface: surface.to_string(),
            entry: Some(entry),
            state,
        },
        captured_at,
    })
}

fn load_document(path: &Path, working_directory: &Path) -> Result<Document, String> {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        working_directory.join(path)
    };
    let canonical = path
        .canonicalize()
        .map_err(|error| format!("failed to resolve {}: {error}", path.display()))?;
    let metadata = canonical
        .metadata()
        .map_err(|error| format!("failed to inspect {}: {error}", canonical.display()))?;
    if !metadata.is_file() {
        return Err(format!("{} is not a regular file", canonical.display()));
    }
    let source = std::fs::read_to_string(&canonical)
        .map_err(|error| format!("failed to read {} as UTF-8: {error}", canonical.display()))?;
    let format = DocumentFormat::from_path(&canonical);
    Ok(Document::parse(canonical, format, &source))
}

fn event_message(event: Event, at: DateTime<Utc>) -> Option<Message> {
    match event {
        Event::Key(key) => key_input(key, at).map(Message::Input),
        Event::Resize(width, height) => Some(Message::Resize { width, height }),
        _ => None,
    }
}

fn key_input(key: KeyEvent, at: DateTime<Utc>) -> Option<InputEvent> {
    let kind = match key.kind {
        KeyEventKind::Press => InputKind::Press,
        KeyEventKind::Repeat => InputKind::Repeat,
        KeyEventKind::Release => InputKind::Release,
    };
    let app_key = match key.code {
        KeyCode::Char(character) if key.modifiers.contains(KeyModifiers::CONTROL) => {
            AppKey::Ctrl(character.to_ascii_lowercase())
        }
        KeyCode::Char(' ') => AppKey::Space,
        KeyCode::Char(character) => AppKey::Char(character),
        KeyCode::Enter => AppKey::Enter,
        KeyCode::Esc => AppKey::Esc,
        KeyCode::Tab | KeyCode::BackTab => AppKey::Tab,
        KeyCode::Backspace => AppKey::Backspace,
        KeyCode::Delete => AppKey::Delete,
        KeyCode::Left => AppKey::Left,
        KeyCode::Right => AppKey::Right,
        KeyCode::Home => AppKey::Home,
        KeyCode::End => AppKey::End,
        _ => return None,
    };
    Some(InputEvent {
        key: app_key,
        kind,
        at,
    })
}

type Hook = dyn for<'a> Fn(&std::panic::PanicHookInfo<'a>) + Send + Sync + 'static;

struct PanicHookGuard {
    previous: Arc<Mutex<Option<Box<Hook>>>>,
}

impl PanicHookGuard {
    fn install() -> Self {
        let previous = Arc::new(Mutex::new(Some(std::panic::take_hook())));
        let hook_previous = previous.clone();
        std::panic::set_hook(Box::new(move |info| {
            restore_terminal_after_panic();
            if let Ok(previous) = hook_previous.lock() {
                if let Some(previous) = previous.as_ref() {
                    previous(info);
                }
            }
        }));
        Self { previous }
    }
}

impl Drop for PanicHookGuard {
    fn drop(&mut self) {
        if std::thread::panicking() {
            return;
        }
        if let Ok(mut previous) = self.previous.lock() {
            if let Some(previous) = previous.take() {
                std::panic::set_hook(previous);
            }
        }
    }
}

fn restore_terminal_after_panic() {
    let _ = disable_raw_mode();
    let mut output = io::stdout();
    let _ = execute!(output, Show, LeaveAlternateScreen);
}
