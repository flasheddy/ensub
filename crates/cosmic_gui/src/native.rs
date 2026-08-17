use std::path::PathBuf;

use chrono::Utc;
use core_engine::{
    LibraryOrder, LibraryQuery, LibraryStorageAdapter, ReviewHistoryQuery, ReviewRating,
    ReviewUpdate, StorageAdapter,
};
use cosmic::app::{Core, Settings, Task};
use cosmic::iced::{clipboard, event, keyboard, Length, Size, Subscription};
use cosmic::{executor, widget, ApplicationExt, Element};
use ensub_sqlite::{BundledLexicon, SqliteStorage};
use ensub_theme::Theme;
use language_engine::{capture_from_candidate, extract_candidates, ParseOptions};

use crate::{
    to_cosmic_theme, update, DashboardData, Effect, GlobalShortcut, KeyEventKind,
    Message as DomainMessage, Model, Page, ReaderEffect, ReaderKey, ReaderMessage, ReaderShortcut,
    ReviewPhase,
};

mod reader;

#[derive(Debug, Clone)]
pub struct GuiFlags {
    pub database_path: PathBuf,
    pub lexicon_cache_dir: PathBuf,
    pub capture_mode: bool,
}

pub fn run(flags: GuiFlags) -> cosmic::iced::Result {
    run_with_theme(flags, Theme::default())
}

pub fn run_with_theme(flags: GuiFlags, theme: Theme) -> cosmic::iced::Result {
    let size = if flags.capture_mode {
        Size::new(560.0, 420.0)
    } else {
        Size::new(1180.0, 760.0)
    };
    let settings = Settings::default().theme(to_cosmic_theme(theme)).size(size);
    if flags.capture_mode {
        cosmic::app::run::<HudApplication>(settings, flags)
    } else {
        cosmic::app::run::<NativeApp>(settings, flags)
    }
}

#[derive(Debug, Clone)]
pub enum HostMessage {
    Navigate(Page),
    Domain(DomainMessage),
    SearchChanged(String),
    SortLibrary(LibraryOrder),
    PageLibrary(i64),
    SelectLibrary(usize),
    Reader(ReaderMessage),
    ManualTextChanged(String),
    ParseManual,
    Parsed(Result<Vec<language_engine::Candidate>, String>),
    ToggleCandidate(usize, bool),
    CaptureCandidates,
    Captured(Result<u64, String>),
    Clipboard(Option<String>),
    CancelCapture,
    Reveal,
    SkipReview,
    Rate(u8),
    ClearError,
    KeyPressed {
        key: keyboard::Key,
        modifiers: keyboard::Modifiers,
        repeat: bool,
        captured: bool,
    },
}

pub struct NativeApp {
    core: Core,
    flags: GuiFlags,
    model: Model,
    search: String,
    library_order: LibraryOrder,
    library_offset: u64,
    selected_library: usize,
    manual_text: String,
    candidates: Vec<language_engine::Candidate>,
    selected_candidates: Vec<bool>,
    capture_status: Option<String>,
}

struct HudApplication(NativeApp);

impl cosmic::Application for HudApplication {
    type Executor = executor::Default;
    type Flags = GuiFlags;
    type Message = HostMessage;

    const APP_ID: &'static str = "dev.ensub.Ensub.Hud";

    fn core(&self) -> &Core {
        self.0.core()
    }

    fn core_mut(&mut self) -> &mut Core {
        self.0.core_mut()
    }

    fn init(core: Core, mut flags: Self::Flags) -> (Self, Task<Self::Message>) {
        flags.capture_mode = true;
        let (app, task) = <NativeApp as cosmic::Application>::init(core, flags);
        (Self(app), task)
    }

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        self.0.update(message)
    }

    fn view(&self) -> Element<'_, Self::Message> {
        self.0.view()
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        self.0.subscription()
    }
}

impl cosmic::Application for NativeApp {
    type Executor = executor::Default;
    type Flags = GuiFlags;
    type Message = HostMessage;

    const APP_ID: &'static str = "dev.ensub.Ensub";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(mut core: Core, flags: Self::Flags) -> (Self, Task<Self::Message>) {
        let capture_mode = flags.capture_mode;
        core.set_keyboard_nav(false);
        let mut app = Self {
            core,
            flags,
            model: Model::new(current_time()),
            search: String::new(),
            library_order: LibraryOrder::RecentlyCaptured,
            library_offset: 0,
            selected_library: 0,
            manual_text: String::new(),
            candidates: Vec::new(),
            selected_candidates: Vec::new(),
            capture_status: None,
        };
        app.set_header_title(
            if capture_mode {
                "Capture word"
            } else {
                "Ensub"
            }
            .to_string(),
        );
        if capture_mode {
            (
                app,
                clipboard::read()
                    .map(HostMessage::Clipboard)
                    .map(cosmic::Action::App),
            )
        } else {
            let task = app.apply_domain(DomainMessage::Navigate {
                page: Page::Dashboard,
                as_of: current_time(),
            });
            (app, task)
        }
    }

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            HostMessage::Navigate(page) => self.apply_domain(DomainMessage::Navigate {
                page,
                as_of: current_time(),
            }),
            HostMessage::Domain(message) => self.apply_domain(message),
            HostMessage::SearchChanged(search) => {
                self.search = search;
                self.library_offset = 0;
                self.model.library_generation = self.model.library_generation.saturating_add(1);
                self.run_effect(Effect::LoadLibrary {
                    generation: self.model.library_generation,
                })
            }
            HostMessage::SortLibrary(order) => {
                self.library_order = order;
                self.library_offset = 0;
                self.model.library_generation = self.model.library_generation.saturating_add(1);
                self.run_effect(Effect::LoadLibrary {
                    generation: self.model.library_generation,
                })
            }
            HostMessage::PageLibrary(delta) => {
                self.library_offset = if delta < 0 {
                    self.library_offset.saturating_sub(50)
                } else {
                    let proposed = self.library_offset.saturating_add(50);
                    self.model.library.as_ref().map_or(proposed, |page| {
                        let last_page = page.total.saturating_sub(1) / 50 * 50;
                        proposed.min(last_page)
                    })
                };
                self.selected_library = 0;
                self.model.library_generation = self.model.library_generation.saturating_add(1);
                self.run_effect(Effect::LoadLibrary {
                    generation: self.model.library_generation,
                })
            }
            HostMessage::SelectLibrary(index) => {
                self.selected_library = index;
                Task::none()
            }
            HostMessage::Reader(message) => self.apply_domain(DomainMessage::Reader(message)),
            HostMessage::ManualTextChanged(text) => {
                self.manual_text = text;
                Task::none()
            }
            HostMessage::ParseManual => self.parse_task(),
            HostMessage::Parsed(result) => {
                match result {
                    Ok(candidates) => {
                        self.capture_status = Some(format!("{} candidates", candidates.len()));
                        self.selected_candidates = vec![true; candidates.len()];
                        self.candidates = candidates;
                    }
                    Err(error) => self.model.error = Some(error),
                }
                Task::none()
            }
            HostMessage::ToggleCandidate(index, selected) => {
                if let Some(value) = self.selected_candidates.get_mut(index) {
                    *value = selected;
                }
                Task::none()
            }
            HostMessage::CaptureCandidates => self.capture_task(),
            HostMessage::Captured(result) => {
                let close = self.flags.capture_mode && result.is_ok();
                match result {
                    Ok(count) => {
                        self.capture_status = Some(format!("Captured {count} words"));
                        self.candidates.clear();
                        self.selected_candidates.clear();
                    }
                    Err(error) => self.model.error = Some(error),
                }
                if close {
                    self.close_main_window()
                } else {
                    Task::none()
                }
            }
            HostMessage::Clipboard(contents) => {
                self.manual_text = contents.unwrap_or_default();
                Task::none()
            }
            HostMessage::CancelCapture => self.close_main_window(),
            HostMessage::Reveal => self.apply_domain(DomainMessage::RevealReview),
            HostMessage::SkipReview => self.apply_domain(DomainMessage::SkipReview),
            HostMessage::Rate(value) => match ReviewRating::try_from(value) {
                Ok(rating) => self.apply_domain(DomainMessage::Rate(rating, current_time())),
                Err(error) => {
                    self.model.error = Some(error.to_string());
                    Task::none()
                }
            },
            HostMessage::ClearError => {
                self.model.error = None;
                Task::none()
            }
            HostMessage::KeyPressed {
                key,
                modifiers,
                repeat,
                captured,
            } => self.handle_key(key, modifiers, repeat, captured),
        }
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        event::listen_with(|event, status, _window| match event {
            cosmic::iced::Event::Keyboard(keyboard::Event::KeyPressed {
                key,
                modifiers,
                repeat,
                ..
            }) => Some(HostMessage::KeyPressed {
                key,
                modifiers,
                repeat,
                captured: status == event::Status::Captured,
            }),
            _ => None,
        })
    }

    fn view(&self) -> Element<'_, Self::Message> {
        if self.flags.capture_mode {
            return self.capture_view();
        }
        let navigation = [
            ("Dashboard", Page::Dashboard),
            ("Library", Page::Library),
            ("Reader", Page::Reader),
            ("Parse Text", Page::ParseText),
            ("Review", Page::Review),
        ]
        .into_iter()
        .fold(
            widget::row::with_capacity(5).spacing(8),
            |row, (label, page)| {
                let button = if self.model.page == page {
                    widget::button::suggested(label)
                } else {
                    widget::button::text(label)
                };
                row.push(button.on_press(HostMessage::Navigate(page)))
            },
        );
        let mut content = widget::column::with_capacity(4)
            .spacing(12)
            .push(navigation);
        if let Some(error) = self.model.error.as_deref() {
            content = content.push(widget::warning(error).on_close(HostMessage::ClearError));
        }
        content
            .push(self.page_view())
            .padding(16)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

impl NativeApp {
    fn apply_domain(&mut self, message: DomainMessage) -> Task<HostMessage> {
        let effects = update(&mut self.model, message);
        Task::batch(effects.into_iter().map(|effect| self.run_effect(effect)))
    }

    fn run_effect(&self, effect: Effect) -> Task<HostMessage> {
        if let Effect::Reader(reader_effect) = &effect {
            return self.run_reader_effect(reader_effect.clone());
        }
        let database_path = self.flags.database_path.clone();
        let search = self.search.clone();
        let library_order = self.library_order;
        let library_offset = self.library_offset;
        cosmic::task::future(async move {
            let failure_effect = effect.clone();
            let outcome = tokio::task::spawn_blocking(move || -> DomainMessage {
                let mut storage = match SqliteStorage::open(&database_path) {
                    Ok(storage) => storage,
                    Err(error) => return effect_error(effect, error.to_string()),
                };
                match effect {
                    Effect::LoadDashboard { as_of } => {
                        let result = (|| {
                            let statistics = storage.review_statistics(as_of)?;
                            let from = as_of
                                .checked_sub_signed(chrono::TimeDelta::days(30))
                                .unwrap_or(as_of);
                            let activity = storage.review_activity(from, as_of)?;
                            let history = storage.review_history(&ReviewHistoryQuery {
                                limit: 10,
                                ..ReviewHistoryQuery::default()
                            })?;
                            Ok::<_, ensub_sqlite::SqliteError>(DashboardData {
                                statistics,
                                activity,
                                history,
                            })
                        })()
                        .map_err(|error| error.to_string());
                        DomainMessage::DashboardLoaded(result)
                    }
                    Effect::LoadLibrary { generation } => DomainMessage::LibraryLoaded {
                        generation,
                        result: storage
                            .library_page(&LibraryQuery {
                                search,
                                order: library_order,
                                offset: library_offset,
                                ..LibraryQuery::default()
                            })
                            .map_err(|error| error.to_string()),
                    },
                    Effect::LoadReviewBatch { as_of } => DomainMessage::ReviewBatchLoaded(
                        storage
                            .due_review_batch(as_of, 50)
                            .map_err(|error| error.to_string()),
                    ),
                    Effect::CommitReview {
                        expected,
                        replacement,
                        reviewed_at,
                    } => {
                        let result = storage
                            .commit_review(&expected, &replacement, reviewed_at)
                            .map(|update| update == ReviewUpdate::Updated)
                            .map_err(|error| error.to_string());
                        DomainMessage::ReviewCommitted(result, reviewed_at)
                    }
                    Effect::Reader(reader_effect) => effect_error(
                        Effect::Reader(reader_effect),
                        "reader effect reached the storage executor".to_string(),
                    ),
                }
            })
            .await;
            HostMessage::Domain(match outcome {
                Ok(message) => message,
                Err(error) => effect_error(failure_effect, error.to_string()),
            })
        })
    }

    fn parse_task(&self) -> Task<HostMessage> {
        let text = self.manual_text.clone();
        let cache = self.flags.lexicon_cache_dir.clone();
        cosmic::task::future(async move {
            let result = tokio::task::spawn_blocking(move || {
                let lexicon = BundledLexicon::open(cache).map_err(|error| error.to_string())?;
                extract_candidates(&text, &lexicon, ParseOptions::default())
                    .map(|report| report.candidates)
                    .map_err(|error| error.to_string())
            })
            .await
            .map_err(|error| error.to_string())
            .and_then(|result| result);
            HostMessage::Parsed(result)
        })
    }

    fn capture_task(&self) -> Task<HostMessage> {
        let captures = self
            .candidates
            .iter()
            .zip(&self.selected_candidates)
            .filter(|(_, selected)| **selected)
            .map(|(candidate, _)| {
                capture_from_candidate(
                    candidate,
                    if self.flags.capture_mode {
                        "hud:clipboard"
                    } else {
                        "gui:manual"
                    },
                    current_time(),
                )
            })
            .collect::<Vec<_>>();
        if captures.is_empty() {
            return cosmic::task::message(cosmic::Action::App(HostMessage::Captured(Err(
                "select at least one candidate".to_string(),
            ))));
        }
        let database_path = self.flags.database_path.clone();
        cosmic::task::future(async move {
            let result = tokio::task::spawn_blocking(move || {
                let mut storage =
                    SqliteStorage::open(database_path).map_err(|error| error.to_string())?;
                storage
                    .save_captures(&captures)
                    .map(|results| results.len() as u64)
                    .map_err(|error| error.to_string())
            })
            .await
            .map_err(|error| error.to_string())
            .and_then(|result| result);
            HostMessage::Captured(result)
        })
    }

    fn page_view(&self) -> Element<'_, HostMessage> {
        match self.model.page {
            Page::Dashboard => {
                let Some(dashboard) = self.model.dashboard.as_ref() else {
                    return widget::text::body("Loading dashboard...").into();
                };
                let stats = dashboard.statistics;
                let pass_rate = if dashboard.activity.total_reviews == 0 {
                    0.0
                } else {
                    dashboard.activity.passing_reviews as f64 * 100.0
                        / dashboard.activity.total_reviews as f64
                };
                let mut column = widget::column::with_capacity(16).spacing(8).push(
                    widget::text::body(format!("Due: {}    Total: {}\nNew: {}    1-6 days: {}    7-30 days: {}    31-90 days: {}    91+ days: {}", stats.due_cards, stats.total_cards, stats.intervals.new, stats.intervals.days_1_to_6, stats.intervals.days_7_to_30, stats.intervals.days_31_to_90, stats.intervals.days_91_plus)),
                ).push(widget::text::title4("Last 30 UTC days")).push(
                    widget::text::body(format!("Reviews: {}    Pass rate: {:.1}%\nRatings 0-5: {} / {} / {} / {} / {} / {}", dashboard.activity.total_reviews, pass_rate, dashboard.activity.ratings[0], dashboard.activity.ratings[1], dashboard.activity.ratings[2], dashboard.activity.ratings[3], dashboard.activity.ratings[4], dashboard.activity.ratings[5])),
                ).push(widget::text::title4("Recent reviews"));
                for entry in &dashboard.history.entries {
                    column = column.push(widget::text::body(format!(
                        "{}    rating {}    {}",
                        entry.word.lemma,
                        entry.rating.value(),
                        entry.reviewed_at.format("%Y-%m-%d %H:%M UTC")
                    )));
                }
                widget::scrollable(column).height(Length::Fill).into()
            }
            Page::Library => {
                let controls = widget::column::with_capacity(3)
                    .spacing(8)
                    .push(
                        widget::text_input::search_input("Search vocabulary", &self.search)
                            .on_input(HostMessage::SearchChanged),
                    )
                    .push(
                        widget::row::with_capacity(3)
                            .spacing(6)
                            .push(
                                widget::button::text("Recent").on_press(HostMessage::SortLibrary(
                                    LibraryOrder::RecentlyCaptured,
                                )),
                            )
                            .push(
                                widget::button::text("A-Z")
                                    .on_press(HostMessage::SortLibrary(LibraryOrder::Alphabetical)),
                            )
                            .push(
                                widget::button::text("Due first")
                                    .on_press(HostMessage::SortLibrary(LibraryOrder::DueFirst)),
                            ),
                    )
                    .push(
                        widget::row::with_capacity(2)
                            .spacing(6)
                            .push(
                                widget::button::text("Previous")
                                    .on_press(HostMessage::PageLibrary(-1)),
                            )
                            .push(
                                widget::button::text("Next").on_press(HostMessage::PageLibrary(1)),
                            ),
                    );
                let mut list = widget::column::with_capacity(52).spacing(8).push(controls);
                if let Some(page) = &self.model.library {
                    list = list.push(widget::text::caption(format!(
                        "{} words - showing {}-{}",
                        page.total,
                        if page.cards.is_empty() {
                            0
                        } else {
                            page.offset.saturating_add(1)
                        },
                        page.offset.saturating_add(page.cards.len() as u64)
                    )));
                    for (index, card) in page.cards.iter().enumerate() {
                        list = list.push(
                            widget::button::text(format!(
                                "{}  /{}/  - {} days",
                                card.word.lemma, card.word.phonetic, card.state.interval_days
                            ))
                            .on_press(HostMessage::SelectLibrary(index)),
                        );
                    }
                }
                let rail: Element<'_, HostMessage> = self
                    .model
                    .library
                    .as_ref()
                    .and_then(|page| page.cards.get(self.selected_library))
                    .map_or_else(
                        || widget::text::body("Select a word").into(),
                        |card| {
                            let mut details = widget::column::with_capacity(12)
                                .spacing(8)
                                .push(widget::text::title3(&card.word.lemma))
                                .push(widget::text::body(format!("/{}/", card.word.phonetic)))
                                .push(widget::text::body(&card.word.definition))
                                .push(widget::text::caption(format!(
                                    "Interval: {} days\nDue: {}",
                                    card.state.interval_days,
                                    card.state.next_review_at.format("%Y-%m-%d %H:%M UTC")
                                )))
                                .push(widget::text::title4("Contexts"));
                            for context in &card.contexts {
                                details = details.push(widget::text::body(format!(
                                    "{}\nSource: {}",
                                    context.sentence, context.source
                                )));
                            }
                            widget::scrollable(details).height(Length::Fill).into()
                        },
                    );
                widget::row::with_capacity(2)
                    .spacing(16)
                    .push(widget::scrollable(list).width(Length::FillPortion(3)))
                    .push(widget::container(rail).width(Length::FillPortion(2)))
                    .height(Length::Fill)
                    .into()
            }
            Page::Reader => self.reader_view(),
            Page::ParseText => self.capture_contents(),
            Page::Review => self.review_view(),
        }
    }

    fn capture_contents(&self) -> Element<'_, HostMessage> {
        let mut column = widget::column::with_capacity(8)
            .spacing(10)
            .push(
                widget::text_input::text_input("Paste or type English text", &self.manual_text)
                    .on_input(HostMessage::ManualTextChanged),
            )
            .push(widget::button::suggested("Find words").on_press(HostMessage::ParseManual));
        for (index, candidate) in self.candidates.iter().enumerate() {
            let checked = self
                .selected_candidates
                .get(index)
                .copied()
                .unwrap_or(false);
            column = column.push(
                widget::checkbox(checked)
                    .label(format!(
                        "{}  /{}/  {}",
                        candidate.entry.lemma, candidate.entry.phonetic, candidate.sentence
                    ))
                    .on_toggle(move |selected| HostMessage::ToggleCandidate(index, selected)),
            );
        }
        if !self.candidates.is_empty() {
            column = column.push(
                widget::button::suggested("Capture selected")
                    .on_press(HostMessage::CaptureCandidates),
            );
        }
        if let Some(status) = &self.capture_status {
            column = column.push(widget::text::caption(status));
        }
        widget::scrollable(column).height(Length::Fill).into()
    }

    fn capture_view(&self) -> Element<'_, HostMessage> {
        widget::container(
            widget::column::with_capacity(2)
                .spacing(10)
                .push(self.capture_contents())
                .push(widget::button::text("Cancel").on_press(HostMessage::CancelCapture)),
        )
        .padding(16)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    fn review_view(&self) -> Element<'_, HostMessage> {
        let Some(card) = self.model.review.cards.get(self.model.review.active_index) else {
            return widget::text::body("No cards due").into();
        };
        let mut column = widget::column::with_capacity(10)
            .spacing(10)
            .push(widget::text::title3(&card.word.lemma));
        if let Some(context) = card.contexts.first() {
            column = column.push(widget::text::body(&context.sentence));
        }
        if self.model.review.phase == ReviewPhase::Prompt {
            column = column.push(
                widget::row::with_capacity(2)
                    .spacing(6)
                    .push(widget::button::suggested("Reveal").on_press(HostMessage::Reveal))
                    .push(widget::button::text("Skip").on_press(HostMessage::SkipReview)),
            );
        } else if self.model.review.phase == ReviewPhase::Revealed {
            column = column
                .push(widget::text::body(format!(
                    "/{}/\n{}",
                    card.word.phonetic, card.word.definition
                )))
                .push(
                    (0_u8..=5).fold(widget::row::with_capacity(6).spacing(6), |row, rating| {
                        row.push(
                            widget::button::text(rating.to_string())
                                .on_press(HostMessage::Rate(rating)),
                        )
                    }),
                );
        }
        column.into()
    }

    fn close_main_window(&self) -> Task<HostMessage> {
        self.core
            .main_window_id()
            .map_or_else(Task::none, cosmic::iced::window::close)
    }

    fn handle_key(
        &mut self,
        key: keyboard::Key,
        modifiers: keyboard::Modifiers,
        repeat: bool,
        captured: bool,
    ) -> Task<HostMessage> {
        use cosmic::iced::keyboard::key::Named;

        if self.flags.capture_mode && key == keyboard::Key::Named(Named::Escape) {
            return self.close_main_window();
        }
        if captured {
            return if key == keyboard::Key::Named(Named::Tab) {
                self.move_widget_focus(modifiers.shift())
            } else {
                Task::none()
            };
        }
        let Some((shortcut_key, event_kind)) = shortcut_key(&key, modifiers, repeat) else {
            return Task::none();
        };
        if let Some(shortcut) = GlobalShortcut::from_key(shortcut_key.clone(), event_kind) {
            return self.handle_global_shortcut(shortcut);
        }
        if self.model.page == Page::Reader {
            if let Some(shortcut) = ReaderShortcut::from_key(shortcut_key, event_kind) {
                return self.handle_reader_shortcut(shortcut);
            }
        }
        match (&self.model.page, key) {
            (Page::Review, keyboard::Key::Named(Named::Enter)) => {
                self.apply_domain(DomainMessage::RevealReview)
            }
            (Page::Review, keyboard::Key::Character(value)) => {
                if value.as_str() == " " {
                    return self.apply_domain(DomainMessage::RevealReview);
                }
                value
                    .as_str()
                    .parse::<u8>()
                    .ok()
                    .filter(|rating| *rating <= 5)
                    .map_or_else(Task::none, |rating| match ReviewRating::try_from(rating) {
                        Ok(rating) => {
                            self.apply_domain(DomainMessage::Rate(rating, current_time()))
                        }
                        Err(_) => Task::none(),
                    })
            }
            _ => Task::none(),
        }
    }

    fn handle_global_shortcut(&mut self, shortcut: GlobalShortcut) -> Task<HostMessage> {
        let page = match shortcut {
            GlobalShortcut::NextPage => Some(self.model.page.next()),
            GlobalShortcut::PreviousPage => Some(self.model.page.previous()),
            GlobalShortcut::Navigate(page) => Some(page),
            GlobalShortcut::ReleaseFocus => None,
        };
        match page {
            Some(page) => Task::batch([
                self.apply_domain(DomainMessage::Navigate {
                    page,
                    as_of: current_time(),
                }),
                release_widget_focus(),
            ]),
            None => release_widget_focus(),
        }
    }

    fn move_widget_focus(&self, backwards: bool) -> Task<HostMessage> {
        let task = if backwards {
            cosmic::iced::widget::operation::focus_previous()
        } else {
            cosmic::iced::widget::operation::focus_next()
        };
        task.map(cosmic::Action::App)
    }
}

fn effect_error(effect: Effect, error: String) -> DomainMessage {
    match effect {
        Effect::LoadDashboard { .. } => DomainMessage::DashboardLoaded(Err(error)),
        Effect::LoadLibrary { generation } => DomainMessage::LibraryLoaded {
            generation,
            result: Err(error),
        },
        Effect::LoadReviewBatch { .. } => DomainMessage::ReviewBatchLoaded(Err(error)),
        Effect::CommitReview { reviewed_at, .. } => {
            DomainMessage::ReviewCommitted(Err(error), reviewed_at)
        }
        Effect::Reader(reader_effect) => DomainMessage::Reader(match reader_effect {
            ReaderEffect::PickDocument { generation } => ReaderMessage::DocumentOpened {
                generation,
                result: Err(error),
            },
            ReaderEffect::HydrateWord {
                generation,
                cache_key,
                ..
            } => ReaderMessage::WordHydrated {
                generation,
                cache_key,
                result: Err(error),
            },
            ReaderEffect::CaptureWord {
                generation,
                cache_key,
                entry,
                ..
            } => ReaderMessage::CaptureFinished {
                generation,
                cache_key,
                lemma: entry.lemma,
                result: Err(error),
            },
        }),
    }
}

fn shortcut_key(
    key: &keyboard::Key,
    modifiers: keyboard::Modifiers,
    repeat: bool,
) -> Option<(ReaderKey, KeyEventKind)> {
    use cosmic::iced::keyboard::key::Named;

    let event_kind = if repeat {
        KeyEventKind::Repeated
    } else {
        KeyEventKind::Pressed
    };
    let modified = modifiers.control() || modifiers.alt() || modifiers.logo();
    let reader_key = match key {
        keyboard::Key::Character(value) if modified => {
            ReaderKey::ModifiedCharacter(value.to_string())
        }
        keyboard::Key::Character(value) => ReaderKey::Character(value.to_string()),
        keyboard::Key::Named(Named::ArrowLeft) if !modified => ReaderKey::NamedLeft,
        keyboard::Key::Named(Named::ArrowRight) if !modified => ReaderKey::NamedRight,
        keyboard::Key::Named(Named::ArrowUp) if !modified => ReaderKey::NamedUp,
        keyboard::Key::Named(Named::ArrowDown) if !modified => ReaderKey::NamedDown,
        keyboard::Key::Named(Named::Enter) if !modified => ReaderKey::Enter,
        keyboard::Key::Named(Named::Tab) if !modified && modifiers.shift() => ReaderKey::ShiftTab,
        keyboard::Key::Named(Named::Tab) if !modified => ReaderKey::Tab,
        keyboard::Key::Named(Named::Escape) if !modified => ReaderKey::Escape,
        _ => return None,
    };
    Some((reader_key, event_kind))
}

fn release_widget_focus() -> Task<HostMessage> {
    cosmic::iced::runtime::task::effect(cosmic::iced::runtime::Action::widget(
        cosmic::iced::core::widget::operation::focusable::unfocus(),
    ))
    .map(cosmic::Action::App)
}

fn current_time() -> chrono::DateTime<Utc> {
    std::time::SystemTime::now().into()
}
