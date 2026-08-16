//! Terminal reader and review surface for Ensub.

#![forbid(unsafe_code)]

mod app;
mod document;
mod layout;
mod runtime;
mod view;

pub use app::{
    update, AppKey, CaptureFeedback, Effect, InputEvent, InputKind, Message, Mode, Model, Notice,
    WordDetails,
};
pub use document::{
    Block, BlockKind, Document, DocumentFormat, DocumentToken, InlineStyle, StyledRange,
};
pub use layout::{DocumentLayout, TokenPlacement, VisualLine};
pub use runtime::{execute_effect, run, TerminalControl, TerminalGuard, TuiConfig, TuiError};
pub use view::render;
