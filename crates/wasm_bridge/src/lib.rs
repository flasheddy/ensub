//! Browser bindings and persistence adapter for Ensub.

#![forbid(unsafe_code)]

mod sandbox;
mod storage;

#[cfg(target_arch = "wasm32")]
mod browser;

pub use sandbox::{
    CandidateDto, CaptureOutput, CaptureParsedInput, ContextDto, DueReviewsInput, DueReviewsOutput,
    IntervalDistributionDto, ParseInput, ParseOutput, ReviewCardDto, ReviewInput, ReviewOutput,
    Sandbox, SandboxError, StatsInput, StatsOutput,
};

pub use storage::{
    SnapshotAccess, SnapshotBackend, SnapshotError, SnapshotStorage, SNAPSHOT_SCHEMA_VERSION,
};

#[cfg(target_arch = "wasm32")]
pub use browser::EnsubSandbox;

#[cfg(target_arch = "wasm32")]
pub use storage::{LocalStorageBackend, LocalStorageBackendError};
