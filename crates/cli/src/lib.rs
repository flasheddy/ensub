#![forbid(unsafe_code)]

mod app;
mod args;
mod paths;
mod prompt;

pub use app::execute;
pub use args::{AddArgs, Cli, Command, ParseArgs, ReviewArgs, TuiArgs};
pub use paths::{lexicon_cache_dir, resolve_database_path};
pub use prompt::{Prompt, ReviewResponse, TerminalPrompt};
