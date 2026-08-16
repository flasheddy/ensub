use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::{BlockKind, Document};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisualLine {
    pub text: String,
    pub block_index: usize,
    pub start: usize,
    pub end: usize,
    pub prefix_width: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenPlacement {
    pub token_index: usize,
    pub line: usize,
    pub x: u16,
    pub width: u16,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DocumentLayout {
    width: u16,
    lines: Vec<VisualLine>,
    placements: Vec<TokenPlacement>,
}

impl DocumentLayout {
    pub fn new(document: &Document, width: u16) -> Self {
        if width == 0 {
            return Self::default();
        }

        let mut lines = Vec::new();
        for (block_index, block) in document.blocks().iter().enumerate() {
            if block_index > 0 {
                lines.push(VisualLine {
                    text: String::new(),
                    block_index,
                    start: 0,
                    end: 0,
                    prefix_width: 0,
                });
            }
            if block.kind == BlockKind::Rule {
                lines.push(VisualLine {
                    text: "-".repeat(usize::from(width)),
                    block_index,
                    start: 0,
                    end: 0,
                    prefix_width: 0,
                });
                continue;
            }
            let prefix = block_prefix(block.kind);
            let prefix_width = u16::try_from(UnicodeWidthStr::width(prefix)).unwrap_or(u16::MAX);
            let available = width.saturating_sub(prefix_width).max(1);
            for (start, end) in wrap_ranges(&block.text, available) {
                let content = block.text.get(start..end).unwrap_or_default();
                lines.push(VisualLine {
                    text: format!("{prefix}{content}"),
                    block_index,
                    start,
                    end,
                    prefix_width,
                });
            }
        }

        let mut placements = Vec::new();
        for (token_index, token) in document.tokens().iter().enumerate() {
            if let Some((line_index, line)) = lines.iter().enumerate().find(|(_, line)| {
                line.block_index == token.block_index
                    && token.start >= line.start
                    && token.start < line.end
            }) {
                let block = &document.blocks()[token.block_index];
                let before = block.text.get(line.start..token.start).unwrap_or_default();
                let surface = block.text.get(token.start..token.end).unwrap_or_default();
                placements.push(TokenPlacement {
                    token_index,
                    line: line_index,
                    x: line
                        .prefix_width
                        .saturating_add(width_as_u16(UnicodeWidthStr::width(before))),
                    width: width_as_u16(UnicodeWidthStr::width(surface)),
                });
            }
        }

        Self {
            width,
            lines,
            placements,
        }
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn lines(&self) -> &[VisualLine] {
        &self.lines
    }

    pub fn placements(&self) -> &[TokenPlacement] {
        &self.placements
    }

    pub fn placement(&self, token_index: usize) -> Option<&TokenPlacement> {
        self.placements
            .iter()
            .find(|placement| placement.token_index == token_index)
    }

    pub fn nearest_token_on_line(&self, line: usize, preferred_x: u16) -> Option<usize> {
        self.placements
            .iter()
            .filter(|placement| placement.line == line)
            .min_by_key(|placement| placement.x.abs_diff(preferred_x))
            .map(|placement| placement.token_index)
    }
}

fn block_prefix(kind: BlockKind) -> &'static str {
    match kind {
        BlockKind::ListItem => "- ",
        BlockKind::Quote => "> ",
        BlockKind::Code => "  ",
        BlockKind::Paragraph | BlockKind::Heading | BlockKind::Rule => "",
    }
}

fn wrap_ranges(text: &str, width: u16) -> Vec<(usize, usize)> {
    let mut result = Vec::new();
    let mut logical_start = 0;
    for logical in text.split_inclusive('\n') {
        let content = logical.strip_suffix('\n').unwrap_or(logical);
        wrap_logical_line(content, logical_start, width, &mut result);
        logical_start = logical_start.saturating_add(logical.len());
    }
    if text.is_empty() || text.ends_with('\n') {
        result.push((text.len(), text.len()));
    }
    result
}

fn wrap_logical_line(line: &str, base: usize, width: u16, result: &mut Vec<(usize, usize)>) {
    if line.is_empty() {
        result.push((base, base));
        return;
    }

    let maximum = usize::from(width.max(1));
    let mut start = 0;
    while start < line.len() {
        while start < line.len() {
            let Some(character) = line[start..].chars().next() else {
                break;
            };
            if !character.is_whitespace() {
                break;
            }
            start += character.len_utf8();
        }
        if start >= line.len() {
            break;
        }

        let mut used: usize = 0;
        let mut end = start;
        let mut last_whitespace = None;
        for (relative, grapheme) in line[start..].grapheme_indices(true) {
            let grapheme_width = UnicodeWidthStr::width(grapheme);
            if used.saturating_add(grapheme_width) > maximum && end > start {
                break;
            }
            used = used.saturating_add(grapheme_width);
            end = start + relative + grapheme.len();
            if grapheme.chars().all(char::is_whitespace) {
                last_whitespace = Some(start + relative);
            }
            if used >= maximum {
                break;
            }
        }
        if end < line.len() {
            if let Some(space) = last_whitespace.filter(|space| *space > start) {
                end = space;
            }
        }
        if end == start {
            let next = line[start..]
                .chars()
                .next()
                .map(char::len_utf8)
                .unwrap_or(1);
            end = start.saturating_add(next).min(line.len());
        }
        let trimmed_end = line[start..end].trim_end().len() + start;
        result.push((base + start, base + trimmed_end.max(start)));
        start = end;
    }
}

fn width_as_u16(width: usize) -> u16 {
    u16::try_from(width).unwrap_or(u16::MAX)
}
