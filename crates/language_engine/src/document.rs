use std::path::{Path, PathBuf};

use crate::segment_text;
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentFormat {
    Markdown,
    PlainText,
}

impl DocumentFormat {
    pub fn from_path(path: &Path) -> Self {
        match path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_lowercase)
            .as_deref()
        {
            Some("md" | "markdown") => Self::Markdown,
            _ => Self::PlainText,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    Paragraph,
    Heading,
    ListItem,
    Quote,
    Code,
    Rule,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InlineStyle {
    pub bold: bool,
    pub italic: bool,
    pub underlined: bool,
    pub dim: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyledRange {
    pub start: usize,
    pub end: usize,
    pub style: InlineStyle,
    pub capturable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub kind: BlockKind,
    pub text: String,
    pub ranges: Vec<StyledRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentToken {
    pub block_index: usize,
    pub start: usize,
    pub end: usize,
    pub surface: String,
    pub sentence: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    path: PathBuf,
    blocks: Vec<Block>,
    tokens: Vec<DocumentToken>,
}

impl Document {
    pub fn parse(path: PathBuf, format: DocumentFormat, source: &str) -> Self {
        let normalized = sanitize_text(source);
        let blocks = match format {
            DocumentFormat::Markdown => parse_markdown(&normalized),
            DocumentFormat::PlainText => parse_plain_text(&normalized),
        };
        let tokens = collect_tokens(&blocks);
        Self {
            path,
            blocks,
            tokens,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }

    pub fn tokens(&self) -> &[DocumentToken] {
        &self.tokens
    }

    pub fn rendered_text(&self) -> String {
        self.blocks
            .iter()
            .map(|block| block.text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

fn parse_plain_text(source: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut paragraph = Vec::new();

    for line in source.lines() {
        if line.trim().is_empty() {
            push_plain_text_paragraph(&mut blocks, &mut paragraph);
        } else {
            paragraph.push(line);
        }
    }
    push_plain_text_paragraph(&mut blocks, &mut paragraph);

    blocks
}

fn push_plain_text_paragraph(blocks: &mut Vec<Block>, lines: &mut Vec<&str>) {
    if lines.is_empty() {
        return;
    }

    let text = lines.join("\n");
    lines.clear();
    blocks.push(Block {
        kind: BlockKind::Paragraph,
        ranges: vec![StyledRange {
            start: 0,
            end: text.len(),
            style: InlineStyle::default(),
            capturable: true,
        }],
        text,
    });
}

fn sanitize_text(source: &str) -> String {
    let source = source.replace("\r\n", "\n").replace('\r', "\n");
    let mut sanitized = String::with_capacity(source.len());
    let mut characters = source.chars().peekable();
    while let Some(character) = characters.next() {
        if character == '\u{1b}' {
            discard_escape_sequence(&mut characters);
            continue;
        }
        match character {
            '\n' => sanitized.push('\n'),
            '\t' => sanitized.push_str("    "),
            character if character.is_control() => sanitized.push(' '),
            character => sanitized.push(character),
        }
    }
    sanitized
}

fn discard_escape_sequence<I>(characters: &mut std::iter::Peekable<I>)
where
    I: Iterator<Item = char>,
{
    match characters.peek().copied() {
        Some('[') => {
            let _ = characters.next();
            for character in characters.by_ref() {
                if ('@'..='~').contains(&character) {
                    break;
                }
            }
        }
        Some(']') => {
            let _ = characters.next();
            while let Some(character) = characters.next() {
                if character == '\u{7}' {
                    break;
                }
                if character == '\u{1b}' && characters.next_if_eq(&'\\').is_some() {
                    break;
                }
            }
        }
        Some(_) => {
            let _ = characters.next();
        }
        None => {}
    }
}

fn collect_tokens(blocks: &[Block]) -> Vec<DocumentToken> {
    let mut tokens = Vec::new();
    for (block_index, block) in blocks.iter().enumerate() {
        if block.kind == BlockKind::Code || block.kind == BlockKind::Rule {
            continue;
        }
        let segmentation = segment_text(&block.text);
        for sentence in segmentation.sentences {
            let sentence_text = block.text[sentence.start..sentence.end].to_string();
            for word in sentence.words {
                if !block.ranges.iter().any(|range| {
                    range.capturable && word.start >= range.start && word.end <= range.end
                }) {
                    continue;
                }
                tokens.push(DocumentToken {
                    block_index,
                    start: word.start,
                    end: word.end,
                    surface: block.text[word.start..word.end].to_string(),
                    sentence: sentence_text.clone(),
                });
            }
        }
    }
    tokens
}

fn parse_markdown(source: &str) -> Vec<Block> {
    let options = Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TABLES
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS
        | Options::ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS;
    let mut parser = MarkdownBuilder::default();
    for event in Parser::new_ext(source, options) {
        parser.event(event);
    }
    parser.finish()
}

#[derive(Default)]
struct MarkdownBuilder {
    blocks: Vec<Block>,
    current: Option<BlockBuilder>,
    bold_depth: usize,
    italic_depth: usize,
    link_depth: usize,
    image_depth: usize,
    metadata_depth: usize,
    quote_depth: usize,
    item_depth: usize,
}

impl MarkdownBuilder {
    fn event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) => {
                if self.metadata_depth == 0 {
                    let capturable = self.image_depth == 0
                        && self
                            .current
                            .as_ref()
                            .is_none_or(|block| block.kind != BlockKind::Code);
                    self.append(&text, capturable, self.image_depth > 0);
                }
            }
            Event::Code(code) | Event::InlineMath(code) | Event::DisplayMath(code) => {
                if self.metadata_depth == 0 {
                    self.append(&code, false, true);
                }
            }
            Event::SoftBreak => self.append(" ", true, false),
            Event::HardBreak => self.append("\n", true, false),
            Event::Rule => {
                self.flush();
                self.blocks.push(Block {
                    kind: BlockKind::Rule,
                    text: String::new(),
                    ranges: Vec::new(),
                });
            }
            Event::TaskListMarker(checked) => {
                self.append(if checked { "[x] " } else { "[ ] " }, false, true);
            }
            Event::FootnoteReference(label) => self.append(&label, false, true),
            Event::Html(_) | Event::InlineHtml(_) => {}
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => {
                if self.current.is_none() {
                    let kind = if self.quote_depth > 0 {
                        BlockKind::Quote
                    } else if self.item_depth > 0 {
                        BlockKind::ListItem
                    } else {
                        BlockKind::Paragraph
                    };
                    self.begin(kind);
                }
            }
            Tag::Heading { .. } => self.begin(BlockKind::Heading),
            Tag::BlockQuote(_) => self.quote_depth = self.quote_depth.saturating_add(1),
            Tag::CodeBlock(_) => self.begin(BlockKind::Code),
            Tag::Item => {
                self.item_depth = self.item_depth.saturating_add(1);
                self.begin(BlockKind::ListItem);
            }
            Tag::Emphasis => self.italic_depth = self.italic_depth.saturating_add(1),
            Tag::Strong => self.bold_depth = self.bold_depth.saturating_add(1),
            Tag::Link { .. } => self.link_depth = self.link_depth.saturating_add(1),
            Tag::Image { .. } => self.image_depth = self.image_depth.saturating_add(1),
            Tag::MetadataBlock(_) => self.metadata_depth = self.metadata_depth.saturating_add(1),
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => {
                if self.item_depth == 0 {
                    self.flush();
                }
            }
            TagEnd::Heading(_) | TagEnd::CodeBlock => self.flush(),
            TagEnd::BlockQuote(_) => {
                self.flush();
                self.quote_depth = self.quote_depth.saturating_sub(1);
            }
            TagEnd::Item => {
                self.flush();
                self.item_depth = self.item_depth.saturating_sub(1);
            }
            TagEnd::Emphasis => self.italic_depth = self.italic_depth.saturating_sub(1),
            TagEnd::Strong => self.bold_depth = self.bold_depth.saturating_sub(1),
            TagEnd::Link => self.link_depth = self.link_depth.saturating_sub(1),
            TagEnd::Image => self.image_depth = self.image_depth.saturating_sub(1),
            TagEnd::MetadataBlock(_) => self.metadata_depth = self.metadata_depth.saturating_sub(1),
            TagEnd::TableCell => self.append(" | ", false, true),
            TagEnd::TableRow => self.append("\n", false, true),
            _ => {}
        }
    }

    fn begin(&mut self, kind: BlockKind) {
        if self
            .current
            .as_ref()
            .is_some_and(|block| !block.text.is_empty())
        {
            self.flush();
        }
        self.current = Some(BlockBuilder::new(kind));
    }

    fn append(&mut self, text: &str, capturable: bool, dim: bool) {
        if self.metadata_depth > 0 {
            return;
        }
        if self.current.is_none() {
            self.begin(if self.quote_depth > 0 {
                BlockKind::Quote
            } else {
                BlockKind::Paragraph
            });
        }
        if let Some(current) = self.current.as_mut() {
            current.append(
                text,
                InlineStyle {
                    bold: self.bold_depth > 0,
                    italic: self.italic_depth > 0,
                    underlined: self.link_depth > 0,
                    dim,
                },
                capturable,
            );
        }
    }

    fn flush(&mut self) {
        if let Some(block) = self.current.take().and_then(BlockBuilder::finish) {
            self.blocks.push(block);
        }
    }

    fn finish(mut self) -> Vec<Block> {
        self.flush();
        self.blocks
    }
}

struct BlockBuilder {
    kind: BlockKind,
    text: String,
    ranges: Vec<StyledRange>,
}

impl BlockBuilder {
    fn new(kind: BlockKind) -> Self {
        Self {
            kind,
            text: String::new(),
            ranges: Vec::new(),
        }
    }

    fn append(&mut self, text: &str, style: InlineStyle, capturable: bool) {
        let start = self.text.len();
        self.text.push_str(text);
        self.ranges.push(StyledRange {
            start,
            end: self.text.len(),
            style,
            capturable,
        });
    }

    fn finish(self) -> Option<Block> {
        if self.text.trim().is_empty() && self.kind != BlockKind::Rule {
            return None;
        }
        Some(Block {
            kind: self.kind,
            text: self.text.trim().to_string(),
            ranges: trim_ranges(&self.text, self.ranges),
        })
    }
}

fn trim_ranges(text: &str, ranges: Vec<StyledRange>) -> Vec<StyledRange> {
    let leading = text.len().saturating_sub(text.trim_start().len());
    let trailing_end = text.trim_end().len();
    ranges
        .into_iter()
        .filter_map(|range| {
            let start = range.start.max(leading);
            let end = range.end.min(trailing_end);
            (start < end).then_some(StyledRange {
                start: start - leading,
                end: end - leading,
                style: range.style,
                capturable: range.capturable,
            })
        })
        .collect()
}
