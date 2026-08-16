use std::path::PathBuf;

use crate::SqliteError;

/// Resolves the shared native database path, preserving an explicit override.
pub fn resolve_database_path(override_path: Option<PathBuf>) -> Result<PathBuf, SqliteError> {
    if let Some(path) = override_path {
        return Ok(path);
    }

    let data_dir =
        dirs::data_local_dir().ok_or(SqliteError::PlatformDirectoryUnavailable("data"))?;
    Ok(data_dir.join("ensub").join("ensub.sqlite3"))
}

/// Resolves the shared cache directory for the extracted bundled lexicon.
pub fn lexicon_cache_dir() -> Result<PathBuf, SqliteError> {
    let cache_dir = dirs::cache_dir().ok_or(SqliteError::PlatformDirectoryUnavailable("cache"))?;
    Ok(cache_dir.join("ensub").join("lexicon"))
}
