use std::path::PathBuf;
use std::time::Duration;

use core_engine::{LibraryStorageAdapter, ReviewRating, ReviewUpdate, StorageAdapter};
use cosmic::app::{Core, Task};
use cosmic::iced::core::window;
use cosmic::iced::window::Id;
use cosmic::iced::{Length, Rectangle, Subscription, Vector};
use cosmic::surface::action::{app_popup, destroy_popup};
use cosmic::{widget, Element};
use ensub_sqlite::SqliteStorage;
use ensub_theme::Theme;

use crate::{
    badge_text, to_cosmic_theme, update, Effect, Message as DomainMessage, Model, ReviewPhase,
};

#[derive(Debug, Clone)]
pub struct AppletFlags {
    pub database_path: PathBuf,
}

pub fn run(flags: AppletFlags) -> cosmic::iced::Result {
    run_with_theme(flags, Theme::default())
}

pub fn run_with_theme(flags: AppletFlags, theme: Theme) -> cosmic::iced::Result {
    cosmic::applet::run::<NativeApplet>(NativeAppletFlags { flags, theme })
}

#[derive(Debug, Clone)]
pub struct NativeAppletFlags {
    flags: AppletFlags,
    theme: Theme,
}

pub struct NativeApplet {
    core: Core,
    flags: AppletFlags,
    model: Model,
    popup: Option<Id>,
    badge_label: String,
}

#[derive(Debug, Clone)]
pub enum HostMessage {
    TogglePopup(Vector, Rectangle),
    PopupClosed(Id),
    Surface(cosmic::surface::Action),
    Domain(DomainMessage),
    Tick,
    Reveal,
    Rate(u8),
    LaunchCapture,
    LaunchFinished(Result<(), String>),
}

impl cosmic::Application for NativeApplet {
    type Executor = cosmic::SingleThreadExecutor;
    type Flags = NativeAppletFlags;
    type Message = HostMessage;

    const APP_ID: &'static str = "dev.ensub.Ensub.Applet";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, flags: Self::Flags) -> (Self, Task<Self::Message>) {
        let theme = flags.theme;
        let mut applet = Self {
            core,
            flags: flags.flags,
            model: Model::new(current_time()),
            popup: None,
            badge_label: "ESB 0".to_string(),
        };
        let refresh = applet.apply_domain(DomainMessage::Tick(current_time()));
        let apply_theme = cosmic::task::message(cosmic::Action::Cosmic(
            cosmic::app::Action::AppThemeChange(to_cosmic_theme(theme)),
        ));
        (applet, Task::batch([refresh, apply_theme]))
    }

    fn on_close_requested(&self, id: window::Id) -> Option<Self::Message> {
        Some(HostMessage::PopupClosed(id))
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        cosmic::iced::time::every(Duration::from_secs(30)).map(|_| HostMessage::Tick)
    }

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            HostMessage::TogglePopup(offset, bounds) => {
                if let Some(id) = self.popup.take() {
                    self.model.popup_open = false;
                    return cosmic::task::message(cosmic::Action::Cosmic(
                        cosmic::app::Action::Surface(destroy_popup(id)),
                    ));
                }
                let id = Id::unique();
                self.popup = Some(id);
                let surface = app_popup::<NativeApplet>(
                    |_| Default::default(),
                    move |state: &mut NativeApplet| {
                        state.popup = Some(id);
                        let parent = state.core.main_window_id().unwrap_or(Id::NONE);
                        let mut settings = state
                            .core
                            .applet
                            .get_popup_settings(parent, id, None, None, None);
                        settings.positioner.anchor_rect = Rectangle {
                            x: (bounds.x - offset.x) as i32,
                            y: (bounds.y - offset.y) as i32,
                            width: bounds.width as i32,
                            height: bounds.height as i32,
                        };
                        settings
                    },
                    Some(Box::new(|state: &NativeApplet| {
                        state.popup_view().map(cosmic::Action::App)
                    })),
                );
                let refresh = self.apply_domain(DomainMessage::PopupOpened(current_time()));
                Task::batch([
                    cosmic::task::message(cosmic::Action::Cosmic(cosmic::app::Action::Surface(
                        surface,
                    ))),
                    refresh,
                ])
            }
            HostMessage::PopupClosed(id) => {
                if self.popup == Some(id) {
                    self.popup = None;
                    let _ = update(&mut self.model, DomainMessage::PopupClosed);
                }
                Task::none()
            }
            HostMessage::Surface(action) => {
                cosmic::task::message(cosmic::Action::Cosmic(cosmic::app::Action::Surface(action)))
            }
            HostMessage::Domain(message) => self.apply_domain(message),
            HostMessage::Tick => self.apply_domain(DomainMessage::Tick(current_time())),
            HostMessage::Reveal => self.apply_domain(DomainMessage::Reveal),
            HostMessage::Rate(value) => match ReviewRating::try_from(value) {
                Ok(rating) => self.apply_domain(DomainMessage::Rate(rating, current_time())),
                Err(error) => {
                    self.model.error = Some(error.to_string());
                    Task::none()
                }
            },
            HostMessage::LaunchCapture => launch_capture_task(),
            HostMessage::LaunchFinished(result) => {
                if let Err(error) = result {
                    self.model.error = Some(error);
                }
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let popup = self.popup;
        let button = self
            .core
            .applet
            .text_button(self.badge_label.as_str(), HostMessage::Tick)
            .on_press_with_rectangle(move |offset, bounds| {
                let _ = popup;
                HostMessage::TogglePopup(offset, bounds)
            });
        Element::from(self.core.applet.applet_tooltip(
            button,
            format!("{} Ensub reviews due", self.model.due_count),
            self.popup.is_some(),
            HostMessage::Surface,
            None,
        ))
    }

    fn view_window(&self, _id: Id) -> Element<'_, Self::Message> {
        self.popup_view()
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}

impl NativeApplet {
    fn apply_domain(&mut self, message: DomainMessage) -> Task<HostMessage> {
        let effects = update(&mut self.model, message);
        self.badge_label = format!("ESB {}", badge_text(self.model.due_count));
        Task::batch(effects.into_iter().map(|effect| self.run_effect(effect)))
    }

    fn run_effect(&self, effect: Effect) -> Task<HostMessage> {
        if effect == Effect::LaunchCaptureHud {
            return launch_capture_task();
        }
        let database_path = self.flags.database_path.clone();
        cosmic::task::future(async move {
            let failure_effect = effect.clone();
            let outcome = tokio::task::spawn_blocking(move || -> DomainMessage {
                let mut storage = match SqliteStorage::open(database_path) {
                    Ok(storage) => storage,
                    Err(error) => return effect_error(effect, error.to_string()),
                };
                match effect {
                    Effect::RefreshDueCount { as_of } => DomainMessage::DueCountLoaded(
                        storage.due_count(as_of).map_err(|error| error.to_string()),
                    ),
                    Effect::LoadDueCard { as_of } => DomainMessage::DueCardLoaded(
                        storage
                            .due_review_batch(as_of, 1)
                            .map(|mut cards| cards.pop())
                            .map_err(|error| error.to_string()),
                    ),
                    Effect::CommitReview {
                        expected,
                        replacement,
                        reviewed_at,
                    } => DomainMessage::ReviewCommitted(
                        storage
                            .commit_review(&expected, &replacement, reviewed_at)
                            .map(|update| update == ReviewUpdate::Updated)
                            .map_err(|error| error.to_string()),
                        reviewed_at,
                    ),
                    Effect::LaunchCaptureHud => DomainMessage::DueCountLoaded(Ok(0)),
                }
            })
            .await;
            HostMessage::Domain(match outcome {
                Ok(message) => message,
                Err(error) => effect_error(failure_effect, error.to_string()),
            })
        })
    }

    fn popup_view(&self) -> Element<'_, HostMessage> {
        let mut column = widget::column::with_capacity(12)
            .spacing(10)
            .padding(12)
            .width(Length::Fixed(360.0))
            .push(widget::text::title4(format!(
                "{} reviews due",
                self.model.due_count
            )))
            .push(
                widget::button::suggested("Capture from clipboard")
                    .on_press(HostMessage::LaunchCapture),
            );
        if let Some(error) = self.model.error.as_deref() {
            column = column.push(widget::text::caption(error));
        }
        if let Some(card) = self.model.card.as_ref() {
            column = column.push(widget::text::title3(&card.word.lemma));
            if let Some(context) = card.contexts.first() {
                column = column.push(widget::text::body(&context.sentence));
            }
            match self.model.review_phase {
                ReviewPhase::Prompt => {
                    column = column
                        .push(widget::button::suggested("Reveal").on_press(HostMessage::Reveal));
                }
                ReviewPhase::Revealed => {
                    column = column
                        .push(widget::text::body(format!(
                            "/{}/\n{}",
                            card.word.phonetic, card.word.definition
                        )))
                        .push((0_u8..=5).fold(
                            widget::row::with_capacity(6).spacing(4),
                            |row, rating| {
                                row.push(
                                    widget::button::text(rating.to_string())
                                        .on_press(HostMessage::Rate(rating)),
                                )
                            },
                        ));
                }
                ReviewPhase::Saving => {
                    column = column.push(widget::text::caption("Saving review..."));
                }
                ReviewPhase::Empty => {}
            }
        } else {
            column = column.push(widget::text::body("No cards due"));
        }
        self.core.applet.popup_container(column).into()
    }
}

fn effect_error(effect: Effect, error: String) -> DomainMessage {
    match effect {
        Effect::RefreshDueCount { .. } => DomainMessage::DueCountLoaded(Err(error)),
        Effect::LoadDueCard { .. } => DomainMessage::DueCardLoaded(Err(error)),
        Effect::CommitReview { reviewed_at, .. } => {
            DomainMessage::ReviewCommitted(Err(error), reviewed_at)
        }
        Effect::LaunchCaptureHud => DomainMessage::DueCountLoaded(Err(error)),
    }
}

fn launch_capture_task() -> Task<HostMessage> {
    cosmic::task::future(async move {
        let result = tokio::task::spawn_blocking(|| {
            let sibling = std::env::current_exe()
                .ok()
                .and_then(|path| path.parent().map(|parent| parent.join("ensub-gui")))
                .filter(|path| path.is_file());
            let program = sibling.unwrap_or_else(|| PathBuf::from("ensub-gui"));
            std::process::Command::new(&program)
                .arg("--capture")
                .spawn()
                .map(|_| ())
                .map_err(|error| format!("failed to launch {}: {error}", program.display()))
        })
        .await
        .map_err(|error| error.to_string())
        .and_then(|result| result);
        HostMessage::LaunchFinished(result)
    })
}

fn current_time() -> chrono::DateTime<chrono::Utc> {
    std::time::SystemTime::now().into()
}
