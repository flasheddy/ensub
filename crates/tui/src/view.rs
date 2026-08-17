use ensub_theme::{Rgb, Theme};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block as UiBlock, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::{BlockKind, Mode, Model};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorPolicy {
    Enabled,
    Disabled,
}

impl ColorPolicy {
    #[must_use]
    pub fn from_env() -> Self {
        if std::env::var_os("NO_COLOR").is_some() {
            Self::Disabled
        } else {
            Self::Enabled
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Palette {
    background: Color,
    surface: Color,
    surface_raised: Color,
    border: Color,
    border_strong: Color,
    text: Color,
    muted: Color,
    accent: Color,
    selection: Color,
    on_selection: Color,
    warning: Color,
    info: Color,
    danger: Color,
}

impl Palette {
    fn new(theme: &Theme, policy: ColorPolicy) -> Self {
        let color = |value| match policy {
            ColorPolicy::Enabled => terminal_color(value),
            ColorPolicy::Disabled => Color::Reset,
        };
        Self {
            background: color(theme.background),
            surface: color(theme.surface),
            surface_raised: color(theme.surface_raised),
            border: color(theme.border),
            border_strong: color(theme.border_strong),
            text: color(theme.text),
            muted: color(theme.text_muted),
            accent: color(theme.accent),
            selection: color(theme.selection),
            on_selection: color(theme.on_selection),
            warning: color(theme.warning),
            info: color(theme.info),
            danger: color(theme.danger),
        }
    }

    fn base(self) -> Style {
        Style::default().fg(self.text).bg(self.background)
    }

    fn raised(self) -> Style {
        Style::default().fg(self.text).bg(self.surface_raised)
    }

    fn recessed(self) -> Style {
        Style::default().fg(self.text).bg(self.surface)
    }
}

fn terminal_color(color: Rgb) -> Color {
    Color::Rgb(color.red, color.green, color.blue)
}

pub fn render(frame: &mut Frame<'_>, model: &Model) {
    render_with_theme(frame, model, &Theme::default(), ColorPolicy::from_env());
}

pub fn render_with_theme(frame: &mut Frame<'_>, model: &Model, theme: &Theme, policy: ColorPolicy) {
    let palette = Palette::new(theme, policy);
    let area = frame.area();
    frame.render_widget(UiBlock::default().style(palette.base()), area);
    if area.width < 50 || area.height < 12 {
        render_too_small(frame, area, palette);
        return;
    }

    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);
    let content = vertical[0];
    let status = vertical[1];

    if area.width >= 100 && model.panel_visible() {
        let panes = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(40), Constraint::Length(35)])
            .split(content);
        render_reader(frame, model, panes[0], palette);
        render_vocabulary(frame, model, panes[1], palette);
    } else {
        render_reader(frame, model, content, palette);
    }
    render_status(frame, model, status, palette);

    match model.mode() {
        Mode::OpenPath => render_path_overlay(frame, model, content, palette),
        Mode::Vocabulary => render_vocabulary_overlay(frame, model, content, palette),
        Mode::Review => render_review_overlay(frame, model, content, palette),
        Mode::Reader => {}
    }
}

fn render_too_small(frame: &mut Frame<'_>, area: Rect, palette: Palette) {
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new("Terminal too small\nMinimum size: 50 x 12")
            .alignment(Alignment::Center)
            .style(palette.base().fg(palette.muted)),
        area,
    );
}

fn render_reader(frame: &mut Frame<'_>, model: &Model, area: Rect, palette: Palette) {
    let inner = area.inner(Margin {
        horizontal: 1,
        vertical: 1,
    });
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let visible = model
        .layout()
        .lines()
        .iter()
        .enumerate()
        .skip(model.viewport_top())
        .take(usize::from(inner.height))
        .map(|(line_index, _)| reader_line(model, line_index, palette))
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(visible).style(palette.base()), inner);
}

fn reader_line(model: &Model, line_index: usize, palette: Palette) -> Line<'static> {
    let Some(document) = model.document() else {
        return Line::default();
    };
    let Some(visual) = model.layout().lines().get(line_index) else {
        return Line::default();
    };
    let Some(block) = document.blocks().get(visual.block_index) else {
        return Line::default();
    };
    if block.kind == BlockKind::Rule {
        return Line::from(Span::styled(
            visual.text.clone(),
            palette.base().fg(palette.muted),
        ));
    }

    let mut spans = Vec::new();
    let prefix = match block.kind {
        BlockKind::ListItem => "- ",
        BlockKind::Quote => "> ",
        BlockKind::Code => "  ",
        _ => "",
    };
    if !prefix.is_empty() {
        spans.push(Span::styled(
            prefix.to_string(),
            palette.base().fg(palette.muted),
        ));
    }
    let content = block.text.get(visual.start..visual.end).unwrap_or_default();
    for (relative, grapheme) in content.grapheme_indices(true) {
        let offset = visual.start.saturating_add(relative);
        let style = reader_style(model, visual.block_index, offset, block.kind, palette);
        spans.push(Span::styled(grapheme.to_string(), style));
    }
    Line::from(spans)
}

fn reader_style(
    model: &Model,
    block_index: usize,
    offset: usize,
    kind: BlockKind,
    palette: Palette,
) -> Style {
    let mut style = match kind {
        BlockKind::Heading => palette
            .base()
            .fg(palette.accent)
            .add_modifier(Modifier::BOLD),
        BlockKind::Quote => palette
            .base()
            .fg(palette.muted)
            .add_modifier(Modifier::ITALIC),
        BlockKind::Code => palette.base().fg(palette.muted),
        _ => palette.base(),
    };
    let Some(document) = model.document() else {
        return style;
    };
    if let Some(block) = document.blocks().get(block_index) {
        if let Some(range) = block
            .ranges
            .iter()
            .find(|range| offset >= range.start && offset < range.end)
        {
            if range.style.bold {
                style = style.add_modifier(Modifier::BOLD);
            }
            if range.style.italic {
                style = style.add_modifier(Modifier::ITALIC);
            }
            if range.style.underlined {
                style = style.add_modifier(Modifier::UNDERLINED);
            }
            if range.style.dim {
                style = style.add_modifier(Modifier::DIM);
            }
        }
    }
    if let Some((token_index, token)) = document.tokens().iter().enumerate().find(|(_, token)| {
        token.block_index == block_index && offset >= token.start && offset < token.end
    }) {
        if let Some(state) = model
            .word_details(&token.surface)
            .and_then(|details| details.state.as_ref())
        {
            let due = model
                .as_of()
                .is_some_and(|as_of| state.next_review_at <= as_of);
            style = style
                .fg(if due { palette.warning } else { palette.info })
                .add_modifier(Modifier::UNDERLINED);
        }
        if model.active_token_index() == Some(token_index) {
            style = style
                .fg(palette.on_selection)
                .bg(palette.selection)
                .add_modifier(Modifier::BOLD);
        }
    }
    style
}

fn render_vocabulary(frame: &mut Frame<'_>, model: &Model, area: Rect, palette: Palette) {
    let block = UiBlock::default()
        .borders(Borders::LEFT)
        .title(" Vocabulary ")
        .style(palette.raised())
        .border_style(Style::default().fg(palette.border));
    let inner = block.inner(area).inner(Margin {
        horizontal: 1,
        vertical: 1,
    });
    frame.render_widget(block, area);
    frame.render_widget(
        Paragraph::new(vocabulary_lines(model, palette))
            .style(palette.raised())
            .wrap(Wrap { trim: false }),
        inner,
    );
}

fn vocabulary_lines(model: &Model, palette: Palette) -> Vec<Line<'static>> {
    let Some(token) = model.active_token() else {
        return vec![Line::from(Span::styled(
            "No active word",
            palette.raised().fg(palette.muted),
        ))];
    };
    let mut lines = vec![Line::from(Span::styled(
        token.surface.clone(),
        palette
            .raised()
            .fg(palette.accent)
            .add_modifier(Modifier::BOLD),
    ))];
    let Some(details) = model.active_word_details() else {
        lines.push(Line::from(Span::styled(
            "Looking up...",
            palette.raised().fg(palette.muted),
        )));
        return lines;
    };
    let Some(entry) = details.entry.as_ref() else {
        lines.push(Line::from(Span::styled(
            "Not in bundled lexicon",
            palette.raised().fg(palette.muted),
        )));
        return lines;
    };
    lines
        .push(Line::from(entry.lemma.clone()).style(palette.raised().add_modifier(Modifier::BOLD)));
    lines.push(
        Line::from(format!("/{}/", entry.phonetic)).style(palette.raised().fg(palette.muted)),
    );
    lines.push(Line::default());
    for definition in &entry.definitions {
        lines.push(Line::from(format!(
            "{}: {}",
            definition.part_of_speech, definition.text
        )));
    }
    lines.push(Line::default());
    match details.state.as_ref() {
        Some(state) => {
            lines.push(Line::from("SRS").style(palette.raised().fg(palette.accent)));
            lines.push(Line::from(format!(
                "Interval: {} days",
                state.interval_days
            )));
            let due = model
                .as_of()
                .is_some_and(|as_of| state.next_review_at <= as_of);
            lines.push(
                Line::from(if due { "Due now" } else { "Scheduled" }).style(
                    palette
                        .raised()
                        .fg(if due { palette.warning } else { palette.info }),
                ),
            );
        }
        None => lines.push(Line::from("Not captured").style(palette.raised().fg(palette.muted))),
    }
    lines.push(Line::default());
    lines.push(Line::from("Context").style(palette.raised().fg(palette.accent)));
    lines.push(Line::from(token.sentence.clone()));
    if let Some(document) = model.document() {
        lines.push(
            Line::from(document.path().display().to_string())
                .style(palette.raised().fg(palette.muted)),
        );
    }
    lines
}

fn render_vocabulary_overlay(frame: &mut Frame<'_>, model: &Model, area: Rect, palette: Palette) {
    let overlay = area.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });
    frame.render_widget(Clear, overlay);
    let block = UiBlock::default()
        .borders(Borders::ALL)
        .title(" Vocabulary ")
        .style(palette.raised())
        .border_style(Style::default().fg(palette.border_strong));
    let inner = block.inner(overlay).inner(Margin {
        horizontal: 1,
        vertical: 1,
    });
    frame.render_widget(block, overlay);
    frame.render_widget(
        Paragraph::new(vocabulary_lines(model, palette))
            .style(palette.raised())
            .wrap(Wrap { trim: false }),
        inner,
    );
}

fn render_status(frame: &mut Frame<'_>, model: &Model, area: Rect, palette: Palette) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(10),
            Constraint::Min(8),
            Constraint::Length(7),
            Constraint::Length(10),
        ])
        .split(area);
    let mode = match model.mode() {
        Mode::Reader => "NORMAL",
        Mode::OpenPath => "OPEN",
        Mode::Vocabulary => "WORD",
        Mode::Review => "REVIEW",
    };
    frame.render_widget(
        Paragraph::new(mode).style(
            palette
                .recessed()
                .fg(palette.accent)
                .add_modifier(Modifier::BOLD),
        ),
        chunks[0],
    );
    let center = model.notice().map_or_else(
        || {
            model
                .document()
                .map(|document| document.path().display().to_string())
                .unwrap_or_default()
        },
        |notice| notice.message.clone(),
    );
    let center_style = model.notice().filter(|notice| notice.is_error).map_or_else(
        || palette.recessed(),
        |_| palette.recessed().fg(palette.danger),
    );
    frame.render_widget(Paragraph::new(center).style(center_style), chunks[1]);
    frame.render_widget(
        Paragraph::new(format!("{}%", model.progress_percent()))
            .style(palette.recessed())
            .alignment(Alignment::Right),
        chunks[2],
    );
    let due = model
        .due_count()
        .map_or_else(|| "Due: -".to_string(), |count| format!("Due: {count}"));
    frame.render_widget(
        Paragraph::new(due)
            .alignment(Alignment::Right)
            .style(palette.recessed().fg(palette.warning)),
        chunks[3],
    );
}

fn render_path_overlay(frame: &mut Frame<'_>, model: &Model, area: Rect, palette: Palette) {
    let width = area.width.saturating_sub(4).clamp(20, 72);
    let overlay = centered(area, width, 5.min(area.height));
    frame.render_widget(Clear, overlay);
    let block = UiBlock::default()
        .borders(Borders::ALL)
        .title(" Open file ")
        .style(palette.raised())
        .border_style(Style::default().fg(palette.border_strong));
    let inner = block.inner(overlay).inner(Margin {
        horizontal: 1,
        vertical: 1,
    });
    frame.render_widget(block, overlay);
    frame.render_widget(
        Paragraph::new(model.path_input().to_string()).style(palette.raised()),
        inner,
    );
    let cursor_width = u16::try_from(UnicodeWidthStr::width(
        &model.path_input()[..model.path_cursor()],
    ))
    .unwrap_or(u16::MAX);
    frame.set_cursor_position((
        inner
            .x
            .saturating_add(cursor_width.min(inner.width.saturating_sub(1))),
        inner.y,
    ));
}

fn render_review_overlay(frame: &mut Frame<'_>, model: &Model, area: Rect, palette: Palette) {
    let width = area.width.saturating_sub(2).clamp(20, 76);
    let height = area.height.saturating_sub(2).clamp(8, 18);
    let overlay = centered(area, width, height);
    frame.render_widget(Clear, overlay);
    let progress = model.review_progress().map_or_else(
        || "Quick review".to_string(),
        |(index, total)| format!("Quick review {index} / {total}"),
    );
    let block = UiBlock::default()
        .borders(Borders::ALL)
        .title(format!(" {progress} "))
        .style(palette.raised())
        .border_style(Style::default().fg(palette.warning));
    let inner = block.inner(overlay).inner(Margin {
        horizontal: 2,
        vertical: 1,
    });
    frame.render_widget(block, overlay);
    let Some(card) = model.review_card() else {
        frame.render_widget(
            Paragraph::new("Loading due reviews...")
                .alignment(Alignment::Center)
                .style(palette.raised().fg(palette.muted)),
            inner,
        );
        return;
    };
    let mut lines = vec![
        Line::from(card.word.lemma.clone()).style(
            palette
                .raised()
                .fg(palette.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Line::default(),
    ];
    if let Some(context) = card.contexts.first() {
        lines.push(Line::from(context.sentence.clone()));
        lines.push(
            Line::from(format!("source: {}", context.source))
                .style(palette.raised().fg(palette.muted)),
        );
    }
    if model.review_revealed() {
        lines.push(Line::default());
        lines.push(
            Line::from(format!("/{}/", card.word.phonetic))
                .style(palette.raised().fg(palette.warning)),
        );
        for definition in card.word.definition.lines() {
            lines.push(Line::from(definition.to_string()));
        }
        lines.push(Line::default());
        lines.push(
            Line::from("0 Blackout  1 Incorrect  2 Familiar  3 Difficult  4 Good  5 Easy")
                .style(palette.raised().fg(palette.muted)),
        );
    }
    frame.render_widget(
        Paragraph::new(lines)
            .style(palette.raised())
            .wrap(Wrap { trim: false }),
        inner,
    );
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    Rect {
        x: area.x.saturating_add(area.width.saturating_sub(width) / 2),
        y: area
            .y
            .saturating_add(area.height.saturating_sub(height) / 2),
        width: width.min(area.width),
        height: height.min(area.height),
    }
}
