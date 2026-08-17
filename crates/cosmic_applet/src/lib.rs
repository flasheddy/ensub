//! Pure state and native host support for the Ensub COSMIC panel applet.

#![forbid(unsafe_code)]

mod native;
mod reducer;
mod theme;

pub use native::{run, run_with_theme, AppletFlags};
pub use reducer::{badge_text, update, Effect, Message, Model, ReviewPhase};
pub use theme::to_cosmic_theme;
