use std::collections::HashSet;

use sentencex::get_sentence_boundaries;
use serde::{Deserialize, Serialize};
use unicode_segmentation::UnicodeSegmentation;

use crate::{Lexicon, LexiconEntry};

const ENGLISH_STOPWORDS: &str = include_str!("../assets/en_stopwords.txt");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseOptions {
    pub include_stopwords: bool,
    pub max_candidates: usize,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            include_stopwords: false,
            max_candidates: 100,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    pub surface: String,
    pub sentence: String,
    pub sentence_start: usize,
    pub sentence_end: usize,
    pub token_start: usize,
    pub token_end: usize,
    pub entry: LexiconEntry,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParseReport {
    pub candidates: Vec<Candidate>,
    pub lookup_misses: usize,
    pub filtered_stopwords: usize,
    pub truncated_candidates: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextSegmentation {
    pub sentences: Vec<SentenceSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SentenceSpan {
    pub start: usize,
    pub end: usize,
    pub words: Vec<WordSpan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WordSpan {
    pub start: usize,
    pub end: usize,
}

pub fn segment_text(text: &str) -> TextSegmentation {
    let mut sentences = Vec::new();
    for boundary in get_sentence_boundaries("en", text) {
        let (start, end) = trimmed_span(text, boundary.start_byte, boundary.end_byte);
        if start == end {
            continue;
        }
        let sentence = &text[start..end];
        let urls = url_spans(sentence);
        let words = token_spans(sentence)
            .into_iter()
            .filter(|(word_start, _)| {
                !urls
                    .iter()
                    .any(|(url_start, url_end)| word_start >= url_start && word_start < url_end)
            })
            .map(|(word_start, word_end)| WordSpan {
                start: start + word_start,
                end: start + word_end,
            })
            .collect();
        sentences.push(SentenceSpan { start, end, words });
    }
    TextSegmentation { sentences }
}

pub fn extract_candidates<L: Lexicon>(
    text: &str,
    lexicon: &L,
    options: ParseOptions,
) -> Result<ParseReport, L::Error> {
    let mut report = ParseReport::default();
    let mut seen_lemmas = HashSet::new();

    for sentence_span in segment_text(text).sentences {
        let sentence = &text[sentence_span.start..sentence_span.end];
        for word in sentence_span.words {
            let surface = &text[word.start..word.end];
            let normalized = surface.to_lowercase();
            if !options.include_stopwords && is_stopword(&normalized) {
                report.filtered_stopwords = report.filtered_stopwords.saturating_add(1);
                continue;
            }

            let Some(entry) = lexicon.lookup(&normalized)? else {
                report.lookup_misses = report.lookup_misses.saturating_add(1);
                continue;
            };
            let lemma_key = entry.lemma.to_lowercase();
            if !seen_lemmas.insert(lemma_key) {
                continue;
            }
            if report.candidates.len() >= options.max_candidates {
                report.truncated_candidates = report.truncated_candidates.saturating_add(1);
                continue;
            }

            report.candidates.push(Candidate {
                surface: surface.to_string(),
                sentence: sentence.to_string(),
                sentence_start: sentence_span.start,
                sentence_end: sentence_span.end,
                token_start: word.start,
                token_end: word.end,
                entry,
            });
        }
    }

    Ok(report)
}

fn trimmed_span(text: &str, start: usize, end: usize) -> (usize, usize) {
    let slice = &text[start..end];
    let leading = slice.len() - slice.trim_start().len();
    let trailing = slice.len() - slice.trim_end().len();
    (start + leading, end - trailing)
}

fn token_spans(sentence: &str) -> Vec<(usize, usize)> {
    let boundaries: Vec<(usize, &str)> = sentence.split_word_bound_indices().collect();
    let mut spans = Vec::new();
    let mut index = 0;

    while index < boundaries.len() {
        let (start, segment) = boundaries[index];
        if !is_word_segment(segment) {
            index += 1;
            continue;
        }

        let mut end = start + segment.len();
        let mut cursor = index + 1;
        while cursor + 1 < boundaries.len()
            && is_connector(boundaries[cursor].1)
            && is_word_segment(boundaries[cursor + 1].1)
        {
            end = boundaries[cursor + 1].0 + boundaries[cursor + 1].1.len();
            cursor += 2;
        }

        spans.push((start, end));
        index = cursor;
    }

    spans
}

fn url_spans(sentence: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut starts: Vec<usize> = ["https://", "http://", "www."]
        .into_iter()
        .flat_map(|prefix| sentence.match_indices(prefix).map(|(start, _)| start))
        .collect();
    starts.sort_unstable();
    starts.dedup();

    for start in starts {
        if spans
            .last()
            .is_some_and(|(_, previous_end)| start < *previous_end)
        {
            continue;
        }
        let remainder = &sentence[start..];
        let relative_end = match remainder.char_indices().find_map(|(index, character)| {
            (index > 0 && (character.is_whitespace() || is_url_delimiter(character)))
                .then_some(index)
        }) {
            Some(index) => index,
            None => remainder.len(),
        };
        spans.push((start, start + relative_end));
    }
    spans
}

fn is_url_delimiter(character: char) -> bool {
    matches!(character, '<' | '>' | '"' | '\'' | '“' | '”' | '‘' | '’')
}

fn is_word_segment(segment: &str) -> bool {
    segment.chars().any(char::is_alphabetic)
        && segment
            .chars()
            .all(|character| character.is_alphabetic() || is_connector_char(character))
}

fn is_connector(segment: &str) -> bool {
    matches!(segment, "'" | "’" | "-")
}

fn is_connector_char(character: char) -> bool {
    matches!(character, '\'' | '’' | '-')
}

fn is_stopword(word: &str) -> bool {
    ENGLISH_STOPWORDS.lines().any(|stopword| stopword == word)
}
