//! Pure state and native host support for the Ensub COSMIC applications.

#![forbid(unsafe_code)]

mod native;
mod reader;
mod reducer;

pub use native::{run, GuiFlags};
pub use reader::{
    build_block_runs, reader_badge, reader_uses_split_layout, update_reader, GlobalShortcut,
    KeyEventKind, ReaderBadge, ReaderEffect, ReaderKey, ReaderMessage, ReaderModel, ReaderRun,
    ReaderShortcut, ReaderWordDetails, READER_SPLIT_MIN_WIDTH,
};
pub use reducer::{
    update, update_hud, DashboardData, Effect, HudEffect, HudMessage, HudModel, Message, Model,
    Page, ReviewModel, ReviewPhase,
};
