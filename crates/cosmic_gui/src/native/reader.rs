use core_engine::StorageAdapter;
use cosmic::app::Task;
use cosmic::iced::widget::scrollable::AbsoluteOffset;
use cosmic::iced::widget::text::Wrapping;
use cosmic::iced::widget::{operation, rich_text, span, Id};
use cosmic::iced::{font, Border, Font, Length, Padding};
use cosmic::{widget, Element};
use ensub_sqlite::{BundledLexicon, SqliteStorage};
use language_engine::{capture_from_entry, word_id_for_lemma, Document, DocumentFormat, Lexicon};

use super::{current_time, HostMessage, NativeApp};
use crate::{
    build_block_runs, reader_badge, reader_uses_split_layout, ReaderEffect, ReaderMessage,
    ReaderShortcut, ReaderWordDetails,
};

const READER_SCROLL_STEP: f32 = 64.0;
const READER_SCROLL_ID: &str = "ensub-reader-document";

impl NativeApp {
    pub(super) fn reader_view(&self) -> Element<'_, HostMessage> {
        widget::responsive(|size| self.reader_layout(reader_uses_split_layout(size.width))).into()
    }

    fn reader_layout(&self, wide: bool) -> Element<'_, HostMessage> {
        let reader = self.reading_pane();
        let rail = self.word_rail();
        if wide {
            widget::row::with_capacity(2)
                .spacing(20)
                .push(widget::container(reader).width(Length::FillPortion(13)))
                .push(widget::container(rail).width(Length::FillPortion(7)))
                .height(Length::Fill)
                .into()
        } else {
            widget::column::with_capacity(2)
                .spacing(16)
                .push(widget::container(reader).height(Length::FillPortion(3)))
                .push(widget::container(rail).height(Length::FillPortion(2)))
                .height(Length::Fill)
                .into()
        }
    }

    fn reading_pane(&self) -> Element<'_, HostMessage> {
        let reader = &self.model.reader;
        let path = reader
            .document
            .as_ref()
            .map(|document| document.path().display().to_string())
            .unwrap_or_else(|| "No document open".to_string());
        let progress = reader.document.as_ref().map_or_else(
            || "0 words".to_string(),
            |document| {
                reader.cursor.map_or_else(
                    || format!("{} words", document.tokens().len()),
                    |cursor| {
                        format!(
                            "Word {} of {}",
                            cursor.saturating_add(1),
                            document.tokens().len()
                        )
                    },
                )
            },
        );
        let header = widget::row::with_capacity(3)
            .spacing(10)
            .align_y(cosmic::iced::Alignment::Center)
            .push(
                widget::button::suggested("Open document")
                    .on_press(HostMessage::Reader(ReaderMessage::OpenRequested)),
            )
            .push(widget::text::caption(path).width(Length::Fill))
            .push(widget::text::caption(progress));

        let mut paragraphs = widget::column::with_capacity(
            reader
                .document
                .as_ref()
                .map_or(1, |document| document.blocks().len()),
        )
        .spacing(16);
        if let Some(document) = reader.document.as_ref() {
            for block_index in 0..document.blocks().len() {
                paragraphs = paragraphs.push(self.reader_paragraph(document, block_index));
            }
            if document.blocks().is_empty() {
                paragraphs = paragraphs.push(widget::text::body("This document is empty."));
            }
        } else {
            paragraphs = paragraphs.push(widget::text::body(
                "Open a Markdown or plain-text document.",
            ));
        }

        let scrollable = widget::scrollable(
            widget::container(paragraphs)
                .padding(Padding::from([18, 12]))
                .width(Length::Fill),
        )
        .id(Id::new(READER_SCROLL_ID))
        .height(Length::Fill);
        widget::column::with_capacity(2)
            .spacing(10)
            .push(header)
            .push(scrollable)
            .height(Length::Fill)
            .into()
    }

    fn reader_paragraph<'a>(
        &self,
        document: &'a Document,
        block_index: usize,
    ) -> Element<'a, HostMessage> {
        let palette = cosmic::theme::active();
        let accent = palette.cosmic().accent.base;
        let on_accent = palette.cosmic().on_accent_color();
        let selected = self.model.reader.cursor;
        let spans = build_block_runs(document, block_index)
            .into_iter()
            .map(|run| {
                let mut font = Font::default();
                if run.style.bold {
                    font.weight = font::Weight::Bold;
                }
                if run.style.italic {
                    font.style = font::Style::Italic;
                }
                let is_selected = run.token_index.is_some() && run.token_index == selected;
                let mut value = span(run.text)
                    .font(font)
                    .underline(run.style.underlined)
                    .link_maybe(run.token_index);
                if is_selected {
                    value = value
                        .background(accent)
                        .color(on_accent)
                        .border(Border {
                            radius: 4.0.into(),
                            ..Border::default()
                        })
                        .padding([2, 3]);
                }
                value
            })
            .collect::<Vec<_>>();

        rich_text(spans)
            .size(18)
            .line_height(1.55)
            .width(Length::Fill)
            .wrapping(Wrapping::WordOrGlyph)
            .on_link_click(|index| HostMessage::Reader(ReaderMessage::SelectToken(index)))
            .into()
    }

    fn word_rail(&self) -> Element<'_, HostMessage> {
        let reader = &self.model.reader;
        let Some(document) = reader.document.as_ref() else {
            return widget::container(widget::text::body("Select a word to inspect."))
                .padding(16)
                .width(Length::Fill)
                .height(Length::Fill)
                .into();
        };
        let Some(token) = reader.cursor.and_then(|index| document.tokens().get(index)) else {
            return widget::container(widget::text::body("No word candidates in this document."))
                .padding(16)
                .width(Length::Fill)
                .height(Length::Fill)
                .into();
        };

        let mut rail = widget::column::with_capacity(12)
            .spacing(10)
            .push(widget::text::caption("WORD INSPECTOR"));
        match reader.details.as_ref() {
            Some(details) => {
                let badge = reader_badge(details.state.as_ref(), current_time()).label();
                if let Some(entry) = details.entry.as_ref() {
                    rail = rail
                        .push(widget::text::title2(&entry.lemma))
                        .push(widget::text::caption(format!("[{badge}]")))
                        .push(widget::text::title4(format!("/{}/", entry.phonetic)));
                    for definition in &entry.definitions {
                        rail = rail
                            .push(widget::text::caption(&definition.part_of_speech))
                            .push(widget::text::body(&definition.text));
                    }
                } else {
                    rail = rail
                        .push(widget::text::title2(&details.surface))
                        .push(widget::text::caption("[New]"))
                        .push(widget::text::body("Not in bundled lexicon"));
                }
            }
            None => {
                rail = rail
                    .push(widget::text::title2(&token.surface))
                    .push(widget::text::body("Looking up word..."));
            }
        }
        rail = rail
            .push(widget::divider::horizontal::default())
            .push(widget::text::caption("CONTEXT"))
            .push(widget::text::body(&token.sentence));

        let can_capture = reader
            .details
            .as_ref()
            .and_then(|details| details.entry.as_ref())
            .is_some()
            && !reader.capturing;
        let capture_button = if can_capture {
            widget::button::suggested("Capture Context (C)").on_press(HostMessage::Reader(
                ReaderMessage::CaptureRequested {
                    captured_at: current_time(),
                },
            ))
        } else {
            widget::button::suggested(if reader.capturing {
                "Capturing..."
            } else {
                "Capture Context (C)"
            })
        };
        rail = rail.push(capture_button);
        if let Some(feedback) = reader.feedback.as_deref() {
            rail = rail.push(widget::text::caption(feedback));
        }
        if let Some(error) = reader.error.as_deref() {
            rail = rail.push(
                widget::warning(error).on_close(HostMessage::Reader(ReaderMessage::ClearError)),
            );
        }

        widget::container(widget::scrollable(rail).height(Length::Fill))
            .padding(16)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    pub(super) fn run_reader_effect(&self, effect: ReaderEffect) -> Task<HostMessage> {
        match effect {
            ReaderEffect::PickDocument { generation } => pick_document_task(generation),
            ReaderEffect::HydrateWord {
                generation,
                cache_key,
                surface,
            } => {
                let cache_dir = self.flags.lexicon_cache_dir.clone();
                let database_path = self.flags.database_path.clone();
                cosmic::task::future(async move {
                    let result = tokio::task::spawn_blocking(move || {
                        let lexicon =
                            BundledLexicon::open(cache_dir).map_err(|error| error.to_string())?;
                        let entry = lexicon
                            .lookup(&surface)
                            .map_err(|error| error.to_string())?;
                        let state = if let Some(entry) = entry.as_ref() {
                            let storage = SqliteStorage::open(database_path)
                                .map_err(|error| error.to_string())?;
                            storage
                                .review_state(&word_id_for_lemma(&entry.lemma))
                                .map_err(|error| error.to_string())?
                        } else {
                            None
                        };
                        Ok::<_, String>(ReaderWordDetails {
                            surface,
                            entry,
                            state,
                        })
                    })
                    .await
                    .map_err(|error| error.to_string())
                    .and_then(|result| result);
                    HostMessage::Reader(ReaderMessage::WordHydrated {
                        generation,
                        cache_key,
                        result,
                    })
                })
            }
            ReaderEffect::CaptureWord {
                generation,
                cache_key,
                surface,
                sentence,
                source_path,
                entry,
                captured_at,
            } => {
                let database_path = self.flags.database_path.clone();
                let lemma = entry.lemma.clone();
                cosmic::task::future(async move {
                    let result = tokio::task::spawn_blocking(move || {
                        let canonical_path = match std::fs::canonicalize(&source_path) {
                            Ok(path) => path,
                            Err(_) => source_path,
                        };
                        let source = format!("gui:{}", canonical_path.display());
                        let capture = capture_from_entry(
                            &surface,
                            Some(&sentence),
                            &source,
                            entry,
                            captured_at,
                        );
                        let mut storage = SqliteStorage::open(database_path)
                            .map_err(|error| error.to_string())?;
                        let saved = storage
                            .save_capture(&capture)
                            .map_err(|error| error.to_string())?;
                        let state = storage
                            .review_state(&capture.word.id)
                            .map_err(|error| error.to_string())?
                            .ok_or_else(|| "captured word has no review state".to_string())?;
                        Ok::<_, String>((saved, state))
                    })
                    .await
                    .map_err(|error| error.to_string())
                    .and_then(|result| result);
                    HostMessage::Reader(ReaderMessage::CaptureFinished {
                        generation,
                        cache_key,
                        lemma,
                        result,
                    })
                })
            }
        }
    }

    pub(super) fn scroll_reader(&self, delta: f32) -> Task<HostMessage> {
        operation::scroll_by(
            Id::new(READER_SCROLL_ID),
            AbsoluteOffset { x: 0.0, y: delta },
        )
        .map(cosmic::Action::App)
    }

    pub(super) fn handle_reader_shortcut(&mut self, shortcut: ReaderShortcut) -> Task<HostMessage> {
        match shortcut {
            ReaderShortcut::MovePrevious => {
                self.apply_domain(crate::Message::Reader(ReaderMessage::MovePrevious))
            }
            ReaderShortcut::MoveNext => {
                self.apply_domain(crate::Message::Reader(ReaderMessage::MoveNext))
            }
            ReaderShortcut::ScrollUp => self.scroll_reader(-READER_SCROLL_STEP),
            ReaderShortcut::ScrollDown => self.scroll_reader(READER_SCROLL_STEP),
            ReaderShortcut::Capture => {
                self.apply_domain(crate::Message::Reader(ReaderMessage::CaptureRequested {
                    captured_at: current_time(),
                }))
            }
            ReaderShortcut::Open => {
                self.apply_domain(crate::Message::Reader(ReaderMessage::OpenRequested))
            }
        }
    }
}

fn pick_document_task(generation: u64) -> Task<HostMessage> {
    cosmic::task::future(async move {
        let filter = cosmic::dialog::file_chooser::FileFilter::new("Text documents")
            .glob("*.md")
            .glob("*.markdown")
            .glob("*.txt");
        let result = match cosmic::dialog::file_chooser::open::Dialog::new()
            .title("Open reading document")
            .filter(filter)
            .open_file()
            .await
        {
            Ok(response) => response
                .url()
                .to_file_path()
                .map_err(|_| "selected resource is not a local file".to_string())
                .and_then(|path| {
                    std::fs::read_to_string(&path)
                        .map(|contents| {
                            let format = DocumentFormat::from_path(&path);
                            Some(Document::parse(path, format, &contents))
                        })
                        .map_err(|error| error.to_string())
                }),
            Err(cosmic::dialog::file_chooser::Error::Cancelled) => Ok(None),
            Err(error) => Err(error.to_string()),
        };
        HostMessage::Reader(ReaderMessage::DocumentOpened { generation, result })
    })
}
