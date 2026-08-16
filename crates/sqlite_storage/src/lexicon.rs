use std::fs::{File, OpenOptions};
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use fs2::FileExt;
use language_engine::{Definition, Lexicon, LexiconEntry};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use sha2::{Digest, Sha256};

use crate::SqliteError;

const LEXICON_SCHEMA_VERSION: &str = "1";
const EMBEDDED_ASSET: &[u8] = include_bytes!("../assets/lexicon-v1.sqlite3.zst");
const COMPRESSED_SHA256: &str = "2065aad3c76357341f2c4e66cc1125f6e5d8afd8f621112bad50bd16c38b3c1f";
const DATABASE_SHA256: &str = "cad6ada703bee58c93c1d84fbd285a711f63cbd9666cde1567b136c9bbefd51b";
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

pub struct SqliteLexicon {
    connection: Connection,
}

pub struct BundledLexicon {
    inner: SqliteLexicon,
    installed_path: PathBuf,
}

impl SqliteLexicon {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SqliteError> {
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        connection.pragma_update(None, "query_only", "ON")?;
        let version = connection
            .query_row(
                "SELECT value FROM metadata WHERE key = 'schema_version'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .unwrap_or_default();
        if version != LEXICON_SCHEMA_VERSION {
            return Err(SqliteError::UnsupportedLexiconSchema(version));
        }
        Ok(Self { connection })
    }
}

impl BundledLexicon {
    pub fn open(cache_directory: impl AsRef<Path>) -> Result<Self, SqliteError> {
        let cache_directory = cache_directory.as_ref();
        std::fs::create_dir_all(cache_directory)
            .map_err(|error| SqliteError::io(cache_directory, error))?;
        let lock_path = cache_directory.join("lexicon-v1.lock");
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&lock_path)
            .map_err(|error| SqliteError::io(&lock_path, error))?;
        lock.lock_exclusive()
            .map_err(|error| SqliteError::io(&lock_path, error))?;

        let installed_path =
            cache_directory.join(format!("lexicon-v1-{}.sqlite3", &DATABASE_SHA256[..12]));
        ensure_installed(&installed_path)?;
        fs2::FileExt::unlock(&lock).map_err(|error| SqliteError::io(&lock_path, error))?;

        let inner = SqliteLexicon::open(&installed_path)?;
        Ok(Self {
            inner,
            installed_path,
        })
    }

    pub fn installed_path(&self) -> &Path {
        &self.installed_path
    }
}

impl Lexicon for BundledLexicon {
    type Error = SqliteError;

    fn lookup(&self, surface: &str) -> Result<Option<LexiconEntry>, Self::Error> {
        self.inner.lookup(surface)
    }
}

impl Lexicon for SqliteLexicon {
    type Error = SqliteError;

    fn lookup(&self, surface: &str) -> Result<Option<LexiconEntry>, Self::Error> {
        let surface = surface.trim().to_lowercase();
        if surface.is_empty() {
            return Ok(None);
        }

        let lexeme = self
            .connection
            .query_row(
                "SELECT l.id, l.lemma, l.phonetic
                 FROM forms f
                 JOIN lexemes l ON l.id = f.lexeme_id
                 WHERE f.surface = ?1 COLLATE NOCASE
                 ORDER BY f.priority ASC, l.lemma ASC, l.id ASC
                 LIMIT 1",
                [surface],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?;
        let Some((lexeme_id, lemma, phonetic)) = lexeme else {
            return Ok(None);
        };
        if lemma.trim().is_empty() || phonetic.trim().is_empty() {
            return Ok(None);
        }

        let mut statement = self.connection.prepare(
            "SELECT part_of_speech, definition
             FROM senses
             WHERE lexeme_id = ?1
               AND definition != ''
             ORDER BY rank ASC
             LIMIT 3",
        )?;
        let definitions = statement
            .query_map([lexeme_id], |row| {
                Ok(Definition {
                    part_of_speech: row.get(0)?,
                    text: row.get(1)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        if definitions.is_empty() {
            return Ok(None);
        }

        Ok(Some(LexiconEntry {
            lemma,
            phonetic,
            definitions,
        }))
    }
}

fn ensure_installed(installed_path: &Path) -> Result<(), SqliteError> {
    if installed_path.exists() {
        let digest = sha256_file(installed_path)?;
        if digest == DATABASE_SHA256 {
            return Ok(());
        }
        std::fs::remove_file(installed_path)
            .map_err(|error| SqliteError::io(installed_path, error))?;
    }

    let compressed_digest = sha256_bytes(EMBEDDED_ASSET);
    if compressed_digest != COMPRESSED_SHA256 {
        return Err(SqliteError::LexiconChecksum {
            subject: "embedded compressed asset".to_string(),
            expected: COMPRESSED_SHA256.to_string(),
            actual: compressed_digest,
        });
    }

    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let file_name = installed_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| SqliteError::InvalidLexiconInstallPath(installed_path.to_path_buf()))?;
    let temporary_path =
        installed_path.with_file_name(format!(".{file_name}.tmp-{}-{counter}", std::process::id()));
    let mut temporary = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary_path)
        .map_err(|error| SqliteError::io(&temporary_path, error))?;
    let extraction = zstd::stream::copy_decode(Cursor::new(EMBEDDED_ASSET), &mut temporary)
        .map_err(|error| SqliteError::io(&temporary_path, error));
    if let Err(error) = extraction {
        let _ = std::fs::remove_file(&temporary_path);
        return Err(error);
    }
    temporary
        .flush()
        .map_err(|error| SqliteError::io(&temporary_path, error))?;
    temporary
        .sync_all()
        .map_err(|error| SqliteError::io(&temporary_path, error))?;
    drop(temporary);

    let actual = sha256_file(&temporary_path)?;
    if actual != DATABASE_SHA256 {
        let _ = std::fs::remove_file(&temporary_path);
        return Err(SqliteError::LexiconChecksum {
            subject: temporary_path.display().to_string(),
            expected: DATABASE_SHA256.to_string(),
            actual,
        });
    }
    let mut permissions = std::fs::metadata(&temporary_path)
        .map_err(|error| SqliteError::io(&temporary_path, error))?
        .permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&temporary_path, permissions)
        .map_err(|error| SqliteError::io(&temporary_path, error))?;
    std::fs::rename(&temporary_path, installed_path)
        .map_err(|error| SqliteError::io(installed_path, error))?;
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String, SqliteError> {
    let mut file = File::open(path).map_err(|error| SqliteError::io(path, error))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| SqliteError::io(path, error))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(to_hex(hasher.finalize().as_slice()))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    to_hex(Sha256::digest(bytes).as_slice())
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
