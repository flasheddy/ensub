//! Native SQLite adapters for Ensub user data and the bundled lexicon.

#![forbid(unsafe_code)]

mod error;
mod lexicon;
mod paths;
mod storage;

pub use error::SqliteError;
pub use lexicon::{BundledLexicon, SqliteLexicon};
pub use paths::{lexicon_cache_dir, resolve_database_path};
pub use storage::SqliteStorage;
