use chrono::{DateTime, Utc};
use core_engine::{
    MediaDomainError, PodcastEpisode, PodcastFeed, TranscriptCue, TranscriptDocument,
    TranscriptFormat, TranscriptResource,
};
use serde::Deserialize;
use thiserror::Error;
use url::Url;

use crate::podcast_feed::episode_identity;
use crate::transcript::transcript_tokens;

pub const PODCAST_FIXTURE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PodcastFixture {
    pub feed: PodcastFeed,
    pub episode: PodcastEpisode,
    pub transcript: TranscriptDocument,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PodcastFixtureError {
    #[error("podcast fixture JSON is invalid: {message}")]
    InvalidJson { message: String },
    #[error("podcast fixture schema version {schema_version} is unsupported")]
    UnsupportedSchema { schema_version: String },
    #[error("podcast fixture source URL is invalid")]
    InvalidSourceUrl,
    #[error("podcast fixture field {field} does not resolve to a valid HTTP(S) URL")]
    InvalidResolvedUrl { field: &'static str },
    #[error("podcast fixture metadata field {field} is invalid")]
    InvalidMetadata { field: &'static str },
    #[error("podcast fixture must contain at least one transcript cue")]
    EmptyCues,
    #[error("podcast fixture cue {cue_index} field {field} is invalid")]
    InvalidCue {
        cue_index: usize,
        field: &'static str,
    },
    #[error("podcast fixture transcript domain validation failed: {source}")]
    Domain {
        cue_index: Option<usize>,
        #[source]
        source: MediaDomainError,
    },
}

impl PodcastFixtureError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidJson { .. } => "podcast_fixture_invalid_json",
            Self::UnsupportedSchema { .. } => "podcast_fixture_unsupported_schema",
            Self::InvalidSourceUrl => "podcast_fixture_invalid_source_url",
            Self::InvalidResolvedUrl { .. } => "podcast_fixture_invalid_resolved_url",
            Self::InvalidMetadata { .. } => "podcast_fixture_invalid_metadata",
            Self::EmptyCues => "podcast_fixture_empty_cues",
            Self::InvalidCue { .. } => "podcast_fixture_invalid_cue",
            Self::Domain { .. } => "podcast_fixture_transcript_domain_error",
        }
    }

    pub const fn cue_index(&self) -> Option<usize> {
        match self {
            Self::InvalidCue { cue_index, .. } => Some(*cue_index),
            Self::Domain { cue_index, .. } => *cue_index,
            Self::InvalidJson { .. }
            | Self::UnsupportedSchema { .. }
            | Self::InvalidSourceUrl
            | Self::InvalidResolvedUrl { .. }
            | Self::InvalidMetadata { .. }
            | Self::EmptyCues => None,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPodcastFixture {
    schema_version: serde_json::Number,
    feed: RawFeed,
    episode: RawEpisode,
    transcript: RawTranscript,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFeed {
    title: String,
    language: String,
    description: String,
    artwork_url: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEpisode {
    publisher_guid: String,
    title: String,
    published_at: String,
    language: String,
    enclosure_url: String,
    enclosure_mime_type: String,
    duration_ms: serde_json::Number,
    artwork_url: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTranscript {
    url: String,
    mime_type: String,
    format: String,
    language: String,
    relation: String,
    cues: Vec<RawCue>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCue {
    source_cue_id: String,
    start_ms: serde_json::Number,
    end_ms: serde_json::Number,
    text: String,
}

pub fn parse_podcast_fixture(
    source_url: &str,
    json_bytes: &[u8],
) -> Result<PodcastFixture, PodcastFixtureError> {
    let raw: RawPodcastFixture =
        serde_json::from_slice(json_bytes).map_err(|error| PodcastFixtureError::InvalidJson {
            message: error.to_string(),
        })?;
    if raw.schema_version.as_u64() != Some(u64::from(PODCAST_FIXTURE_SCHEMA_VERSION)) {
        return Err(PodcastFixtureError::UnsupportedSchema {
            schema_version: raw.schema_version.to_string(),
        });
    }

    let source_url = canonical_source_url(source_url)?;
    let feed_title = metadata(raw.feed.title, "feed.title")?;
    let feed_language = metadata(raw.feed.language, "feed.language")?;
    let feed_description = metadata(raw.feed.description, "feed.description")?;
    let feed_artwork_url = resource_url(
        &source_url,
        &raw.feed.artwork_url,
        "feed.artwork_url",
        false,
    )?;

    let publisher_guid = metadata(raw.episode.publisher_guid, "episode.publisher_guid")?;
    let episode_title = metadata(raw.episode.title, "episode.title")?;
    let published_at = DateTime::parse_from_rfc3339(raw.episode.published_at.trim())
        .map(|value| value.with_timezone(&Utc))
        .map_err(|_| PodcastFixtureError::InvalidMetadata {
            field: "episode.published_at",
        })?;
    let episode_language = metadata(raw.episode.language, "episode.language")?;
    let enclosure_url = resource_url(
        &source_url,
        &raw.episode.enclosure_url,
        "episode.enclosure_url",
        false,
    )?;
    let enclosure_mime_type = normalized_mime(&raw.episode.enclosure_mime_type)
        .filter(|value| value.starts_with("audio/"))
        .ok_or(PodcastFixtureError::InvalidMetadata {
            field: "episode.enclosure_mime_type",
        })?;
    let duration_ms = raw
        .episode
        .duration_ms
        .as_u64()
        .filter(|value| *value > 0)
        .ok_or(PodcastFixtureError::InvalidMetadata {
            field: "episode.duration_ms",
        })?;
    let episode_artwork_url = resource_url(
        &source_url,
        &raw.episode.artwork_url,
        "episode.artwork_url",
        false,
    )?;

    let transcript_url = resource_url(&source_url, &raw.transcript.url, "transcript.url", true)?;
    let transcript_mime_type = normalized_mime(&raw.transcript.mime_type)
        .filter(|value| value == "text/vtt")
        .ok_or(PodcastFixtureError::InvalidMetadata {
            field: "transcript.mime_type",
        })?;
    if raw.transcript.format != "web_vtt" {
        return Err(PodcastFixtureError::InvalidMetadata {
            field: "transcript.format",
        });
    }
    let transcript_language = metadata(raw.transcript.language, "transcript.language")?;
    if raw.transcript.relation != "captions" {
        return Err(PodcastFixtureError::InvalidMetadata {
            field: "transcript.relation",
        });
    }
    if raw.transcript.cues.is_empty() {
        return Err(PodcastFixtureError::EmptyCues);
    }

    let resource = TranscriptResource {
        url: transcript_url,
        mime_type: transcript_mime_type,
        format: Some(TranscriptFormat::WebVtt),
        language: Some(transcript_language.clone()),
        relation: Some("captions".to_string()),
    };
    let mut cues = Vec::with_capacity(raw.transcript.cues.len());
    let mut previous_start_ms = None;
    for (cue_index, raw_cue) in raw.transcript.cues.into_iter().enumerate() {
        let source_cue_id = cue_metadata(raw_cue.source_cue_id, cue_index, "source_cue_id")?;
        if raw_cue.text.trim().is_empty() {
            return Err(PodcastFixtureError::InvalidCue {
                cue_index,
                field: "text",
            });
        }
        let start_ms = cue_number(&raw_cue.start_ms, cue_index, "start_ms")?;
        let end_ms = cue_number(&raw_cue.end_ms, cue_index, "end_ms")?;
        if end_ms == 0 || start_ms >= end_ms {
            return Err(PodcastFixtureError::InvalidCue {
                cue_index,
                field: "bounds",
            });
        }
        if previous_start_ms.is_some_and(|previous| start_ms < previous) {
            return Err(PodcastFixtureError::InvalidCue {
                cue_index,
                field: "start_ms",
            });
        }
        if end_ms > duration_ms {
            return Err(PodcastFixtureError::InvalidCue {
                cue_index,
                field: "end_ms",
            });
        }

        let source_order = u32::try_from(cue_index).map_err(|_| PodcastFixtureError::Domain {
            cue_index: Some(cue_index),
            source: MediaDomainError::CueIndexCapacityOverflow,
        })?;
        let cue_id = format!("cue-{source_order}");
        let tokens = transcript_tokens(&cue_id, &raw_cue.text).map_err(|source| {
            PodcastFixtureError::Domain {
                cue_index: Some(cue_index),
                source,
            }
        })?;
        let cue = TranscriptCue::try_new(
            cue_id,
            source_order,
            start_ms,
            end_ms,
            raw_cue.text,
            tokens,
            TranscriptFormat::WebVtt,
            Some(source_cue_id),
        )
        .map_err(|source| PodcastFixtureError::Domain {
            cue_index: Some(cue_index),
            source,
        })?;
        previous_start_ms = Some(start_ms);
        cues.push(cue);
    }

    let transcript = TranscriptDocument::try_new(resource.clone(), cues).map_err(|source| {
        PodcastFixtureError::Domain {
            cue_index: domain_cue_index(&source),
            source,
        }
    })?;
    let identity = episode_identity(&source_url, Some(&publisher_guid), &enclosure_url);
    let feed = PodcastFeed {
        source_url: source_url.to_string(),
        title: feed_title,
        language: Some(feed_language),
        description: Some(feed_description),
        artwork_url: Some(feed_artwork_url),
    };
    let episode = PodcastEpisode {
        identity,
        publisher_guid: Some(publisher_guid),
        title: episode_title,
        published_at: Some(published_at),
        language: Some(episode_language),
        enclosure_url,
        enclosure_mime_type,
        duration_ms: Some(duration_ms),
        artwork_url: Some(episode_artwork_url),
        transcript_resources: vec![resource],
    };

    Ok(PodcastFixture {
        feed,
        episode,
        transcript,
    })
}

fn canonical_source_url(value: &str) -> Result<Url, PodcastFixtureError> {
    let mut url = Url::parse(value.trim()).map_err(|_| PodcastFixtureError::InvalidSourceUrl)?;
    if !valid_http_url(&url) {
        return Err(PodcastFixtureError::InvalidSourceUrl);
    }
    url.set_fragment(None);
    Ok(url)
}

fn resource_url(
    base: &Url,
    value: &str,
    field: &'static str,
    allow_fragment: bool,
) -> Result<String, PodcastFixtureError> {
    if value.trim().is_empty() {
        return Err(PodcastFixtureError::InvalidResolvedUrl { field });
    }
    let mut url = base
        .join(value.trim())
        .map_err(|_| PodcastFixtureError::InvalidResolvedUrl { field })?;
    if !valid_http_url(&url) {
        return Err(PodcastFixtureError::InvalidResolvedUrl { field });
    }
    if !allow_fragment {
        url.set_fragment(None);
    }
    Ok(url.to_string())
}

fn valid_http_url(url: &Url) -> bool {
    matches!(url.scheme(), "http" | "https")
        && url.host_str().is_some()
        && url.username().is_empty()
        && url.password().is_none()
}

fn metadata(value: String, field: &'static str) -> Result<String, PodcastFixtureError> {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        Err(PodcastFixtureError::InvalidMetadata { field })
    } else {
        Ok(normalized)
    }
}

fn cue_metadata(
    value: String,
    cue_index: usize,
    field: &'static str,
) -> Result<String, PodcastFixtureError> {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        Err(PodcastFixtureError::InvalidCue { cue_index, field })
    } else {
        Ok(normalized)
    }
}

fn normalized_mime(value: &str) -> Option<String> {
    let essence = value.split(';').next()?.trim().to_ascii_lowercase();
    let (top_level, subtype) = essence.split_once('/')?;
    if top_level.is_empty()
        || subtype.is_empty()
        || subtype.contains('/')
        || !top_level.chars().all(is_mime_token_character)
        || !subtype.chars().all(is_mime_token_character)
    {
        return None;
    }
    Some(essence)
}

fn is_mime_token_character(character: char) -> bool {
    character.is_ascii_alphanumeric()
        || matches!(
            character,
            '!' | '#' | '$' | '&' | '^' | '_' | '.' | '+' | '-'
        )
}

fn cue_number(
    value: &serde_json::Number,
    cue_index: usize,
    field: &'static str,
) -> Result<u64, PodcastFixtureError> {
    value
        .as_u64()
        .ok_or(PodcastFixtureError::InvalidCue { cue_index, field })
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
