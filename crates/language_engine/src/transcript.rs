use core_engine::{
    MediaDomainError, TranscriptCue, TranscriptDocument, TranscriptFormat, TranscriptResource,
    TranscriptToken,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::segment_text;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimestampEndpoint {
    Start,
    End,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TranscriptParseError {
    #[error("transcript resource format is unsupported")]
    UnsupportedFormat,
    #[error("WebVTT header is missing at line {line}")]
    MissingWebVttHeader { line: usize },
    #[error("cue {cue_index} is missing a timing line at line {line}")]
    MissingTimingLine { cue_index: usize, line: usize },
    #[error("cue {cue_index} has an invalid {endpoint:?} timestamp at line {line}")]
    InvalidTimestamp {
        cue_index: usize,
        line: usize,
        endpoint: TimestampEndpoint,
    },
    #[error("cue {cue_index} {endpoint:?} timestamp overflows at line {line}")]
    TimestampOverflow {
        cue_index: usize,
        line: usize,
        endpoint: TimestampEndpoint,
    },
    #[error("transcript domain validation failed: {source}")]
    Domain {
        cue_index: Option<usize>,
        line: Option<usize>,
        #[source]
        source: MediaDomainError,
    },
}

impl TranscriptParseError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::UnsupportedFormat => "transcript_unsupported_format",
            Self::MissingWebVttHeader { .. } => "transcript_missing_webvtt_header",
            Self::MissingTimingLine { .. } => "transcript_missing_timing_line",
            Self::InvalidTimestamp { .. } => "transcript_invalid_timestamp",
            Self::TimestampOverflow { .. } => "transcript_timestamp_overflow",
            Self::Domain { source, .. } => match source {
                MediaDomainError::InvalidCueBounds { .. } => "transcript_invalid_cue_bounds",
                MediaDomainError::CueStartOutOfOrder { .. } => "transcript_cue_start_out_of_order",
                _ => "transcript_domain_error",
            },
        }
    }

    pub const fn cue_index(&self) -> Option<usize> {
        match self {
            Self::MissingTimingLine { cue_index, .. }
            | Self::InvalidTimestamp { cue_index, .. }
            | Self::TimestampOverflow { cue_index, .. } => Some(*cue_index),
            Self::Domain { cue_index, .. } => *cue_index,
            Self::UnsupportedFormat | Self::MissingWebVttHeader { .. } => None,
        }
    }

    pub const fn line(&self) -> Option<usize> {
        match self {
            Self::MissingWebVttHeader { line }
            | Self::MissingTimingLine { line, .. }
            | Self::InvalidTimestamp { line, .. }
            | Self::TimestampOverflow { line, .. } => Some(*line),
            Self::Domain { line, .. } => *line,
            Self::UnsupportedFormat => None,
        }
    }
}

pub fn parse_transcript(
    resource: TranscriptResource,
    source: &str,
) -> Result<TranscriptDocument, TranscriptParseError> {
    let format = resource
        .format
        .ok_or(TranscriptParseError::UnsupportedFormat)?;
    let normalized = source
        .strip_prefix('\u{feff}')
        .unwrap_or(source)
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    let parsed = match format {
        TranscriptFormat::WebVtt => parse_webvtt(&normalized)?,
        TranscriptFormat::Srt => parse_srt(&normalized)?,
    };
    build_document(resource, format, parsed)
}

#[derive(Debug)]
struct ParsedCue {
    physical_index: usize,
    timing_line: usize,
    source_id: Option<String>,
    start_ms: u64,
    end_ms: u64,
    text_lines: Vec<String>,
}

#[derive(Debug)]
struct Block<'a> {
    start_line: usize,
    lines: Vec<&'a str>,
}

fn parse_webvtt(source: &str) -> Result<Vec<ParsedCue>, TranscriptParseError> {
    let lines: Vec<&str> = source.split('\n').collect();
    let header = lines.first().copied().unwrap_or_default();
    if header != "WEBVTT" && !header.starts_with("WEBVTT ") && !header.starts_with("WEBVTT\t") {
        return Err(TranscriptParseError::MissingWebVttHeader { line: 1 });
    }
    let blocks = collect_blocks(lines.get(1..).unwrap_or_default(), 2);
    let mut cues = Vec::new();
    let mut physical_index = 0;
    for block in blocks {
        let first = block.lines[0].trim();
        if first == "NOTE" || first.starts_with("NOTE ") || first == "STYLE" || first == "REGION" {
            continue;
        }
        let (timing_offset, source_id) = if first.contains("-->") {
            (0, None)
        } else if block.lines.get(1).is_some_and(|line| line.contains("-->")) {
            (1, nonempty(first))
        } else {
            return Err(TranscriptParseError::MissingTimingLine {
                cue_index: physical_index,
                line: block.start_line,
            });
        };
        let timing_line = block.start_line + timing_offset;
        let (start_ms, end_ms) = parse_timing_line(
            block.lines[timing_offset],
            TranscriptFormat::WebVtt,
            physical_index,
            timing_line,
        )?;
        cues.push(ParsedCue {
            physical_index,
            timing_line,
            source_id,
            start_ms,
            end_ms,
            text_lines: block.lines[timing_offset + 1..]
                .iter()
                .map(|line| (*line).to_string())
                .collect(),
        });
        physical_index += 1;
    }
    Ok(cues)
}

fn parse_srt(source: &str) -> Result<Vec<ParsedCue>, TranscriptParseError> {
    if source.trim().is_empty() {
        return Ok(Vec::new());
    }
    let lines: Vec<&str> = source.split('\n').collect();
    let blocks = collect_blocks(&lines, 1);
    let mut cues = Vec::new();
    for (physical_index, block) in blocks.into_iter().enumerate() {
        let first = block.lines[0].trim();
        let (timing_offset, source_id) = if first.contains("-->") {
            (0, None)
        } else if first.chars().all(|character| character.is_ascii_digit())
            && block.lines.get(1).is_some_and(|line| line.contains("-->"))
        {
            (1, nonempty(first))
        } else {
            return Err(TranscriptParseError::MissingTimingLine {
                cue_index: physical_index,
                line: block.start_line,
            });
        };
        let timing_line = block.start_line + timing_offset;
        let (start_ms, end_ms) = parse_timing_line(
            block.lines[timing_offset],
            TranscriptFormat::Srt,
            physical_index,
            timing_line,
        )?;
        cues.push(ParsedCue {
            physical_index,
            timing_line,
            source_id,
            start_ms,
            end_ms,
            text_lines: block.lines[timing_offset + 1..]
                .iter()
                .map(|line| (*line).to_string())
                .collect(),
        });
    }
    Ok(cues)
}

fn collect_blocks<'a>(lines: &'a [&'a str], first_line: usize) -> Vec<Block<'a>> {
    let mut blocks = Vec::new();
    let mut current = Vec::new();
    let mut start_line = first_line;
    for (offset, line) in lines.iter().enumerate() {
        let line_number = first_line + offset;
        if line.trim().is_empty() {
            if !current.is_empty() {
                blocks.push(Block {
                    start_line,
                    lines: std::mem::take(&mut current),
                });
            }
            start_line = line_number + 1;
        } else {
            if current.is_empty() {
                start_line = line_number;
            }
            current.push(*line);
        }
    }
    if !current.is_empty() {
        blocks.push(Block {
            start_line,
            lines: current,
        });
    }
    blocks
}

fn parse_timing_line(
    line: &str,
    format: TranscriptFormat,
    cue_index: usize,
    line_number: usize,
) -> Result<(u64, u64), TranscriptParseError> {
    let (start, remainder) =
        line.split_once("-->")
            .ok_or(TranscriptParseError::MissingTimingLine {
                cue_index,
                line: line_number,
            })?;
    let end = remainder.split_whitespace().next().unwrap_or_default();
    let start_ms = parse_timestamp(
        start.trim(),
        format,
        cue_index,
        line_number,
        TimestampEndpoint::Start,
    )?;
    let end_ms = parse_timestamp(end, format, cue_index, line_number, TimestampEndpoint::End)?;
    Ok((start_ms, end_ms))
}

fn parse_timestamp(
    value: &str,
    format: TranscriptFormat,
    cue_index: usize,
    line: usize,
    endpoint: TimestampEndpoint,
) -> Result<u64, TranscriptParseError> {
    let invalid = || TranscriptParseError::InvalidTimestamp {
        cue_index,
        line,
        endpoint,
    };
    let overflow = || TranscriptParseError::TimestampOverflow {
        cue_index,
        line,
        endpoint,
    };
    let separator = match format {
        TranscriptFormat::WebVtt => '.',
        TranscriptFormat::Srt => {
            if value.contains(',') {
                ','
            } else {
                '.'
            }
        }
    };
    let (clock, fraction) = value.rsplit_once(separator).ok_or_else(invalid)?;
    if fraction.is_empty()
        || fraction.len() > 3
        || !fraction.chars().all(|character| character.is_ascii_digit())
    {
        return Err(invalid());
    }
    let fraction_value = fraction.parse::<u64>().map_err(|_| invalid())?;
    let fraction_ms = match fraction.len() {
        1 => fraction_value.checked_mul(100).ok_or_else(overflow)?,
        2 => fraction_value.checked_mul(10).ok_or_else(overflow)?,
        3 => fraction_value,
        _ => return Err(invalid()),
    };
    let parts: Vec<&str> = clock.split(':').collect();
    let (hours, minutes, seconds) = match (format, parts.as_slice()) {
        (TranscriptFormat::WebVtt, [minutes, seconds]) => (0, *minutes, *seconds),
        (_, [hours, minutes, seconds]) => {
            let hours = hours.parse::<u64>().map_err(|_| invalid())?;
            (hours, *minutes, *seconds)
        }
        _ => return Err(invalid()),
    };
    if minutes.len() != 2
        || seconds.len() != 2
        || !minutes.chars().all(|character| character.is_ascii_digit())
        || !seconds.chars().all(|character| character.is_ascii_digit())
    {
        return Err(invalid());
    }
    let minutes = minutes.parse::<u64>().map_err(|_| invalid())?;
    let seconds = seconds.parse::<u64>().map_err(|_| invalid())?;
    if minutes >= 60 || seconds >= 60 {
        return Err(invalid());
    }
    hours
        .checked_mul(3_600)
        .and_then(|value| value.checked_add(minutes * 60))
        .and_then(|value| value.checked_add(seconds))
        .and_then(|value| value.checked_mul(1_000))
        .and_then(|value| value.checked_add(fraction_ms))
        .ok_or_else(overflow)
}

fn build_document(
    resource: TranscriptResource,
    format: TranscriptFormat,
    parsed: Vec<ParsedCue>,
) -> Result<TranscriptDocument, TranscriptParseError> {
    let mut cues = Vec::new();
    let mut locations = Vec::new();
    for parsed_cue in parsed {
        let text = normalize_caption_text(&parsed_cue.text_lines);
        let source_order = u32::try_from(cues.len()).map_err(|_| TranscriptParseError::Domain {
            cue_index: Some(parsed_cue.physical_index),
            line: Some(parsed_cue.timing_line),
            source: MediaDomainError::CueIndexCapacityOverflow,
        })?;
        let cue_id = format!("cue-{source_order}");
        let tokens =
            transcript_tokens(&cue_id, &text).map_err(|source| TranscriptParseError::Domain {
                cue_index: Some(parsed_cue.physical_index),
                line: Some(parsed_cue.timing_line),
                source,
            })?;
        let cue = TranscriptCue::try_new(
            cue_id,
            source_order,
            parsed_cue.start_ms,
            parsed_cue.end_ms,
            text,
            tokens,
            format,
            parsed_cue.source_id,
        )
        .map_err(|source| TranscriptParseError::Domain {
            cue_index: Some(parsed_cue.physical_index),
            line: Some(parsed_cue.timing_line),
            source,
        })?;
        if cue.text().is_empty() {
            continue;
        }
        locations.push((parsed_cue.physical_index, parsed_cue.timing_line));
        cues.push(cue);
    }
    TranscriptDocument::try_new(resource, cues).map_err(|source| {
        let retained_index = domain_cue_index(&source);
        let (cue_index, line) = retained_index
            .and_then(|index| locations.get(index).copied())
            .map_or((None, None), |(cue_index, line)| {
                (Some(cue_index), Some(line))
            });
        TranscriptParseError::Domain {
            cue_index,
            line,
            source,
        }
    })
}

fn domain_cue_index(error: &MediaDomainError) -> Option<usize> {
    let cue_id = match error {
        MediaDomainError::InvalidCueBounds { cue_id, .. }
        | MediaDomainError::CueTextTooLong { cue_id }
        | MediaDomainError::TokenSpanOutOfBounds { cue_id, .. }
        | MediaDomainError::TokenSpanNotOnCharBoundary { cue_id, .. }
        | MediaDomainError::TokenSurfaceMismatch { cue_id, .. }
        | MediaDomainError::CueFormatMismatch { cue_id, .. }
        | MediaDomainError::DuplicateCueId { cue_id }
        | MediaDomainError::InvalidCueSourceOrder { cue_id, .. }
        | MediaDomainError::CueStartOutOfOrder { cue_id, .. } => cue_id,
        _ => return None,
    };
    cue_id.strip_prefix("cue-")?.parse().ok()
}

pub(crate) fn transcript_tokens(
    cue_id: &str,
    text: &str,
) -> Result<Vec<TranscriptToken>, MediaDomainError> {
    let mut tokens = Vec::new();
    for word in segment_text(text)
        .sentences
        .into_iter()
        .flat_map(|sentence| sentence.words)
    {
        let start_byte =
            u32::try_from(word.start).map_err(|_| MediaDomainError::CueTextTooLong {
                cue_id: cue_id.to_string(),
            })?;
        let end_byte = u32::try_from(word.end).map_err(|_| MediaDomainError::CueTextTooLong {
            cue_id: cue_id.to_string(),
        })?;
        let surface = text
            .get(word.start..word.end)
            .ok_or_else(|| MediaDomainError::TokenSpanNotOnCharBoundary {
                cue_id: cue_id.to_string(),
                offset: start_byte,
            })?
            .to_string();
        tokens.push(TranscriptToken::try_new(surface, start_byte, end_byte)?);
    }
    Ok(tokens)
}

fn normalize_caption_text(lines: &[String]) -> String {
    lines
        .iter()
        .map(|line| normalize_caption_line(line))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn normalize_caption_line(line: &str) -> String {
    let mut output = String::new();
    let mut cursor = 0;
    while cursor < line.len() {
        let remainder = &line[cursor..];
        if remainder.starts_with("{\\") {
            if let Some(end) = remainder.find('}') {
                cursor += end + 1;
                continue;
            }
        }
        if remainder.starts_with('<') {
            if let Some(end) = remainder.find('>') {
                let content = &remainder[1..end];
                if is_supported_caption_tag(content) {
                    cursor += end + 1;
                    continue;
                }
            }
        }
        if remainder.starts_with('&') {
            if let Some(end) = remainder.find(';') {
                let entity = &remainder[1..end];
                if let Some(character) = caption_entity(entity) {
                    output.push(character);
                    cursor += end + 1;
                    continue;
                }
            }
        }
        let character = remainder.chars().next().unwrap_or_default();
        output.push(character);
        cursor += character.len_utf8();
    }
    output.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_supported_caption_tag(content: &str) -> bool {
    let content = content.trim().trim_start_matches('/');
    if content
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_digit())
        && content.contains(':')
    {
        return true;
    }
    let name = content
        .split(|character: char| character.is_whitespace() || character == '.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(
        name.as_str(),
        "b" | "i" | "u" | "c" | "v" | "lang" | "ruby" | "rt" | "font"
    )
}

fn caption_entity(entity: &str) -> Option<char> {
    match entity {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "nbsp" => Some('\u{00a0}'),
        "lrm" => Some('\u{200e}'),
        "rlm" => Some('\u{200f}'),
        value if value.starts_with("#x") => u32::from_str_radix(&value[2..], 16)
            .ok()
            .and_then(char::from_u32),
        value if value.starts_with('#') => value[1..].parse().ok().and_then(char::from_u32),
        _ => None,
    }
}

fn nonempty(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}
