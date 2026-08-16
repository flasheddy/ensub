use js_sys::{Error, Reflect, Uint8Array};
use serde::de::DeserializeOwned;
use serde::Serialize;
use wasm_bindgen::prelude::*;

use crate::{
    CaptureParsedInput, DueReviewsInput, LocalStorageBackend, ParseInput, ReviewInput, Sandbox,
    SandboxError, SnapshotAccess, SnapshotError, StatsInput,
};

#[wasm_bindgen]
pub struct EnsubSandbox {
    inner: Sandbox<LocalStorageBackend>,
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

fn from_js<T: DeserializeOwned>(value: JsValue) -> Result<T, JsValue> {
    serde_wasm_bindgen::from_value(value)
        .map_err(|error| js_error("invalid_argument", &error.to_string()))
}

fn to_js<T: Serialize>(value: &T) -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(value)
        .map_err(|error| js_error("serialization_failed", &error.to_string()))
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
