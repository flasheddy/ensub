use js_sys::{Error, Number, Reflect, Uint8Array};
use serde::de::DeserializeOwned;
use serde::Serialize;
use wasm_bindgen::prelude::*;

use crate::{
    parse_podcast_feed_dto, parse_transcript_dto, CaptureParsedInput, CapturePodcastInput,
    DueCountInputDto, DueReviewsInput, DueReviewsInputDto, IngestionError, LocalStorageBackend,
    ParseInput, PlayerLearning, PlayerLearningError, PlayerWorkspace, PlayerWorkspaceError,
    PrepareDisambiguationInputDto, PreparePodcastCaptureInput, RateReviewInputDto,
    RevealReviewInputDto, ReviewInput, Sandbox, SandboxError, SnapshotAccess, SnapshotError,
    StatsInput, TranscriptResourceDto, ValidateDisambiguationResponseInputDto,
    MAX_PLAYER_FIXTURE_BYTES,
};

#[wasm_bindgen]
pub struct EnsubSandbox {
    inner: Sandbox<LocalStorageBackend>,
}

#[wasm_bindgen]
pub struct EnsubPlayerLearning {
    inner: PlayerLearning<LocalStorageBackend>,
}

#[wasm_bindgen]
impl EnsubPlayerLearning {
    #[wasm_bindgen(constructor)]
    pub fn new(
        lexicon_bytes: Uint8Array,
        storage_key: String,
        read_only: bool,
    ) -> Result<EnsubPlayerLearning, JsValue> {
        let access = if read_only {
            SnapshotAccess::ReadOnly
        } else {
            SnapshotAccess::ReadWrite
        };
        let backend = LocalStorageBackend::open().map_err(|error| {
            js_error(
                "storage_unavailable",
                &format!("browser storage failed: {error}"),
            )
        })?;
        let inner = PlayerLearning::open(backend, storage_key, access, &lexicon_bytes.to_vec())
            .map_err(learning_error)?;
        Ok(Self { inner })
    }

    #[wasm_bindgen(js_name = lookupToken)]
    pub fn lookup_token(&self, surface: String) -> Result<JsValue, JsValue> {
        to_js(&self.inner.lookup_token(&surface))
    }

    #[wasm_bindgen(js_name = capturePodcast)]
    pub fn capture_podcast(&mut self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: CapturePodcastInput = from_js(input)?;
        to_js(&self.inner.capture_podcast(input).map_err(learning_error)?)
    }

    #[wasm_bindgen(js_name = dueCount)]
    pub fn due_count(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: DueCountInputDto = from_js(input)?;
        to_js(&self.inner.due_count(input).map_err(learning_error)?)
    }

    #[wasm_bindgen(js_name = dueReviews)]
    pub fn due_reviews(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: DueReviewsInputDto = from_js(input)?;
        to_js(&self.inner.due_reviews(input).map_err(learning_error)?)
    }

    #[wasm_bindgen(js_name = revealReview)]
    pub fn reveal_review(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: RevealReviewInputDto = from_js(input)?;
        to_js(&self.inner.reveal_review(input).map_err(learning_error)?)
    }

    pub fn review(&mut self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: RateReviewInputDto = from_js(input)?;
        to_js(&self.inner.review(input).map_err(learning_error)?)
    }

    pub fn reset(&mut self) -> Result<(), JsValue> {
        self.inner.reset().map_err(learning_error)
    }

    #[wasm_bindgen(js_name = prepareDisambiguation)]
    pub fn prepare_disambiguation(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: PrepareDisambiguationInputDto = from_js(input)?;
        to_js(
            &self
                .inner
                .prepare_disambiguation(input)
                .map_err(learning_error)?,
        )
    }

    #[wasm_bindgen(js_name = validateDisambiguationResponse)]
    pub fn validate_disambiguation_response(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: ValidateDisambiguationResponseInputDto = from_js(input)?;
        to_js(
            &self
                .inner
                .validate_disambiguation_response(input)
                .map_err(learning_error)?,
        )
    }
}

#[wasm_bindgen]
impl EnsubSandbox {
    #[wasm_bindgen(constructor)]
    pub fn new(
        lexicon_bytes: Uint8Array,
        storage_key: String,
        read_only: bool,
    ) -> Result<EnsubSandbox, JsValue> {
        let access = if read_only {
            SnapshotAccess::ReadOnly
        } else {
            SnapshotAccess::ReadWrite
        };
        let backend = LocalStorageBackend::open().map_err(|error| {
            js_error(
                "storage_unavailable",
                &format!("browser storage failed: {error}"),
            )
        })?;
        let inner = Sandbox::open(backend, storage_key, access, &lexicon_bytes.to_vec())
            .map_err(sandbox_error)?;
        Ok(Self { inner })
    }

    pub fn parse(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: ParseInput = from_js(input)?;
        to_js(&self.inner.parse(&input).map_err(sandbox_error)?)
    }

    #[wasm_bindgen(js_name = captureParsed)]
    pub fn capture_parsed(&mut self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: CaptureParsedInput = from_js(input)?;
        to_js(&self.inner.capture_parsed(&input).map_err(sandbox_error)?)
    }

    #[wasm_bindgen(js_name = dueReviews)]
    pub fn due_reviews(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: DueReviewsInput = from_js(input)?;
        to_js(&self.inner.due_reviews(&input).map_err(sandbox_error)?)
    }

    pub fn review(&mut self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: ReviewInput = from_js(input)?;
        to_js(&self.inner.review(&input).map_err(sandbox_error)?)
    }

    pub fn stats(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: StatsInput = from_js(input)?;
        to_js(&self.inner.stats(&input).map_err(sandbox_error)?)
    }

    pub fn reset(&mut self) -> Result<(), JsValue> {
        self.inner.reset().map_err(sandbox_error)
    }
}

#[wasm_bindgen]
pub struct EnsubPlayerWorkspace {
    inner: PlayerWorkspace,
}

#[wasm_bindgen]
impl EnsubPlayerWorkspace {
    #[wasm_bindgen(constructor)]
    pub fn new(snapshot_bytes: Uint8Array) -> Result<EnsubPlayerWorkspace, JsValue> {
        let inner = PlayerWorkspace::open(&snapshot_bytes.to_vec()).map_err(player_error)?;
        Ok(Self { inner })
    }

    pub fn view(&self) -> Result<JsValue, JsValue> {
        to_js(&self.inner.view())
    }

    #[wasm_bindgen(js_name = importFeed)]
    pub fn import_feed(
        &mut self,
        source_url: String,
        xml_bytes: Uint8Array,
        fetched_at_ms: f64,
    ) -> Result<JsValue, JsValue> {
        let fetched_at_ms = safe_milliseconds_i64(fetched_at_ms)?;
        let view = self
            .inner
            .import_feed(&source_url, &xml_bytes.to_vec(), fetched_at_ms)
            .map_err(player_error)?;
        to_js(&view)
    }

    #[wasm_bindgen(js_name = importDemoFixture)]
    pub fn import_demo_fixture(
        &mut self,
        source_url: String,
        fixture_bytes: Uint8Array,
        fetched_at_ms: f64,
    ) -> Result<JsValue, JsValue> {
        if fixture_bytes.length() as usize > MAX_PLAYER_FIXTURE_BYTES {
            return Err(player_error(PlayerWorkspaceError::FixtureTooLarge));
        }
        let fetched_at_ms = safe_milliseconds_u64(fetched_at_ms)?;
        let fetched_at_ms = i64::try_from(fetched_at_ms).map_err(|_| {
            js_error(
                "invalid_argument",
                "milliseconds must fit the player timestamp range",
            )
        })?;
        let fixture_bytes = fixture_bytes.to_vec();
        let mut candidate = self.inner.clone();
        let opened = candidate
            .import_demo_fixture(&source_url, &fixture_bytes, fetched_at_ms)
            .map_err(player_error)?;
        let value = to_js(&opened)?;
        self.inner = candidate;
        Ok(value)
    }

    #[wasm_bindgen(js_name = selectEpisode)]
    pub fn select_episode(&mut self, episode_id: String) -> Result<JsValue, JsValue> {
        let opened = self
            .inner
            .select_episode(&episode_id)
            .map_err(player_error)?;
        to_js(&opened)
    }

    #[wasm_bindgen(js_name = selectTranscript)]
    pub fn select_transcript(
        &mut self,
        episode_id: String,
        transcript_url: String,
    ) -> Result<JsValue, JsValue> {
        let opened = self
            .inner
            .select_transcript(&episode_id, &transcript_url)
            .map_err(player_error)?;
        to_js(&opened)
    }

    #[wasm_bindgen(js_name = cacheTranscript)]
    pub fn cache_transcript(
        &mut self,
        episode_id: String,
        transcript_url: String,
        source: String,
        fetched_at_ms: f64,
    ) -> Result<JsValue, JsValue> {
        let fetched_at_ms = safe_milliseconds_i64(fetched_at_ms)?;
        let opened = self
            .inner
            .cache_transcript(&episode_id, &transcript_url, &source, fetched_at_ms)
            .map_err(player_error)?;
        to_js(&opened)
    }

    #[wasm_bindgen(js_name = syncAt)]
    pub fn sync_at(&self, playback_position_ms: f64) -> Result<JsValue, JsValue> {
        let playback_position_ms = safe_milliseconds_u64(playback_position_ms)?;
        let sync = self
            .inner
            .sync_at(playback_position_ms)
            .map_err(player_error)?;
        to_js(&sync)
    }

    #[wasm_bindgen(js_name = nextCueAt)]
    pub fn next_cue_at(&self, playback_position_ms: f64) -> Result<JsValue, JsValue> {
        let playback_position_ms = safe_milliseconds_u64(playback_position_ms)?;
        let cue = self
            .inner
            .next_cue_at(playback_position_ms)
            .map_err(player_error)?;
        optional_to_js(cue.as_ref())
    }

    #[wasm_bindgen(js_name = previousCueAt)]
    pub fn previous_cue_at(&self, playback_position_ms: f64) -> Result<JsValue, JsValue> {
        let playback_position_ms = safe_milliseconds_u64(playback_position_ms)?;
        let cue = self
            .inner
            .previous_cue_at(playback_position_ms)
            .map_err(player_error)?;
        optional_to_js(cue.as_ref())
    }

    #[wasm_bindgen(js_name = preparePodcastCapture)]
    pub fn prepare_podcast_capture(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: PreparePodcastCaptureInput = from_js(input)?;
        to_js(
            &self
                .inner
                .prepare_podcast_capture(&input)
                .map_err(player_error)?,
        )
    }

    pub fn snapshot(&self) -> Result<Uint8Array, JsValue> {
        let snapshot = self.inner.snapshot().map_err(player_error)?;
        Ok(Uint8Array::from(snapshot.as_slice()))
    }
}

#[wasm_bindgen(js_name = parsePodcastFeed)]
pub fn parse_podcast_feed(source_url: String, xml: Uint8Array) -> Result<JsValue, JsValue> {
    to_js(&parse_podcast_feed_dto(&source_url, &xml.to_vec()).map_err(ingestion_error)?)
}

#[wasm_bindgen(js_name = parseTranscript)]
pub fn parse_transcript(resource: JsValue, source: String) -> Result<JsValue, JsValue> {
    let resource: TranscriptResourceDto = from_js(resource)?;
    to_js(&parse_transcript_dto(resource, &source).map_err(ingestion_error)?)
}

fn from_js<T: DeserializeOwned>(value: JsValue) -> Result<T, JsValue> {
    serde_wasm_bindgen::from_value(value)
        .map_err(|error| js_error("invalid_argument", &error.to_string()))
}

fn to_js<T: Serialize>(value: &T) -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(value)
        .map_err(|error| js_error("serialization_failed", &error.to_string()))
}

fn optional_to_js<T: Serialize>(value: Option<&T>) -> Result<JsValue, JsValue> {
    match value {
        Some(value) => to_js(value),
        None => Ok(JsValue::NULL),
    }
}

fn sandbox_error(error: SandboxError) -> JsValue {
    let code = match &error {
        SandboxError::EmptyText
        | SandboxError::InvalidTimestamp(_)
        | SandboxError::InvalidTextBoundary(_)
        | SandboxError::UnknownCandidate(_)
        | SandboxError::DuplicateCandidate(_)
        | SandboxError::InvalidRating(_) => "invalid_argument",
        SandboxError::ReviewConflict => "review_conflict",
        SandboxError::Serialization(_) => "serialization_failed",
        SandboxError::Lexicon(_) => "lexicon_invalid",
        SandboxError::Storage(SnapshotError::ReadOnly) => "storage_read_only",
        SandboxError::Storage(SnapshotError::CorruptSnapshot(_)) => "storage_corrupt",
        SandboxError::Storage(SnapshotError::UnsupportedSchema { .. }) => "unsupported_schema",
        SandboxError::Storage(SnapshotError::Backend { .. }) => "storage_unavailable",
        SandboxError::Storage(_) => "storage_invalid",
        SandboxError::Core(_) => "scheduling_overflow",
    };
    js_error(code, &error.to_string())
}

fn learning_error(error: PlayerLearningError) -> JsValue {
    let code = match &error {
        PlayerLearningError::Lexicon(_) => "lexicon_invalid",
        PlayerLearningError::UnknownToken => "lookup_unknown",
        PlayerLearningError::AmbiguousToken => "lookup_ambiguous",
        PlayerLearningError::InvalidLemma
        | PlayerLearningError::InvalidTimestamp
        | PlayerLearningError::InvalidReviewLimit(_)
        | PlayerLearningError::InvalidRating(_)
        | PlayerLearningError::InvalidCapture(_) => "invalid_argument",
        PlayerLearningError::ReviewNotFound => "review_not_found",
        PlayerLearningError::ReviewConflict => "review_conflict",
        PlayerLearningError::ReviewToken(_) => "serialization_failed",
        PlayerLearningError::Disambiguation(error) => error.code(),
        PlayerLearningError::Storage(SnapshotError::ReadOnly) => "storage_read_only",
        PlayerLearningError::Storage(SnapshotError::CorruptSnapshot(_)) => "storage_corrupt",
        PlayerLearningError::Storage(SnapshotError::UnsupportedSchema { .. }) => {
            "unsupported_schema"
        }
        PlayerLearningError::Storage(SnapshotError::Backend { .. }) => "storage_unavailable",
        PlayerLearningError::Storage(_) => "storage_invalid",
        PlayerLearningError::Core(_) => "review_schedule_failed",
    };
    js_error(code, &error.to_string())
}

fn ingestion_error(error: IngestionError) -> JsValue {
    let value = js_error(error.code(), &error.to_string());
    set_optional_number(&value, "byteOffset", error.byte_offset());
    set_optional_number(
        &value,
        "line",
        error.line().and_then(|line| u64::try_from(line).ok()),
    );
    set_optional_number(
        &value,
        "cueIndex",
        error
            .cue_index()
            .and_then(|cue_index| u64::try_from(cue_index).ok()),
    );
    value
}

fn player_error(error: PlayerWorkspaceError) -> JsValue {
    match error {
        PlayerWorkspaceError::Ingestion(error) => ingestion_error(error),
        error => js_error(error.code(), &error.to_string()),
    }
}

fn safe_milliseconds_i64(value: f64) -> Result<i64, JsValue> {
    if !value.is_finite() || value < 0.0 || value.fract() != 0.0 || value > Number::MAX_SAFE_INTEGER
    {
        return Err(js_error(
            "invalid_argument",
            "milliseconds must be a finite, non-negative safe integer",
        ));
    }
    Ok(value as i64)
}

fn safe_milliseconds_u64(value: f64) -> Result<u64, JsValue> {
    safe_milliseconds_i64(value).map(|value| value as u64)
}

fn set_optional_number(error: &JsValue, property: &str, value: Option<u64>) {
    if let Some(value) = value {
        let _ = Reflect::set(
            error,
            &JsValue::from_str(property),
            &JsValue::from_f64(value as f64),
        );
    }
}

fn js_error(code: &str, message: &str) -> JsValue {
    let error = Error::new(message);
    error.set_name("EnsubError");
    let _ = Reflect::set(
        error.as_ref(),
        &JsValue::from_str("code"),
        &JsValue::from_str(code),
    );
    error.into()
}
