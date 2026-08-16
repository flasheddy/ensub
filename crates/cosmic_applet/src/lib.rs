//! Pure state and native host support for the Ensub COSMIC panel applet.

#![forbid(unsafe_code)]

mod native;
mod reducer;

pub use native::{run, AppletFlags};
pub use reducer::{badge_text, update, Effect, Message, Model, ReviewPhase};
