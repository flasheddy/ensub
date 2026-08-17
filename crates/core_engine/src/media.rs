use std::collections::HashSet;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::MediaDomainError;

pub const AUDIO_SLICE_PADDING_MS: u64 = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptFormat {
    WebVtt,
    Srt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PodcastContextQuality {
    CompleteSentence,
    FallbackCueWindow,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PodcastFeed {
    pub source_url: String,
    pub title: String,
    pub language: Option<String>,
    pub description: Option<String>,
    pub artwork_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpisodeIdentity {
    pub internal_id: String,
    pub feed_url: String,
    pub publisher_guid_aliases: Vec<String>,
    pub enclosure_url_aliases: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PodcastEpisode {
    pub identity: EpisodeIdentity,
    pub publisher_guid: Option<String>,
    pub title: String,
    pub published_at: Option<DateTime<Utc>>,
    pub language: Option<String>,
    pub enclosure_url: String,
    pub enclosure_mime_type: String,
    pub duration_ms: Option<u64>,
    pub artwork_url: Option<String>,
    pub transcript_resources: Vec<TranscriptResource>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranscriptResource {
    pub url: String,
    pub mime_type: String,
    pub format: Option<TranscriptFormat>,
    pub language: Option<String>,
    pub relation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "UncheckedTranscriptToken")]
pub struct TranscriptToken {
    surface: String,
    start_byte: u32,
    end_byte: u32,
}

impl TranscriptToken {
    pub fn try_new(
        surface: String,
        start_byte: u32,
        end_byte: u32,
    ) -> Result<Self, MediaDomainError> {
        if start_byte >= end_byte {
            return Err(MediaDomainError::InvalidTokenBounds {
                start_byte,
                end_byte,
            });
        }

        Ok(Self {
            surface,
            start_byte,
            end_byte,
        })
    }

    pub fn surface(&self) -> &str {
        &self.surface
    }

    pub const fn start_byte(&self) -> u32 {
        self.start_byte
    }

    pub const fn end_byte(&self) -> u32 {
        self.end_byte
    }
}

#[derive(Deserialize)]
struct UncheckedTranscriptToken {
    surface: String,
    start_byte: u32,
    end_byte: u32,
}

impl TryFrom<UncheckedTranscriptToken> for TranscriptToken {
    type Error = MediaDomainError;

    fn try_from(value: UncheckedTranscriptToken) -> Result<Self, Self::Error> {
        Self::try_new(value.surface, value.start_byte, value.end_byte)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "UncheckedTranscriptCue")]
pub struct TranscriptCue {
    id: String,
    source_order: u32,
    start_ms: u64,
    end_ms: u64,
    text: String,
    tokens: Vec<TranscriptToken>,
    source_format: TranscriptFormat,
    source_cue_id: Option<String>,
}

impl TranscriptCue {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        id: String,
        source_order: u32,
        start_ms: u64,
        end_ms: u64,
        text: String,
        tokens: Vec<TranscriptToken>,
        source_format: TranscriptFormat,
        source_cue_id: Option<String>,
    ) -> Result<Self, MediaDomainError> {
        if start_ms >= end_ms {
            return Err(MediaDomainError::InvalidCueBounds {
                cue_id: id,
                start_ms,
                end_ms,
            });
        }

        let text_len_bytes = u32::try_from(text.len())
            .map_err(|_| MediaDomainError::CueTextTooLong { cue_id: id.clone() })?;
        for token in &tokens {
            validate_token_span(&id, &text, text_len_bytes, token)?;
        }

        Ok(Self {
            id,
            source_order,
            start_ms,
            end_ms,
            text,
            tokens,
            source_format,
            source_cue_id,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub const fn source_order(&self) -> u32 {
        self.source_order
    }

    pub const fn start_ms(&self) -> u64 {
        self.start_ms
    }

    pub const fn end_ms(&self) -> u64 {
        self.end_ms
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn tokens(&self) -> &[TranscriptToken] {
        &self.tokens
    }

    pub const fn source_format(&self) -> TranscriptFormat {
        self.source_format
    }

    pub fn source_cue_id(&self) -> Option<&str> {
        self.source_cue_id.as_deref()
    }
}

fn validate_token_span(
    cue_id: &str,
    text: &str,
    text_len_bytes: u32,
    token: &TranscriptToken,
) -> Result<(), MediaDomainError> {
    if token.end_byte > text_len_bytes {
        return Err(MediaDomainError::TokenSpanOutOfBounds {
            cue_id: cue_id.to_string(),
            start_byte: token.start_byte,
            end_byte: token.end_byte,
            text_len_bytes,
        });
    }

    let start =
        usize::try_from(token.start_byte).map_err(|_| MediaDomainError::TokenSpanOutOfBounds {
            cue_id: cue_id.to_string(),
            start_byte: token.start_byte,
            end_byte: token.end_byte,
            text_len_bytes,
        })?;
    let end =
        usize::try_from(token.end_byte).map_err(|_| MediaDomainError::TokenSpanOutOfBounds {
            cue_id: cue_id.to_string(),
            start_byte: token.start_byte,
            end_byte: token.end_byte,
            text_len_bytes,
        })?;

    if !text.is_char_boundary(start) {
        return Err(MediaDomainError::TokenSpanNotOnCharBoundary {
            cue_id: cue_id.to_string(),
            offset: token.start_byte,
        });
    }
    if !text.is_char_boundary(end) {
        return Err(MediaDomainError::TokenSpanNotOnCharBoundary {
            cue_id: cue_id.to_string(),
            offset: token.end_byte,
        });
    }
    if text.get(start..end) != Some(token.surface.as_str()) {
        return Err(MediaDomainError::TokenSurfaceMismatch {
            cue_id: cue_id.to_string(),
            start_byte: token.start_byte,
            end_byte: token.end_byte,
        });
    }

    Ok(())
}

#[derive(Deserialize)]
struct UncheckedTranscriptCue {
    id: String,
    source_order: u32,
    start_ms: u64,
    end_ms: u64,
    text: String,
    tokens: Vec<TranscriptToken>,
    source_format: TranscriptFormat,
    source_cue_id: Option<String>,
}

impl TryFrom<UncheckedTranscriptCue> for TranscriptCue {
    type Error = MediaDomainError;

    fn try_from(value: UncheckedTranscriptCue) -> Result<Self, Self::Error> {
        Self::try_new(
            value.id,
            value.source_order,
            value.start_ms,
            value.end_ms,
            value.text,
            value.tokens,
            value.source_format,
            value.source_cue_id,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "UncheckedTranscriptDocument")]
pub struct TranscriptDocument {
    resource: TranscriptResource,
    cues: Vec<TranscriptCue>,
    #[serde(skip)]
    active_index: ActiveCueIndex,
}

impl TranscriptDocument {
    pub fn try_new(
        resource: TranscriptResource,
        cues: Vec<TranscriptCue>,
    ) -> Result<Self, MediaDomainError> {
        let expected_format = resource
            .format
            .ok_or(MediaDomainError::UnsupportedTranscriptResource)?;
        let mut cue_ids = HashSet::with_capacity(cues.len());
        let mut previous: Option<&TranscriptCue> = None;

        for (index, cue) in cues.iter().enumerate() {
            let expected_order =
                u32::try_from(index).map_err(|_| MediaDomainError::CueIndexCapacityOverflow)?;
            if cue.source_order != expected_order {
                return Err(MediaDomainError::InvalidCueSourceOrder {
                    cue_id: cue.id.clone(),
                    expected: expected_order,
                    actual: cue.source_order,
                });
            }
            if cue.source_format != expected_format {
                return Err(MediaDomainError::CueFormatMismatch {
                    cue_id: cue.id.clone(),
                    expected: expected_format,
                    actual: cue.source_format,
                });
            }
            if !cue_ids.insert(cue.id.as_str()) {
                return Err(MediaDomainError::DuplicateCueId {
                    cue_id: cue.id.clone(),
                });
            }
            if let Some(previous_cue) = previous {
                if cue.start_ms < previous_cue.start_ms {
                    return Err(MediaDomainError::CueStartOutOfOrder {
                        previous_cue_id: previous_cue.id.clone(),
                        previous_start_ms: previous_cue.start_ms,
                        cue_id: cue.id.clone(),
                        start_ms: cue.start_ms,
                    });
                }
            }
            previous = Some(cue);
        }

        let active_index = ActiveCueIndex::try_new(&cues)?;
        Ok(Self {
            resource,
            cues,
            active_index,
        })
    }

    pub fn resource(&self) -> &TranscriptResource {
        &self.resource
    }

    pub fn cues(&self) -> &[TranscriptCue] {
        &self.cues
    }

    pub fn active_cue_indices(&self, playback_position_ms: u64) -> Vec<u32> {
        self.active_cue_indices_with_stats(playback_position_ms).0
    }

    fn active_cue_indices_with_stats(
        &self,
        playback_position_ms: u64,
    ) -> (Vec<u32>, CueLookupStats) {
        let mut stats = CueLookupStats::default();
        let upper = self.cues.partition_point(|cue| {
            stats.partition_probes += 1;
            cue.start_ms <= playback_position_ms
        });
        let mut active = Vec::new();
        self.active_index.collect_active(
            &self.cues,
            upper,
            playback_position_ms,
            &mut active,
            &mut stats,
        );
        (active, stats)
    }
}

#[derive(Deserialize)]
struct UncheckedTranscriptDocument {
    resource: TranscriptResource,
    cues: Vec<TranscriptCue>,
}

impl TryFrom<UncheckedTranscriptDocument> for TranscriptDocument {
    type Error = MediaDomainError;

    fn try_from(value: UncheckedTranscriptDocument) -> Result<Self, Self::Error> {
        Self::try_new(value.resource, value.cues)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ActiveCueIndex {
    leaf_count: usize,
    max_end_ms: Vec<u64>,
}

impl ActiveCueIndex {
    fn try_new(cues: &[TranscriptCue]) -> Result<Self, MediaDomainError> {
        if cues.is_empty() {
            return Ok(Self::default());
        }

        let leaf_count = cues
            .len()
            .checked_next_power_of_two()
            .ok_or(MediaDomainError::CueIndexCapacityOverflow)?;
        let tree_len = leaf_count
            .checked_mul(2)
            .ok_or(MediaDomainError::CueIndexCapacityOverflow)?;
        let mut max_end_ms = vec![0; tree_len];

        for (index, cue) in cues.iter().enumerate() {
            max_end_ms[leaf_count + index] = cue.end_ms;
        }
        for node in (1..leaf_count).rev() {
            max_end_ms[node] = max_end_ms[node * 2].max(max_end_ms[node * 2 + 1]);
        }

        Ok(Self {
            leaf_count,
            max_end_ms,
        })
    }

    fn collect_active(
        &self,
        cues: &[TranscriptCue],
        upper: usize,
        playback_position_ms: u64,
        active: &mut Vec<u32>,
        stats: &mut CueLookupStats,
    ) {
        if self.leaf_count == 0 || upper == 0 {
            return;
        }

        self.collect_node(
            1,
            0,
            self.leaf_count,
            cues,
            upper,
            playback_position_ms,
            active,
            stats,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn collect_node(
        &self,
        node: usize,
        node_start: usize,
        node_end: usize,
        cues: &[TranscriptCue],
        upper: usize,
        playback_position_ms: u64,
        active: &mut Vec<u32>,
        stats: &mut CueLookupStats,
    ) {
        stats.tree_node_visits += 1;
        if node_start >= upper || self.max_end_ms[node] <= playback_position_ms {
            return;
        }

        if node_end - node_start == 1 {
            if let Some(cue) = cues.get(node_start) {
                if cue.start_ms <= playback_position_ms && playback_position_ms < cue.end_ms {
                    active.push(cue.source_order);
                }
            }
            return;
        }

        let middle = node_start + (node_end - node_start) / 2;
        self.collect_node(
            node * 2,
            node_start,
            middle,
            cues,
            upper,
            playback_position_ms,
            active,
            stats,
        );
        self.collect_node(
            node * 2 + 1,
            middle,
            node_end,
            cues,
            upper,
            playback_position_ms,
            active,
            stats,
        );
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct CueLookupStats {
    partition_probes: usize,
    tree_node_visits: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "UncheckedCueRange")]
pub struct CueRange {
    first_cue_id: String,
    last_cue_id: String,
    start_ms: u64,
    end_ms: u64,
}

impl CueRange {
    pub fn try_new(
        first_cue_id: String,
        last_cue_id: String,
        start_ms: u64,
        end_ms: u64,
    ) -> Result<Self, MediaDomainError> {
        if start_ms >= end_ms {
            return Err(MediaDomainError::InvalidCueRangeBounds { start_ms, end_ms });
        }

        Ok(Self {
            first_cue_id,
            last_cue_id,
            start_ms,
            end_ms,
        })
    }

    pub fn try_from_cues(cues: &[TranscriptCue]) -> Result<Self, MediaDomainError> {
        let mut cues_iter = cues.iter();
        let first = cues_iter.next().ok_or(MediaDomainError::EmptyCueRange)?;
        let mut start_ms = first.start_ms;
        let mut end_ms = first.end_ms;

        for cue in cues_iter {
            start_ms = start_ms.min(cue.start_ms);
            end_ms = end_ms.max(cue.end_ms);
        }

        let last = cues.last().ok_or(MediaDomainError::EmptyCueRange)?;
        Self::try_new(first.id.clone(), last.id.clone(), start_ms, end_ms)
    }

    pub fn first_cue_id(&self) -> &str {
        &self.first_cue_id
    }

    pub fn last_cue_id(&self) -> &str {
        &self.last_cue_id
    }

    pub const fn start_ms(&self) -> u64 {
        self.start_ms
    }

    pub const fn end_ms(&self) -> u64 {
        self.end_ms
    }
}

#[derive(Deserialize)]
struct UncheckedCueRange {
    first_cue_id: String,
    last_cue_id: String,
    start_ms: u64,
    end_ms: u64,
}

impl TryFrom<UncheckedCueRange> for CueRange {
    type Error = MediaDomainError;

    fn try_from(value: UncheckedCueRange) -> Result<Self, Self::Error> {
        Self::try_new(
            value.first_cue_id,
            value.last_cue_id,
            value.start_ms,
            value.end_ms,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "UncheckedAudioSlice")]
pub struct AudioSlice {
    audio_source_url: String,
    start_ms: u64,
    end_ms: u64,
}

impl AudioSlice {
    pub fn try_new(
        audio_source_url: String,
        start_ms: u64,
        end_ms: u64,
    ) -> Result<Self, MediaDomainError> {
        Self::try_new_with_duration(audio_source_url, start_ms, end_ms, None)
    }

    fn try_new_with_duration(
        audio_source_url: String,
        start_ms: u64,
        end_ms: u64,
        duration_ms: Option<u64>,
    ) -> Result<Self, MediaDomainError> {
        if start_ms >= end_ms {
            return Err(MediaDomainError::InvalidAudioSliceBounds {
                start_ms,
                end_ms,
                duration_ms,
            });
        }

        Ok(Self {
            audio_source_url,
            start_ms,
            end_ms,
        })
    }

    pub fn audio_source_url(&self) -> &str {
        &self.audio_source_url
    }

    pub const fn start_ms(&self) -> u64 {
        self.start_ms
    }

    pub const fn end_ms(&self) -> u64 {
        self.end_ms
    }
}

#[derive(Deserialize)]
struct UncheckedAudioSlice {
    audio_source_url: String,
    start_ms: u64,
    end_ms: u64,
}

impl TryFrom<UncheckedAudioSlice> for AudioSlice {
    type Error = MediaDomainError;

    fn try_from(value: UncheckedAudioSlice) -> Result<Self, Self::Error> {
        Self::try_new(value.audio_source_url, value.start_ms, value.end_ms)
    }
}

pub fn calculate_padded_audio_slice(
    audio_source_url: String,
    cue_range: &CueRange,
    duration_ms: Option<u64>,
) -> Result<AudioSlice, MediaDomainError> {
    let slice_start_ms = cue_range.start_ms.saturating_sub(AUDIO_SLICE_PADDING_MS);
    let padded_end_ms = cue_range.end_ms.checked_add(AUDIO_SLICE_PADDING_MS).ok_or(
        MediaDomainError::PaddedEndOverflow {
            end_ms: cue_range.end_ms,
            padding_ms: AUDIO_SLICE_PADDING_MS,
        },
    )?;
    let slice_end_ms = duration_ms
        .map(|duration| duration.min(padded_end_ms))
        .unwrap_or(padded_end_ms);

    AudioSlice::try_new_with_duration(audio_source_url, slice_start_ms, slice_end_ms, duration_ms)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PodcastFeedProvenance {
    pub source_url: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PodcastEpisodeProvenance {
    pub internal_id: String,
    pub publisher_guid: Option<String>,
    pub title: String,
    pub published_at: Option<DateTime<Utc>>,
    pub enclosure_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranscriptProvenance {
    pub url: String,
    pub format: TranscriptFormat,
    pub language: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PodcastContext {
    pub sentence: String,
    pub quality: PodcastContextQuality,
    pub feed: PodcastFeedProvenance,
    pub episode: PodcastEpisodeProvenance,
    pub transcript: TranscriptProvenance,
    pub selected_token: TranscriptToken,
    pub normalized_lemma: String,
    pub cue_range: CueRange,
    pub playback_position_ms: u64,
    pub audio_slice: AudioSlice,
    pub captured_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn performance_document(cue_count: u32) -> TranscriptDocument {
        let mut cues = Vec::with_capacity(cue_count as usize);
        cues.push(
            TranscriptCue::try_new(
                "cue-0".to_string(),
                0,
                0,
                u64::from(cue_count) + 1,
                String::new(),
                Vec::new(),
                TranscriptFormat::WebVtt,
                None,
            )
            .expect("test cue must be valid"),
        );
        for source_order in 1..cue_count {
            cues.push(
                TranscriptCue::try_new(
                    format!("cue-{source_order}"),
                    source_order,
                    u64::from(source_order),
                    u64::from(source_order) + 1,
                    String::new(),
                    Vec::new(),
                    TranscriptFormat::WebVtt,
                    None,
                )
                .expect("test cue must be valid"),
            );
        }

        TranscriptDocument::try_new(
            TranscriptResource {
                url: "https://media.example.test/transcript.vtt".to_string(),
                mime_type: "text/vtt".to_string(),
                format: Some(TranscriptFormat::WebVtt),
                language: Some("en".to_string()),
                relation: None,
            },
            cues,
        )
        .expect("test document must be valid")
    }

    #[test]
    fn indexed_lookup_has_a_logarithmic_probe_budget_for_sparse_matches() {
        let cases = [(1_024, 11, 48), (65_536, 17, 72)];

        for (cue_count, max_partition_probes, max_tree_visits) in cases {
            let document = performance_document(cue_count);
            let (active, stats) = document.active_cue_indices_with_stats(u64::from(cue_count));

            assert_eq!(active, vec![0]);
            assert!(
                stats.partition_probes <= max_partition_probes,
                "{} partition probes exceeded budget {}",
                stats.partition_probes,
                max_partition_probes
            );
            assert!(
                stats.tree_node_visits <= max_tree_visits,
                "{} tree visits exceeded budget {}",
                stats.tree_node_visits,
                max_tree_visits
            );
        }
    }
}
