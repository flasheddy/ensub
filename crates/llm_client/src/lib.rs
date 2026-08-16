//! Provider-neutral contextual word disambiguation over OpenAI-compatible APIs.

#![forbid(unsafe_code)]

mod disambiguation;

pub use disambiguation::{resolve_contextual_meaning, DisambiguationRequest, DisambiguationResult};
