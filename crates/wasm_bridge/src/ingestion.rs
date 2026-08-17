use core_engine::{
    EpisodeIdentity, PodcastEpisode, PodcastFeed, TranscriptDocument, TranscriptFormat,
    TranscriptResource,
};
use language_engine::{
    parse_podcast_feed, parse_transcript, PodcastFeedIssue, PodcastFeedIssueDisposition,
    PodcastFeedParseError, TranscriptParseError,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::text::utf16_offset;

#[derive(Debug, Error)]
pub enum IngestionError {
    #[error(transparent)]
    Feed(#[from] PodcastFeedParseError),
    #[error(transparent)]
    Transcript(#[from] TranscriptParseError),
    #[error("parser produced invalid UTF-8 boundary {0}")]
    InvalidTextBoundary(usize),
}

impl IngestionError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Feed(error) => error.code(),
            Self::Transcript(error) => error.code(),
            Self::InvalidTextBoundary(_) => "transcript_invalid_text_boundary",
        }
    }

    pub fn line(&self) -> Option<usize> {
        match self {
            Self::Transcript(error) => error.line(),
            Self::Feed(_) | Self::InvalidTextBoundary(_) => None,
        }
    }

    pub fn cue_index(&self) -> Option<usize> {
        match self {
            Self::Transcript(error) => error.cue_index(),
            Self::Feed(_) | Self::InvalidTextBoundary(_) => None,
        }
    }

    pub fn byte_offset(&self) -> Option<u64> {
        match self {
            Self::Feed(PodcastFeedParseError::InvalidEncoding { byte_offset }) => {
                u64::try_from(*byte_offset).ok()
            }
            Self::Feed(PodcastFeedParseError::ForbiddenDoctype { byte_offset })
            | Self::Feed(PodcastFeedParseError::MalformedXml { byte_offset, .. }) => {
                Some(*byte_offset)
            }
            Self::InvalidTextBoundary(byte_offset) => u64::try_from(*byte_offset).ok(),
            Self::Feed(_) | Self::Transcript(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PodcastFeedParseOutputDto {
    pub feed: PodcastFeedDto,
    pub episodes: Vec<PodcastEpisodeDto>,
    pub issues: Vec<PodcastFeedIssueDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PodcastFeedDto {
    pub source_url: String,
    pub title: String,
    pub language: Option<String>,
    pub description: Option<String>,
    pub artwork_url: Option<String>,
}

impl From<PodcastFeed> for PodcastFeedDto {
    fn from(feed: PodcastFeed) -> Self {
        Self {
            source_url: feed.source_url,
            title: feed.title,
            language: feed.language,
            description: feed.description,
            artwork_url: feed.artwork_url,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeIdentityDto {
    pub internal_id: String,
    pub feed_url: String,
    pub publisher_guid_aliases: Vec<String>,
    pub enclosure_url_aliases: Vec<String>,
}

impl From<EpisodeIdentity> for EpisodeIdentityDto {
    fn from(identity: EpisodeIdentity) -> Self {
        Self {
            internal_id: identity.internal_id,
            feed_url: identity.feed_url,
            publisher_guid_aliases: identity.publisher_guid_aliases,
            enclosure_url_aliases: identity.enclosure_url_aliases,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PodcastEpisodeDto {
    pub identity: EpisodeIdentityDto,
    pub publisher_guid: Option<String>,
    pub title: String,
    pub published_at_ms: Option<i64>,
    pub language: Option<String>,
    pub enclosure_url: String,
    pub enclosure_mime_type: String,
    pub duration_ms: Option<u64>,
    pub artwork_url: Option<String>,
    pub transcript_resources: Vec<TranscriptResourceDto>,
}

impl From<PodcastEpisode> for PodcastEpisodeDto {
    fn from(episode: PodcastEpisode) -> Self {
        Self {
            identity: episode.identity.into(),
            publisher_guid: episode.publisher_guid,
            title: episode.title,
            published_at_ms: episode
                .published_at
                .map(|published_at| published_at.timestamp_millis()),
            language: episode.language,
            enclosure_url: episode.enclosure_url,
            enclosure_mime_type: episode.enclosure_mime_type,
            duration_ms: episode.duration_ms,
            artwork_url: episode.artwork_url,
            transcript_resources: episode
                .transcript_resources
                .into_iter()
                .map(TranscriptResourceDto::from)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptResourceDto {
    pub url: String,
    pub mime_type: String,
    pub format: Option<TranscriptFormat>,
    pub language: Option<String>,
    pub relation: Option<String>,
}

impl From<TranscriptResource> for TranscriptResourceDto {
    fn from(resource: TranscriptResource) -> Self {
        Self {
            url: resource.url,
            mime_type: resource.mime_type,
            format: resource.format,
            language: resource.language,
            relation: resource.relation,
        }
    }
}

impl From<TranscriptResourceDto> for TranscriptResource {
    fn from(resource: TranscriptResourceDto) -> Self {
        Self {
            url: resource.url,
            mime_type: resource.mime_type,
            format: resource.format,
            language: resource.language,
            relation: resource.relation,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PodcastFeedIssueDto {
    pub code: String,
    pub disposition: PodcastFeedIssueDisposition,
    pub entry_index: Option<usize>,
    pub transcript_index: Option<usize>,
    pub first_entry_index: Option<usize>,
}

impl From<PodcastFeedIssue> for PodcastFeedIssueDto {
    fn from(issue: PodcastFeedIssue) -> Self {
        let disposition = issue.disposition();
        let code = issue.code().to_string();
        let (entry_index, transcript_index, first_entry_index) = match issue {
            PodcastFeedIssue::InvalidFeedArtworkUrl => (None, None, None),
            PodcastFeedIssue::MissingEpisodeTitle { entry_index }
            | PodcastFeedIssue::MissingAudioEnclosure { entry_index }
            | PodcastFeedIssue::MissingEnclosureMimeType { entry_index }
            | PodcastFeedIssue::InvalidEnclosureUrl { entry_index }
            | PodcastFeedIssue::InvalidPublicationDate { entry_index }
            | PodcastFeedIssue::InvalidDuration { entry_index }
            | PodcastFeedIssue::InvalidEpisodeArtworkUrl { entry_index } => {
                (Some(entry_index), None, None)
            }
            PodcastFeedIssue::MissingTranscriptUrl {
                entry_index,
                transcript_index,
            }
            | PodcastFeedIssue::MissingTranscriptMimeType {
                entry_index,
                transcript_index,
            }
            | PodcastFeedIssue::InvalidTranscriptUrl {
                entry_index,
                transcript_index,
            } => (Some(entry_index), Some(transcript_index), None),
            PodcastFeedIssue::DuplicateEpisodeIdentity {
                entry_index,
                first_entry_index,
            } => (Some(entry_index), None, Some(first_entry_index)),
        };
        Self {
            code,
            disposition,
            entry_index,
            transcript_index,
            first_entry_index,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptDocumentDto {
    pub resource: TranscriptResourceDto,
    pub cues: Vec<TranscriptCueDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptCueDto {
    pub id: String,
    pub source_order: u32,
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
    pub tokens: Vec<TranscriptTokenDto>,
    pub source_format: TranscriptFormat,
    pub source_cue_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptTokenDto {
    pub surface: String,
    pub start_utf16: usize,
    pub end_utf16: usize,
}

pub fn parse_podcast_feed_dto(
    source_url: &str,
    xml: &[u8],
) -> Result<PodcastFeedParseOutputDto, IngestionError> {
    let report = parse_podcast_feed(source_url, xml)?;
    Ok(PodcastFeedParseOutputDto {
        feed: report.feed.into(),
        episodes: report
            .episodes
            .into_iter()
            .map(PodcastEpisodeDto::from)
            .collect(),
        issues: report
            .issues
            .into_iter()
            .map(PodcastFeedIssueDto::from)
            .collect(),
    })
}

pub fn parse_transcript_dto(
    resource: TranscriptResourceDto,
    source: &str,
) -> Result<TranscriptDocumentDto, IngestionError> {
    let document = parse_transcript(resource.into(), source)?;
    transcript_document_dto(&document)
}

fn transcript_document_dto(
    document: &TranscriptDocument,
) -> Result<TranscriptDocumentDto, IngestionError> {
    let mut cues = Vec::with_capacity(document.cues().len());
    for cue in document.cues() {
        let mut tokens = Vec::with_capacity(cue.tokens().len());
        for token in cue.tokens() {
            let start_byte = usize::try_from(token.start_byte())
                .map_err(|_| IngestionError::InvalidTextBoundary(usize::MAX))?;
            let end_byte = usize::try_from(token.end_byte())
                .map_err(|_| IngestionError::InvalidTextBoundary(usize::MAX))?;
            let start_utf16 = utf16_offset(cue.text(), start_byte)
                .ok_or(IngestionError::InvalidTextBoundary(start_byte))?;
            let end_utf16 = utf16_offset(cue.text(), end_byte)
                .ok_or(IngestionError::InvalidTextBoundary(end_byte))?;
            tokens.push(TranscriptTokenDto {
                surface: token.surface().to_string(),
                start_utf16,
                end_utf16,
            });
        }
        cues.push(TranscriptCueDto {
            id: cue.id().to_string(),
            source_order: cue.source_order(),
            start_ms: cue.start_ms(),
            end_ms: cue.end_ms(),
            text: cue.text().to_string(),
            tokens,
            source_format: cue.source_format(),
            source_cue_id: cue.source_cue_id().map(str::to_string),
        });
    }
    Ok(TranscriptDocumentDto {
        resource: document.resource().clone().into(),
        cues,
    })
}
