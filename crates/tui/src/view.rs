use ratatui::layout::{Alignment, Constraint, Direction, Layout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block as UiBlock, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::{BlockKind, Mode, Model};

const FOCUS: Color = Color::Rgb(94, 196, 182);
const DUE: Color = Color::Rgb(230, 180, 80);
const SCHEDULED: Color = Color::Rgb(108, 182, 235);
const ERROR: Color = Color::Rgb(228, 104, 118);
const MUTED: Color = Color::Rgb(125, 133, 144);

pub fn render(frame: &mut Frame<'_>, model: &Model) {
    let area = frame.area();
    if area.width < 50 || area.height < 12 {
        render_too_small(frame, area);
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
        render_reader(frame, model, panes[0]);
        render_vocabulary(frame, model, panes[1]);
    } else {
        render_reader(frame, model, content);
    }
    render_status(frame, model, status);

    match model.mode() {
        Mode::OpenPath => render_path_overlay(frame, model, content),
        Mode::Vocabulary => render_vocabulary_overlay(frame, model, content),
        Mode::Review => render_review_overlay(frame, model, content),
        Mode::Reader => {}
    }
}

fn render_too_small(frame: &mut Frame<'_>, area: Rect) {
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new("Terminal too small\nMinimum size: 50 x 12")
            .alignment(Alignment::Center)
            .style(Style::default().fg(MUTED)),
        area,
    );
}

fn render_reader(frame: &mut Frame<'_>, model: &Model, area: Rect) {
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
        .map(|(line_index, _)| reader_line(model, line_index))
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(visible), inner);
}

fn reader_line(model: &Model, line_index: usize) -> Line<'static> {
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
            Style::default().fg(MUTED),
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
        spans.push(Span::styled(prefix.to_string(), Style::default().fg(MUTED)));
    }
    let content = block.text.get(visual.start..visual.end).unwrap_or_default();
    for (relative, grapheme) in content.grapheme_indices(true) {
        let offset = visual.start.saturating_add(relative);
        let style = reader_style(model, visual.block_index, offset, block.kind);
        spans.push(Span::styled(grapheme.to_string(), style));
    }
    Line::from(spans)
}

fn reader_style(model: &Model, block_index: usize, offset: usize, kind: BlockKind) -> Style {
    let mut style = match kind {
        BlockKind::Heading => Style::default().fg(FOCUS).add_modifier(Modifier::BOLD),
        BlockKind::Quote => Style::default().fg(MUTED).add_modifier(Modifier::ITALIC),
        BlockKind::Code => Style::default().fg(MUTED),
        _ => Style::default(),
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
                .fg(if due { DUE } else { SCHEDULED })
                .add_modifier(Modifier::UNDERLINED);
        }
        if model.active_token_index() == Some(token_index) {
            style = style
                .fg(FOCUS)
                .add_modifier(Modifier::BOLD | Modifier::REVERSED);
        }
    }
    if std::env::var_os("NO_COLOR").is_some() {
        style.fg = None;
        style.bg = None;
    }
    style
}

fn render_vocabulary(frame: &mut Frame<'_>, model: &Model, area: Rect) {
    let block = UiBlock::default()
        .borders(Borders::LEFT)
        .title(" Vocabulary ")
        .border_style(Style::default().fg(MUTED));
    let inner = block.inner(area).inner(Margin {
        horizontal: 1,
        vertical: 1,
    });
    frame.render_widget(block, area);
    frame.render_widget(
        Paragraph::new(vocabulary_lines(model)).wrap(Wrap { trim: false }),
        inner,
    );
}

fn vocabulary_lines(model: &Model) -> Vec<Line<'static>> {
    let Some(token) = model.active_token() else {
        return vec![Line::from(Span::styled(
            "No active word",
            Style::default().fg(MUTED),
        ))];
    };
    let mut lines = vec![Line::from(Span::styled(
        token.surface.clone(),
        Style::default().fg(FOCUS).add_modifier(Modifier::BOLD),
    ))];
    let Some(details) = model.active_word_details() else {
        lines.push(Line::from(Span::styled(
            "Looking up...",
            Style::default().fg(MUTED),
        )));
        return lines;
    };
    let Some(entry) = details.entry.as_ref() else {
        lines.push(Line::from(Span::styled(
            "Not in bundled lexicon",
            Style::default().fg(MUTED),
        )));
        return lines;
    };
    lines
        .push(Line::from(entry.lemma.clone()).style(Style::default().add_modifier(Modifier::BOLD)));
    lines.push(Line::from(format!("/{}/", entry.phonetic)).style(Style::default().fg(MUTED)));
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
            lines.push(Line::from("SRS").style(Style::default().fg(FOCUS)));
            lines.push(Line::from(format!(
                "Interval: {} days",
                state.interval_days
            )));
            let due = model
                .as_of()
                .is_some_and(|as_of| state.next_review_at <= as_of);
            lines.push(
                Line::from(if due { "Due now" } else { "Scheduled" })
                    .style(Style::default().fg(if due { DUE } else { SCHEDULED })),
            );
        }
        None => lines.push(Line::from("Not captured").style(Style::default().fg(MUTED))),
    }
    lines.push(Line::default());
    lines.push(Line::from("Context").style(Style::default().fg(FOCUS)));
    lines.push(Line::from(token.sentence.clone()));
    if let Some(document) = model.document() {
        lines.push(
            Line::from(document.path().display().to_string()).style(Style::default().fg(MUTED)),
        );
    }
    lines
}

fn render_vocabulary_overlay(frame: &mut Frame<'_>, model: &Model, area: Rect) {
    let overlay = area.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });
    frame.render_widget(Clear, overlay);
    let block = UiBlock::default()
        .borders(Borders::ALL)
        .title(" Vocabulary ")
        .border_style(Style::default().fg(FOCUS));
    let inner = block.inner(overlay).inner(Margin {
        horizontal: 1,
        vertical: 1,
    });
    frame.render_widget(block, overlay);
    frame.render_widget(
        Paragraph::new(vocabulary_lines(model)).wrap(Wrap { trim: false }),
        inner,
    );
}

fn render_status(frame: &mut Frame<'_>, model: &Model, area: Rect) {
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
        Paragraph::new(mode).style(Style::default().fg(FOCUS).add_modifier(Modifier::BOLD)),
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
    let center_style = model
        .notice()
        .filter(|notice| notice.is_error)
        .map_or_else(Style::default, |_| Style::default().fg(ERROR));
    frame.render_widget(Paragraph::new(center).style(center_style), chunks[1]);
    frame.render_widget(
        Paragraph::new(format!("{}%", model.progress_percent())).alignment(Alignment::Right),
        chunks[2],
    );
    let due = model
        .due_count()
        .map_or_else(|| "Due: -".to_string(), |count| format!("Due: {count}"));
    frame.render_widget(
        Paragraph::new(due)
            .alignment(Alignment::Right)
            .style(Style::default().fg(DUE)),
        chunks[3],
    );
}

fn render_path_overlay(frame: &mut Frame<'_>, model: &Model, area: Rect) {
    let width = area.width.saturating_sub(4).clamp(20, 72);
    let overlay = centered(area, width, 5.min(area.height));
    frame.render_widget(Clear, overlay);
    let block = UiBlock::default()
        .borders(Borders::ALL)
        .title(" Open file ")
        .border_style(Style::default().fg(FOCUS));
    let inner = block.inner(overlay).inner(Margin {
        horizontal: 1,
        vertical: 1,
    });
    frame.render_widget(block, overlay);
    frame.render_widget(Paragraph::new(model.path_input().to_string()), inner);
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

fn render_review_overlay(frame: &mut Frame<'_>, model: &Model, area: Rect) {
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
        .border_style(Style::default().fg(DUE));
    let inner = block.inner(overlay).inner(Margin {
        horizontal: 2,
        vertical: 1,
    });
    frame.render_widget(block, overlay);
    let Some(card) = model.review_card() else {
        frame.render_widget(
            Paragraph::new("Loading due reviews...")
                .alignment(Alignment::Center)
                .style(Style::default().fg(MUTED)),
            inner,
        );
        return;
    };
    let mut lines = vec![
        Line::from(card.word.lemma.clone())
            .style(Style::default().fg(FOCUS).add_modifier(Modifier::BOLD)),
        Line::default(),
    ];
    if let Some(context) = card.contexts.first() {
        lines.push(Line::from(context.sentence.clone()));
        lines.push(
            Line::from(format!("source: {}", context.source)).style(Style::default().fg(MUTED)),
        );
    }
    if model.review_revealed() {
        lines.push(Line::default());
        lines.push(Line::from(format!("/{}/", card.word.phonetic)).style(Style::default().fg(DUE)));
        for definition in card.word.definition.lines() {
            lines.push(Line::from(definition.to_string()));
        }
        lines.push(Line::default());
        lines.push(
            Line::from("0 Blackout  1 Incorrect  2 Familiar  3 Difficult  4 Good  5 Easy")
                .style(Style::default().fg(MUTED)),
        );
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
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
