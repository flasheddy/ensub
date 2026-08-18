use std::collections::BTreeMap;

use core_engine::{
    calculate_padded_audio_slice, reconcile_episode_identity, PodcastContextDraft, PodcastEpisode,
    PodcastEpisodeProvenance, PodcastFeed, PodcastFeedProvenance, TranscriptDocument,
    TranscriptFormat, TranscriptProvenance, TranscriptResource,
};
use language_engine::{parse_podcast_feed, parse_transcript, reconstruct_transcript_context};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ingestion::transcript_document_dto;
use crate::{IngestionError, PodcastEpisodeDto, PodcastFeedDto, TranscriptDocumentDto};

pub const PLAYER_CACHE_FORMAT: &str = "ensub-player-cache";
pub const PLAYER_CACHE_SCHEMA_VERSION: u32 = 1;
pub const MAX_PLAYER_FEED_BYTES: usize = 5 * 1024 * 1024;
pub const MAX_PLAYER_TRANSCRIPT_BYTES: usize = 10 * 1024 * 1024;
pub const MAX_PLAYER_CUES: usize = 50_000;
pub const MAX_PLAYER_CACHE_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum PlayerWorkspaceError {
    #[error("feed exceeds the 5 MiB player limit")]
    FeedTooLarge,
    #[error("transcript exceeds the 10 MiB player limit")]
    TranscriptTooLarge,
    #[error("transcript exceeds the 50,000 cue player limit")]
    TooManyCues,
    #[error("player cache exceeds the 64 MiB restore limit")]
    CacheTooLarge,
    #[error("player cache could not be decoded: {0}")]
    InvalidCache(String),
    #[error("unsupported player cache format or schema")]
    UnsupportedCache,
    #[error("revision capacity was exhausted")]
    RevisionOverflow,
    #[error("feed was not found")]
    FeedNotFound,
    #[error("episode was not found")]
    EpisodeNotFound,
    #[error("transcript resource does not belong to the episode")]
    TranscriptNotFound,
    #[error("no transcript is selected")]
    NoTranscriptSelected,
    #[error("lookup selection is stale")]
    StaleSelection,
    #[error("selected cue or token was not found")]
    SelectionNotFound,
    #[error("selected transcript has no supported format")]
    UnsupportedTranscript,
    #[error("capture media context is invalid: {0}")]
    InvalidMedia(String),
    #[error(transparent)]
    Ingestion(#[from] IngestionError),
}

impl PlayerWorkspaceError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::FeedTooLarge => "player_feed_too_large",
            Self::TranscriptTooLarge => "player_transcript_too_large",
            Self::TooManyCues => "player_too_many_cues",
            Self::CacheTooLarge => "player_cache_too_large",
            Self::InvalidCache(_) => "player_cache_invalid",
            Self::UnsupportedCache => "player_cache_unsupported",
            Self::RevisionOverflow => "player_revision_overflow",
            Self::FeedNotFound => "player_feed_not_found",
            Self::EpisodeNotFound => "player_episode_not_found",
            Self::TranscriptNotFound => "player_transcript_not_found",
            Self::NoTranscriptSelected => "player_transcript_not_selected",
            Self::StaleSelection => "player_selection_stale",
            Self::SelectionNotFound => "player_selection_not_found",
            Self::UnsupportedTranscript => "player_transcript_unsupported",
            Self::InvalidMedia(_) => "player_media_invalid",
            Self::Ingestion(error) => error.code(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PlayerCacheEnvelope {
    format: String,
    schema_version: u32,
    cache: PlayerCache,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct PlayerCache {
    revision: u64,
    feeds: BTreeMap<String, CachedFeed>,
    transcripts: BTreeMap<String, CachedTranscript>,
    selected_feed_url: Option<String>,
    selected_episode_id: Option<String>,
    selected_transcript_url: Option<String>,
    last_transcript_language: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedFeed {
    feed: PodcastFeed,
    episodes: Vec<PodcastEpisode>,
    fetched_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedTranscript {
    episode_id: String,
    transcript_url: String,
    document: TranscriptDocument,
    fetched_at_ms: i64,
}

#[derive(Debug, Clone)]
pub struct PlayerWorkspace {
    cache: PlayerCache,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptStateDto {
    None,
    UnsupportedOnly,
    ChoiceRequired,
    Loading,
    Ready,
    Cached,
    Offline,
    Malformed,
    Empty,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerWorkspaceDto {
    pub revision: u64,
    pub feeds: Vec<PodcastFeedDto>,
    pub episodes: Vec<PodcastEpisodeDto>,
    pub selected_feed_url: Option<String>,
    pub selected_episode_id: Option<String>,
    pub selected_transcript_url: Option<String>,
    pub last_transcript_language: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeOpenDto {
    pub revision: u64,
    pub episode: PodcastEpisodeDto,
    pub selected_transcript_url: Option<String>,
    pub transcript_state: TranscriptStateDto,
    pub transcript: Option<TranscriptDocumentDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSyncDto {
    pub active_cue_indices: Vec<u32>,
    pub anchor_cue_index: Option<u32>,
    pub preceding_cue_index: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparePodcastCaptureInput {
    pub revision: u64,
    pub episode_id: String,
    pub transcript_url: String,
    pub cue_id: String,
    pub token_index: usize,
    pub playback_position_ms: u64,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedPodcastCaptureDto {
    pub surface: String,
    pub draft: PodcastContextDraft,
}

impl PlayerWorkspace {
    pub fn open(snapshot: &[u8]) -> Result<Self, PlayerWorkspaceError> {
        if snapshot.is_empty() {
            return Ok(Self {
                cache: PlayerCache::default(),
            });
        }
        if snapshot.len() > MAX_PLAYER_CACHE_BYTES {
            return Err(PlayerWorkspaceError::CacheTooLarge);
        }
        let envelope: PlayerCacheEnvelope = serde_json::from_slice(snapshot)
            .map_err(|error| PlayerWorkspaceError::InvalidCache(error.to_string()))?;
        if envelope.format != PLAYER_CACHE_FORMAT
            || envelope.schema_version != PLAYER_CACHE_SCHEMA_VERSION
        {
            return Err(PlayerWorkspaceError::UnsupportedCache);
        }
        validate_cache(&envelope.cache)?;
        Ok(Self {
            cache: envelope.cache,
        })
    }

    pub fn view(&self) -> PlayerWorkspaceDto {
        let feeds = self
            .cache
            .feeds
            .values()
            .map(|entry| PodcastFeedDto::from(entry.feed.clone()))
            .collect();
        let episodes = self
            .selected_feed()
            .map(|entry| {
                entry
                    .episodes
                    .iter()
                    .cloned()
                    .map(PodcastEpisodeDto::from)
                    .collect()
            })
            .unwrap_or_default();
        PlayerWorkspaceDto {
            revision: self.cache.revision,
            feeds,
            episodes,
            selected_feed_url: self.cache.selected_feed_url.clone(),
            selected_episode_id: self.cache.selected_episode_id.clone(),
            selected_transcript_url: self.cache.selected_transcript_url.clone(),
            last_transcript_language: self.cache.last_transcript_language.clone(),
        }
    }

    pub fn import_feed(
        &mut self,
        source_url: &str,
        xml: &[u8],
        fetched_at_ms: i64,
    ) -> Result<PlayerWorkspaceDto, PlayerWorkspaceError> {
        if xml.len() > MAX_PLAYER_FEED_BYTES {
            return Err(PlayerWorkspaceError::FeedTooLarge);
        }
        let report = parse_podcast_feed(source_url, xml)
            .map_err(IngestionError::from)
            .map_err(PlayerWorkspaceError::from)?;
        let mut candidate = self.cache.clone();
        let previous = candidate.feeds.get(source_url);
        let episodes = report
            .episodes
            .into_iter()
            .map(|mut observed| {
                if let Some(existing) = previous.and_then(|feed| {
                    feed.episodes.iter().find(|episode| {
                        reconcile_episode_identity(&episode.identity, &observed.identity).is_some()
                    })
                }) {
                    if let Some(identity) =
                        reconcile_episode_identity(&existing.identity, &observed.identity)
                    {
                        observed.identity = identity;
                    }
                }
                observed
            })
            .collect();
        candidate.feeds.insert(
            source_url.to_string(),
            CachedFeed {
                feed: report.feed,
                episodes,
                fetched_at_ms,
            },
        );
        candidate.selected_feed_url = Some(source_url.to_string());
        if candidate
            .selected_episode_id
            .as_deref()
            .is_some_and(|id| find_episode(&candidate, id).is_none())
        {
            candidate.selected_episode_id = None;
            candidate.selected_transcript_url = None;
        }
        bump_revision(&mut candidate)?;
        self.cache = candidate;
        Ok(self.view())
    }

    pub fn select_episode(
        &mut self,
        episode_id: &str,
    ) -> Result<EpisodeOpenDto, PlayerWorkspaceError> {
        let episode = find_episode(&self.cache, episode_id)
            .ok_or(PlayerWorkspaceError::EpisodeNotFound)?
            .clone();
        let selected = preferred_resource(&episode, self.cache.last_transcript_language.as_deref())
            .map(|resource| resource.url.clone());
        let mut candidate = self.cache.clone();
        candidate.selected_feed_url = Some(episode.identity.feed_url.clone());
        candidate.selected_episode_id = Some(episode_id.to_string());
        candidate.selected_transcript_url = selected;
        bump_revision(&mut candidate)?;
        self.cache = candidate;
        self.open_episode(&episode)
    }

    pub fn select_transcript(
        &mut self,
        episode_id: &str,
        transcript_url: &str,
    ) -> Result<EpisodeOpenDto, PlayerWorkspaceError> {
        let episode = find_episode(&self.cache, episode_id)
            .ok_or(PlayerWorkspaceError::EpisodeNotFound)?
            .clone();
        let resource = episode
            .transcript_resources
            .iter()
            .find(|resource| resource.url == transcript_url && resource.format.is_some())
            .ok_or(PlayerWorkspaceError::TranscriptNotFound)?;
        let mut candidate = self.cache.clone();
        candidate.selected_feed_url = Some(episode.identity.feed_url.clone());
        candidate.selected_episode_id = Some(episode_id.to_string());
        candidate.selected_transcript_url = Some(transcript_url.to_string());
        candidate.last_transcript_language = resource.language.clone();
        bump_revision(&mut candidate)?;
        self.cache = candidate;
        self.open_episode(&episode)
    }

    pub fn cache_transcript(
        &mut self,
        episode_id: &str,
        transcript_url: &str,
        source: &str,
        fetched_at_ms: i64,
    ) -> Result<EpisodeOpenDto, PlayerWorkspaceError> {
        if source.len() > MAX_PLAYER_TRANSCRIPT_BYTES {
            return Err(PlayerWorkspaceError::TranscriptTooLarge);
        }
        let episode = find_episode(&self.cache, episode_id)
            .ok_or(PlayerWorkspaceError::EpisodeNotFound)?
            .clone();
        let resource = episode
            .transcript_resources
            .iter()
            .find(|resource| resource.url == transcript_url && resource.format.is_some())
            .ok_or(PlayerWorkspaceError::TranscriptNotFound)?
            .clone();
        let document = parse_transcript(resource.clone(), source)
            .map_err(IngestionError::from)
            .map_err(PlayerWorkspaceError::from)?;
        if document.cues().len() > MAX_PLAYER_CUES {
            return Err(PlayerWorkspaceError::TooManyCues);
        }

        let mut candidate = self.cache.clone();
        candidate.transcripts.insert(
            transcript_key(episode_id, transcript_url),
            CachedTranscript {
                episode_id: episode_id.to_string(),
                transcript_url: transcript_url.to_string(),
                document,
                fetched_at_ms,
            },
        );
        candidate.selected_feed_url = Some(episode.identity.feed_url.clone());
        candidate.selected_episode_id = Some(episode_id.to_string());
        candidate.selected_transcript_url = Some(transcript_url.to_string());
        candidate.last_transcript_language = resource.language;
        bump_revision(&mut candidate)?;
        validate_snapshot_size(&candidate)?;
        self.cache = candidate;
        self.open_episode(&episode)
    }

    pub fn sync_at(
        &self,
        playback_position_ms: u64,
    ) -> Result<TranscriptSyncDto, PlayerWorkspaceError> {
        let transcript = self.selected_transcript()?;
        let active_cue_indices = transcript.document.active_cue_indices(playback_position_ms);
        let anchor_cue_index = active_cue_indices.first().copied();
        let preceding_cue_index = if active_cue_indices.is_empty() {
            transcript
                .document
                .cues()
                .iter()
                .rposition(|cue| cue.end_ms() <= playback_position_ms)
                .and_then(|index| u32::try_from(index).ok())
        } else {
            None
        };
        Ok(TranscriptSyncDto {
            active_cue_indices,
            anchor_cue_index,
            preceding_cue_index,
        })
    }

    pub fn prepare_podcast_capture(
        &self,
        input: &PreparePodcastCaptureInput,
    ) -> Result<PreparedPodcastCaptureDto, PlayerWorkspaceError> {
        if input.revision != self.cache.revision
            || self.cache.selected_episode_id.as_deref() != Some(input.episode_id.as_str())
            || self.cache.selected_transcript_url.as_deref() != Some(input.transcript_url.as_str())
        {
            return Err(PlayerWorkspaceError::StaleSelection);
        }
        let episode = find_episode(&self.cache, &input.episode_id)
            .ok_or(PlayerWorkspaceError::StaleSelection)?;
        let feed = self
            .cache
            .feeds
            .get(&episode.identity.feed_url)
            .ok_or(PlayerWorkspaceError::StaleSelection)?;
        let transcript = self.selected_transcript()?;
        let reconstructed =
            reconstruct_transcript_context(&transcript.document, &input.cue_id, input.token_index)
                .map_err(|_| PlayerWorkspaceError::SelectionNotFound)?;
        let format = transcript
            .document
            .resource()
            .format
            .ok_or(PlayerWorkspaceError::UnsupportedTranscript)?;
        let audio_slice = calculate_padded_audio_slice(
            episode.enclosure_url.clone(),
            &reconstructed.cue_range,
            input.duration_ms,
        )
        .map_err(|error| PlayerWorkspaceError::InvalidMedia(error.to_string()))?;
        let surface = reconstructed.selected_token.surface().to_string();
        Ok(PreparedPodcastCaptureDto {
            surface,
            draft: PodcastContextDraft {
                sentence: reconstructed.sentence,
                quality: reconstructed.quality,
                feed: PodcastFeedProvenance {
                    source_url: feed.feed.source_url.clone(),
                    title: feed.feed.title.clone(),
                },
                episode: PodcastEpisodeProvenance {
                    internal_id: episode.identity.internal_id.clone(),
                    publisher_guid: episode.publisher_guid.clone(),
                    title: episode.title.clone(),
                    published_at: episode.published_at,
                    enclosure_url: episode.enclosure_url.clone(),
                },
                transcript: TranscriptProvenance {
                    url: transcript.transcript_url.clone(),
                    format,
                    language: transcript.document.resource().language.clone(),
                },
                selected_cue_id: reconstructed.selected_cue_id,
                selected_token: reconstructed.selected_token,
                cue_range: reconstructed.cue_range,
                playback_position_ms: input.playback_position_ms,
                audio_slice,
            },
        })
    }

    pub fn snapshot(&self) -> Result<Vec<u8>, PlayerWorkspaceError> {
        encode_snapshot(&self.cache)
    }

    fn selected_feed(&self) -> Option<&CachedFeed> {
        self.cache
            .selected_feed_url
            .as_ref()
            .and_then(|url| self.cache.feeds.get(url))
    }

    fn selected_transcript(&self) -> Result<&CachedTranscript, PlayerWorkspaceError> {
        let episode_id = self
            .cache
            .selected_episode_id
            .as_deref()
            .ok_or(PlayerWorkspaceError::NoTranscriptSelected)?;
        let transcript_url = self
            .cache
            .selected_transcript_url
            .as_deref()
            .ok_or(PlayerWorkspaceError::NoTranscriptSelected)?;
        self.cache
            .transcripts
            .get(&transcript_key(episode_id, transcript_url))
            .ok_or(PlayerWorkspaceError::NoTranscriptSelected)
    }

    fn open_episode(
        &self,
        episode: &PodcastEpisode,
    ) -> Result<EpisodeOpenDto, PlayerWorkspaceError> {
        let selected_url = self.cache.selected_transcript_url.clone();
        let cached = selected_url.as_ref().and_then(|url| {
            self.cache
                .transcripts
                .get(&transcript_key(&episode.identity.internal_id, url))
        });
        let supported_count = episode
            .transcript_resources
            .iter()
            .filter(|resource| resource.format.is_some())
            .count();
        let transcript_state = if let Some(entry) = cached {
            if entry.document.cues().is_empty() {
                TranscriptStateDto::Empty
            } else {
                TranscriptStateDto::Ready
            }
        } else if selected_url.is_some() {
            TranscriptStateDto::Loading
        } else if episode.transcript_resources.is_empty() {
            TranscriptStateDto::None
        } else if supported_count == 0 {
            TranscriptStateDto::UnsupportedOnly
        } else {
            TranscriptStateDto::ChoiceRequired
        };
        let transcript = cached
            .map(|entry| transcript_document_dto(&entry.document))
            .transpose()?;
        Ok(EpisodeOpenDto {
            revision: self.cache.revision,
            episode: episode.clone().into(),
            selected_transcript_url: selected_url,
            transcript_state,
            transcript,
        })
    }
}

fn validate_cache(cache: &PlayerCache) -> Result<(), PlayerWorkspaceError> {
    for transcript in cache.transcripts.values() {
        if transcript.document.cues().len() > MAX_PLAYER_CUES {
            return Err(PlayerWorkspaceError::TooManyCues);
        }
        let episode = find_episode(cache, &transcript.episode_id)
            .ok_or_else(|| PlayerWorkspaceError::InvalidCache("orphan transcript".to_string()))?;
        if !episode
            .transcript_resources
            .iter()
            .any(|resource| resource.url == transcript.transcript_url)
        {
            return Err(PlayerWorkspaceError::InvalidCache(
                "transcript resource mismatch".to_string(),
            ));
        }
    }
    Ok(())
}

fn find_episode<'a>(cache: &'a PlayerCache, episode_id: &str) -> Option<&'a PodcastEpisode> {
    cache
        .feeds
        .values()
        .flat_map(|feed| &feed.episodes)
        .find(|episode| episode.identity.internal_id == episode_id)
}

fn preferred_resource<'a>(
    episode: &'a PodcastEpisode,
    language: Option<&str>,
) -> Option<&'a TranscriptResource> {
    let supported = |resource: &&TranscriptResource| {
        matches!(
            resource.format,
            Some(TranscriptFormat::WebVtt | TranscriptFormat::Srt)
        )
    };
    episode
        .transcript_resources
        .iter()
        .filter(supported)
        .find(|resource| language.is_some_and(|value| resource.language.as_deref() == Some(value)))
        .or_else(|| episode.transcript_resources.iter().find(supported))
}

fn transcript_key(episode_id: &str, transcript_url: &str) -> String {
    format!("{episode_id}\n{transcript_url}")
}

fn bump_revision(cache: &mut PlayerCache) -> Result<(), PlayerWorkspaceError> {
    cache.revision = cache
        .revision
        .checked_add(1)
        .ok_or(PlayerWorkspaceError::RevisionOverflow)?;
    Ok(())
}

fn validate_snapshot_size(cache: &PlayerCache) -> Result<(), PlayerWorkspaceError> {
    let _ = encode_snapshot(cache)?;
    Ok(())
}

fn encode_snapshot(cache: &PlayerCache) -> Result<Vec<u8>, PlayerWorkspaceError> {
    let envelope = PlayerCacheEnvelope {
        format: PLAYER_CACHE_FORMAT.to_string(),
        schema_version: PLAYER_CACHE_SCHEMA_VERSION,
        cache: cache.clone(),
    };
    let encoded = serde_json::to_vec(&envelope)
        .map_err(|error| PlayerWorkspaceError::InvalidCache(error.to_string()))?;
    if encoded.len() > MAX_PLAYER_CACHE_BYTES {
        return Err(PlayerWorkspaceError::CacheTooLarge);
    }
    Ok(encoded)
}

impl From<language_engine::PodcastFeedParseError> for PlayerWorkspaceError {
    fn from(error: language_engine::PodcastFeedParseError) -> Self {
        Self::Ingestion(IngestionError::Feed(error))
    }
}

impl From<language_engine::TranscriptParseError> for PlayerWorkspaceError {
    fn from(error: language_engine::TranscriptParseError) -> Self {
        Self::Ingestion(IngestionError::Transcript(error))
    }
}
