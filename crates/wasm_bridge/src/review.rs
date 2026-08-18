use std::collections::BTreeMap;

use core_engine::{PodcastContextQuality, ReviewQueueItem, ReviewState};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DueCountInputDto {
    pub as_of_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DueCountDto {
    pub due_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DueReviewsInputDto {
    pub as_of_ms: i64,
    pub limit: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DueReviewsDto {
    pub as_of_ms: i64,
    pub cards: Vec<ReviewPromptDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPromptDto {
    pub word_id: String,
    pub review_token: String,
    pub headword: String,
    pub next_review_at_ms: i64,
    pub default_context_id: Option<String>,
    pub contexts: Vec<ReviewContextDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewContextDto {
    pub context_id: String,
    pub sentence: String,
    pub selected_surface: Option<String>,
    pub source_label: String,
    pub episode_title: Option<String>,
    pub context_quality: Option<PodcastContextQualityDto>,
    pub captured_at_ms: i64,
    pub audio_slice: Option<AudioSlicePlaybackDto>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PodcastContextQualityDto {
    CompleteSentence,
    FallbackCueWindow,
}

impl From<PodcastContextQuality> for PodcastContextQualityDto {
    fn from(value: PodcastContextQuality) -> Self {
        match value {
            PodcastContextQuality::CompleteSentence => Self::CompleteSentence,
            PodcastContextQuality::FallbackCueWindow => Self::FallbackCueWindow,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioSlicePlaybackDto {
    pub audio_source_url: String,
    pub slice_start_ms: u64,
    pub slice_end_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevealReviewInputDto {
    pub word_id: String,
    pub review_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewAnswerDto {
    pub word_id: String,
    pub review_token: String,
    pub term: String,
    pub lemma: String,
    pub phonetic: String,
    pub definition: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateReviewInputDto {
    pub word_id: String,
    pub review_token: String,
    pub rating: u8,
    pub reviewed_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewTransitionDto {
    pub word_id: String,
    pub rating: u8,
    pub reviewed_at_ms: i64,
    pub ease_factor: f64,
    pub repetitions: u32,
    pub interval_days: u32,
    pub next_review_at_ms: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalReviewState<'a> {
    version: u8,
    word_id: &'a str,
    ease_factor_bits: String,
    repetitions: u32,
    interval_days: u32,
    next_review_unix_seconds: i64,
    next_review_subsec_nanos: u32,
    last_rating: Option<u8>,
}

pub(crate) fn review_token(state: &ReviewState) -> Result<String, serde_json::Error> {
    let canonical = CanonicalReviewState {
        version: 1,
        word_id: state.word_id.as_str(),
        ease_factor_bits: format!("{:016x}", state.ease_factor.to_bits()),
        repetitions: state.repetitions,
        interval_days: state.interval_days,
        next_review_unix_seconds: state.next_review_at.timestamp(),
        next_review_subsec_nanos: state.next_review_at.timestamp_subsec_nanos(),
        last_rating: state.last_rating.map(core_engine::ReviewRating::value),
    };
    let encoded = serde_json::to_vec(&canonical)?;
    Ok(format!("rs1.{:x}", Sha256::digest(encoded)))
}

pub(crate) fn prompt_from_item(
    item: ReviewQueueItem,
) -> Result<ReviewPromptDto, serde_json::Error> {
    let headword = item.card.word.term.clone();
    let mut podcast_contexts = item
        .podcast_contexts
        .into_iter()
        .map(|record| (record.context_id.as_str().to_string(), record))
        .collect::<BTreeMap<_, _>>();
    let contexts = item
        .card
        .contexts
        .into_iter()
        .map(|context| {
            let podcast = podcast_contexts.remove(context.id.as_str());
            let episode_title = podcast
                .as_ref()
                .map(|record| record.context.episode.title.clone());
            let source_label = episode_title
                .clone()
                .unwrap_or_else(|| context.source.clone());
            let context_quality = podcast.as_ref().map(|record| record.context.quality.into());
            let selected_surface = podcast
                .as_ref()
                .map(|record| record.context.selected_token.surface().to_string());
            let audio_slice = podcast.as_ref().map(|record| AudioSlicePlaybackDto {
                audio_source_url: record.context.audio_slice.audio_source_url().to_string(),
                slice_start_ms: record.context.audio_slice.start_ms(),
                slice_end_ms: record.context.audio_slice.end_ms(),
            });
            ReviewContextDto {
                context_id: context.id.into_inner(),
                sentence: context.sentence,
                selected_surface,
                source_label,
                episode_title,
                context_quality,
                captured_at_ms: context.captured_at.timestamp_millis(),
                audio_slice,
            }
        })
        .collect::<Vec<_>>();
    Ok(ReviewPromptDto {
        word_id: item.card.word.id.into_inner(),
        review_token: review_token(&item.card.state)?,
        headword,
        next_review_at_ms: item.card.state.next_review_at.timestamp_millis(),
        default_context_id: contexts.first().map(|context| context.context_id.clone()),
        contexts,
    })
}
