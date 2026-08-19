//! UniFFI facade for native Ensub clients.

#![forbid(unsafe_code)]

use core_engine::TranscriptDocument;
use language_engine::{parse_podcast_fixture, PodcastFixtureError};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct EpisodeDto {
    pub feed_title: String,
    pub episode_title: String,
    pub duration_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TranscriptCueDto {
    pub index: i64,
    pub id: String,
    pub source_cue_id: Option<String>,
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TranscriptSyncDto {
    pub active_cue_indices: Vec<i64>,
    pub anchor_cue_index: Option<i64>,
    pub preceding_cue_index: Option<i64>,
}

#[derive(Debug, Error, uniffi::Error)]
pub enum BindingError {
    #[error("podcast fixture is invalid ({code}): {detail}")]
    InvalidFixture { code: String, detail: String },
    #[error("playback position must not be negative")]
    InvalidPlaybackPosition,
    #[error("a portable numeric value does not fit the native client contract")]
    NumericOverflow,
}

impl From<PodcastFixtureError> for BindingError {
    fn from(error: PodcastFixtureError) -> Self {
        Self::InvalidFixture {
            code: error.code().to_string(),
            detail: error.to_string(),
        }
    }
}

#[derive(uniffi::Object)]
pub struct TranscriptSession {
    episode: EpisodeDto,
    cues: Vec<TranscriptCueDto>,
    document: TranscriptDocument,
}

#[uniffi::export]
impl TranscriptSession {
    #[uniffi::constructor]
    pub fn from_fixture(source_url: String, fixture_bytes: Vec<u8>) -> Result<Self, BindingError> {
        let fixture = parse_podcast_fixture(&source_url, &fixture_bytes)?;
        let duration_ms = signed(
            fixture
                .episode
                .duration_ms
                .ok_or(BindingError::NumericOverflow)?,
        )?;
        let episode = EpisodeDto {
            feed_title: fixture.feed.title,
            episode_title: fixture.episode.title,
            duration_ms,
        };
        let cues = fixture
            .transcript
            .cues()
            .iter()
            .enumerate()
            .map(|(index, cue)| {
                Ok(TranscriptCueDto {
                    index: i64::try_from(index).map_err(|_| BindingError::NumericOverflow)?,
                    id: cue.id().to_string(),
                    source_cue_id: cue.source_cue_id().map(str::to_string),
                    start_ms: signed(cue.start_ms())?,
                    end_ms: signed(cue.end_ms())?,
                    text: cue.text().to_string(),
                })
            })
            .collect::<Result<Vec<_>, BindingError>>()?;
        Ok(Self {
            episode,
            cues,
            document: fixture.transcript,
        })
    }

    pub fn episode(&self) -> EpisodeDto {
        self.episode.clone()
    }

    pub fn cues(&self) -> Vec<TranscriptCueDto> {
        self.cues.clone()
    }

    pub fn sync_at(&self, position_ms: i64) -> Result<TranscriptSyncDto, BindingError> {
        let position_ms =
            u64::try_from(position_ms).map_err(|_| BindingError::InvalidPlaybackPosition)?;
        let active_cue_indices = self
            .document
            .active_cue_indices(position_ms)
            .into_iter()
            .map(|index| Ok(i64::from(index)))
            .collect::<Result<Vec<_>, BindingError>>()?;
        let anchor_cue_index = active_cue_indices.first().copied();
        let preceding_cue_index = if active_cue_indices.is_empty() {
            self.document
                .cues()
                .iter()
                .rposition(|cue| cue.end_ms() <= position_ms)
                .map(|index| i64::try_from(index).map_err(|_| BindingError::NumericOverflow))
                .transpose()?
        } else {
            None
        };
        Ok(TranscriptSyncDto {
            active_cue_indices,
            anchor_cue_index,
            preceding_cue_index,
        })
    }
}

fn signed(value: u64) -> Result<i64, BindingError> {
    i64::try_from(value).map_err(|_| BindingError::NumericOverflow)
}

uniffi::setup_scaffolding!();
