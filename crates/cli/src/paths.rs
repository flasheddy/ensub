use std::path::PathBuf;

use anyhow::Result;

pub fn resolve_database_path(override_path: Option<PathBuf>) -> Result<PathBuf> {
    ensub_sqlite::resolve_database_path(override_path).map_err(Into::into)
}

pub fn lexicon_cache_dir() -> Result<PathBuf> {
    ensub_sqlite::lexicon_cache_dir().map_err(Into::into)
}
