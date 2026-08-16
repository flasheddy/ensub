//! Reproducible Open English WordNet and CMUdict lexicon generation.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::{BufRead, Read, Write};
use std::path::{Path, PathBuf};

use flate2::{Compression, GzBuilder};
use language_engine::{
    lemma_candidates, BrowserLexiconAsset, BrowserLexiconError, BrowserLexiconForm, Definition,
    LexiconEntry, BROWSER_LEXICON_SCHEMA_VERSION,
};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use rusqlite::{params, Connection, TransactionBehavior};
use sha2::Digest;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildReport {
    pub lexemes: u64,
    pub forms: u64,
    pub senses: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserExportReport {
    pub lexemes: u64,
    pub forms: u64,
    pub uncompressed_bytes: u64,
    pub compressed_bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Error)]
pub enum BuildError {
    #[error("lexicon output already exists: {0}")]
    OutputExists(PathBuf),

    #[error("OEWN XML is invalid: {0}")]
    Xml(#[from] quick_xml::Error),

    #[error("OEWN XML text encoding is invalid: {0}")]
    Encoding(#[from] quick_xml::encoding::EncodingError),

    #[error("OEWN XML entity is invalid: {0}")]
    Escape(#[from] quick_xml::escape::EscapeError),

    #[error("CMUdict input could not be read: {0}")]
    CmudictRead(#[source] std::io::Error),

    #[error("SQLite generation failed: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("browser lexicon generation failed: {0}")]
    BrowserLexicon(#[from] BrowserLexiconError),

    #[error("lexicon contains too many {kind} to represent in SQLite")]
    IndexOverflow { kind: &'static str },

    #[error("filesystem operation failed for {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[derive(Debug)]
struct LexicalEntry {
    lemma: String,
    part_of_speech: String,
    synsets: Vec<String>,
}

#[derive(Default)]
struct PendingEntry {
    lemma: Option<String>,
    part_of_speech: Option<String>,
    synsets: Vec<String>,
}

#[derive(Default)]
struct LexemeData {
    senses: Vec<(String, String)>,
}

pub fn build_lexicon<O, C>(
    oewn_xml: O,
    cmudict: C,
    output: impl AsRef<Path>,
) -> Result<BuildReport, BuildError>
where
    O: BufRead,
    C: BufRead,
{
    let output = output.as_ref();
    if output.exists() {
        return Err(BuildError::OutputExists(output.to_path_buf()));
    }

    let (entries, definitions) = parse_oewn(oewn_xml)?;
    let pronunciations = parse_cmudict(cmudict)?;
    let mut lexemes = BTreeMap::<String, LexemeData>::new();

    for entry in entries {
        let data = lexemes.entry(entry.lemma).or_default();
        for synset in entry.synsets {
            if let Some(definition) = definitions.get(&synset) {
                let sense = (entry.part_of_speech.clone(), definition.clone());
                if !data.senses.contains(&sense) {
                    data.senses.push(sense);
                }
            }
        }
    }

    lexemes.retain(|lemma, data| {
        !data.senses.is_empty()
            && pronunciations
                .get(lemma)
                .is_some_and(|phonetic| !phonetic.is_empty())
    });

    let mut connection = Connection::open(output)?;
    connection.execute_batch(
        "PRAGMA journal_mode = OFF;
         PRAGMA synchronous = OFF;
         PRAGMA temp_store = MEMORY;
         CREATE TABLE metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL) STRICT;
         CREATE TABLE lexemes (
            id INTEGER PRIMARY KEY,
            lemma TEXT NOT NULL UNIQUE COLLATE NOCASE,
            phonetic TEXT NOT NULL
         ) STRICT;
         CREATE TABLE forms (
            surface TEXT NOT NULL COLLATE NOCASE,
            lexeme_id INTEGER NOT NULL REFERENCES lexemes(id),
            priority INTEGER NOT NULL,
            PRIMARY KEY(surface, lexeme_id)
         ) STRICT;
         CREATE TABLE senses (
            lexeme_id INTEGER NOT NULL REFERENCES lexemes(id),
            rank INTEGER NOT NULL,
            part_of_speech TEXT NOT NULL,
            definition TEXT NOT NULL,
            PRIMARY KEY(lexeme_id, rank)
         ) STRICT;",
    )?;

    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    transaction.execute("INSERT INTO metadata VALUES ('schema_version', '1')", [])?;
    transaction.execute(
        "INSERT INTO metadata VALUES ('definition_source', 'Open English WordNet 2025')",
        [],
    )?;
    transaction.execute(
        "INSERT INTO metadata VALUES ('pronunciation_source', 'CMUdict 0.7b via cmudict-fast 0.8.0')",
        [],
    )?;

    let mut lemma_ids = HashMap::with_capacity(lexemes.len());
    let mut sense_count = 0_u64;
    for (index, (lemma, data)) in lexemes.iter().enumerate() {
        let id =
            i64::try_from(index + 1).map_err(|_| BuildError::IndexOverflow { kind: "lexemes" })?;
        let phonetic = match pronunciations.get(lemma) {
            Some(phonetic) => phonetic.clone(),
            None => String::new(),
        };
        transaction.execute(
            "INSERT INTO lexemes (id, lemma, phonetic) VALUES (?1, ?2, ?3)",
            params![id, lemma, phonetic],
        )?;
        lemma_ids.insert(lemma.clone(), id);
        for (rank, (part_of_speech, definition)) in data.senses.iter().enumerate() {
            let rank =
                i64::try_from(rank).map_err(|_| BuildError::IndexOverflow { kind: "senses" })?;
            transaction.execute(
                "INSERT INTO senses (lexeme_id, rank, part_of_speech, definition)
                 VALUES (?1, ?2, ?3, ?4)",
                params![id, rank, part_of_speech, definition],
            )?;
            sense_count = sense_count.saturating_add(1);
        }
    }

    let mut forms = BTreeSet::<(String, i64, i64)>::new();
    for (lemma, id) in &lemma_ids {
        forms.insert((lemma.clone(), *id, 0));
    }
    for surface in pronunciations.keys() {
        for candidate in lemma_candidates(surface) {
            if let Some(id) = lemma_ids.get(&candidate) {
                forms.insert((surface.clone(), *id, i64::from(surface != &candidate)));
                break;
            }
        }
    }
    for (surface, id, priority) in &forms {
        transaction.execute(
            "INSERT INTO forms (surface, lexeme_id, priority) VALUES (?1, ?2, ?3)",
            params![surface, id, priority],
        )?;
    }
    transaction.execute(
        "CREATE INDEX forms_lookup_idx ON forms(surface, priority, lexeme_id)",
        [],
    )?;
    transaction.commit()?;
    connection.execute_batch("VACUUM;")?;

    Ok(BuildReport {
        lexemes: lexemes.len() as u64,
        forms: forms.len() as u64,
        senses: sense_count,
    })
}

pub fn compress_lexicon(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
) -> Result<String, BuildError> {
    let input = input.as_ref();
    let output = output.as_ref();
    if output.exists() {
        return Err(BuildError::OutputExists(output.to_path_buf()));
    }
    let mut source = std::fs::File::open(input).map_err(|error| BuildError::Io {
        path: input.to_path_buf(),
        source: error,
    })?;
    let target = std::fs::File::create(output).map_err(|error| BuildError::Io {
        path: output.to_path_buf(),
        source: error,
    })?;
    zstd::stream::copy_encode(&mut source, target, 19).map_err(|error| BuildError::Io {
        path: output.to_path_buf(),
        source: error,
    })?;

    let mut compressed = std::fs::File::open(output).map_err(|error| BuildError::Io {
        path: output.to_path_buf(),
        source: error,
    })?;
    let mut hasher = sha2::Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = compressed
            .read(&mut buffer)
            .map_err(|error| BuildError::Io {
                path: output.to_path_buf(),
                source: error,
            })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let digest = hasher.finalize();
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

pub fn export_browser_lexicon(
    database: impl AsRef<Path>,
    output: impl AsRef<Path>,
) -> Result<BrowserExportReport, BuildError> {
    let database = database.as_ref();
    let output = output.as_ref();
    if output.exists() {
        return Err(BuildError::OutputExists(output.to_path_buf()));
    }

    let connection = Connection::open(database)?;
    let definition_source = metadata_value(&connection, "definition_source")?;
    let pronunciation_source = metadata_value(&connection, "pronunciation_source")?;
    let mut entry_statement = connection
        .prepare("SELECT id, lemma, phonetic FROM lexemes ORDER BY lemma COLLATE BINARY, id")?;
    let raw_entries = entry_statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(entry_statement);

    let mut entries = Vec::with_capacity(raw_entries.len());
    let mut entry_indices = HashMap::with_capacity(raw_entries.len());
    let mut sense_statement = connection.prepare(
        "SELECT part_of_speech, definition
         FROM senses
         WHERE lexeme_id = ?1 AND definition != ''
         ORDER BY rank ASC
         LIMIT 3",
    )?;
    for (entry_index, (id, lemma, phonetic)) in raw_entries.into_iter().enumerate() {
        let encoded_index = u32::try_from(entry_index).map_err(|_| BuildError::IndexOverflow {
            kind: "browser lexemes",
        })?;
        let definitions = sense_statement
            .query_map([id], |row| {
                Ok(Definition {
                    part_of_speech: row.get(0)?,
                    text: row.get(1)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        entry_indices.insert(id, encoded_index);
        entries.push(LexiconEntry {
            lemma,
            phonetic,
            definitions,
        });
    }
    drop(sense_statement);

    let mut form_statement = connection.prepare(
        "SELECT f.surface, f.lexeme_id, f.priority
         FROM forms f
         JOIN lexemes l ON l.id = f.lexeme_id
         ORDER BY f.surface COLLATE NOCASE, f.priority ASC, l.lemma ASC, l.id ASC",
    )?;
    let raw_forms = form_statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut forms = Vec::with_capacity(raw_forms.len());
    for (surface, lexeme_id, priority) in raw_forms {
        let normalized = surface.trim().to_lowercase();
        if let Some(entry_index) = entry_indices.get(&lexeme_id) {
            forms.push(BrowserLexiconForm {
                surface: normalized,
                entry_index: *entry_index,
                priority: u8::try_from(priority).map_err(|_| BuildError::IndexOverflow {
                    kind: "browser form priority",
                })?,
            });
        }
    }
    forms.sort_by(|left, right| {
        (&left.surface, left.priority, left.entry_index).cmp(&(
            &right.surface,
            right.priority,
            right.entry_index,
        ))
    });
    let asset = BrowserLexiconAsset {
        schema_version: BROWSER_LEXICON_SCHEMA_VERSION,
        definition_source,
        pronunciation_source,
        entries,
        forms,
    };
    let encoded = asset.encode()?;
    let mut encoder = GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), Compression::best());
    encoder
        .write_all(&encoded)
        .map_err(|source| BuildError::Io {
            path: output.to_path_buf(),
            source,
        })?;
    let compressed = encoder.finish().map_err(|source| BuildError::Io {
        path: output.to_path_buf(),
        source,
    })?;
    let mut target = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .map_err(|source| BuildError::Io {
            path: output.to_path_buf(),
            source,
        })?;
    if let Err(source) = target.write_all(&compressed) {
        let _ = std::fs::remove_file(output);
        return Err(BuildError::Io {
            path: output.to_path_buf(),
            source,
        });
    }

    Ok(BrowserExportReport {
        lexemes: asset.entries.len() as u64,
        forms: asset.forms.len() as u64,
        uncompressed_bytes: encoded.len() as u64,
        compressed_bytes: compressed.len() as u64,
        sha256: sha256_bytes(&compressed),
    })
}

pub fn export_browser_lexicon_from_zstd(
    compressed_database: impl AsRef<Path>,
    output: impl AsRef<Path>,
) -> Result<BrowserExportReport, BuildError> {
    let compressed_database = compressed_database.as_ref();
    let output = output.as_ref();
    if output.exists() {
        return Err(BuildError::OutputExists(output.to_path_buf()));
    }
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temporary =
        tempfile::NamedTempFile::new_in(parent).map_err(|source| BuildError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    let mut source = std::fs::File::open(compressed_database).map_err(|source| BuildError::Io {
        path: compressed_database.to_path_buf(),
        source,
    })?;
    zstd::stream::copy_decode(&mut source, &mut temporary).map_err(|source| BuildError::Io {
        path: compressed_database.to_path_buf(),
        source,
    })?;
    temporary.flush().map_err(|source| BuildError::Io {
        path: temporary.path().to_path_buf(),
        source,
    })?;
    export_browser_lexicon(temporary.path(), output)
}

fn metadata_value(connection: &Connection, key: &str) -> Result<String, BuildError> {
    connection
        .query_row("SELECT value FROM metadata WHERE key = ?1", [key], |row| {
            row.get(0)
        })
        .map_err(BuildError::from)
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let digest = sha2::Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn parse_oewn<R: BufRead>(
    source: R,
) -> Result<(Vec<LexicalEntry>, HashMap<String, String>), BuildError> {
    let mut reader = Reader::from_reader(source);
    reader.config_mut().trim_text(false);
    let mut buffer = Vec::new();
    let mut entries = Vec::new();
    let mut pending_entry: Option<PendingEntry> = None;
    let mut current_synset: Option<String> = None;
    let mut in_definition = false;
    let mut definition_text = String::new();
    let mut definitions = HashMap::new();

    loop {
        match reader.read_event_into(&mut buffer)? {
            Event::Eof => break,
            Event::Start(event) => match event.name().as_ref() {
                b"LexicalEntry" => pending_entry = Some(PendingEntry::default()),
                b"Lemma" => set_lemma(&reader, &event, pending_entry.as_mut())?,
                b"Sense" => push_sense(&reader, &event, pending_entry.as_mut())?,
                b"Synset" => current_synset = attribute(&reader, &event, b"id")?,
                b"Definition" => {
                    in_definition = true;
                    definition_text.clear();
                }
                _ => {}
            },
            Event::Empty(event) => match event.name().as_ref() {
                b"Lemma" => set_lemma(&reader, &event, pending_entry.as_mut())?,
                b"Sense" => push_sense(&reader, &event, pending_entry.as_mut())?,
                _ => {}
            },
            Event::Text(event) if in_definition => {
                let decoded = event.xml_content()?;
                definition_text.push_str(&quick_xml::escape::unescape(&decoded)?);
            }
            Event::End(event) => match event.name().as_ref() {
                b"LexicalEntry" => {
                    if let Some(entry) = pending_entry.take() {
                        if let (Some(lemma), Some(part_of_speech)) =
                            (entry.lemma, entry.part_of_speech)
                        {
                            if !entry.synsets.is_empty() {
                                entries.push(LexicalEntry {
                                    lemma: lemma.to_lowercase(),
                                    part_of_speech,
                                    synsets: entry.synsets,
                                });
                            }
                        }
                    }
                }
                b"Definition" => {
                    if let Some(synset) = &current_synset {
                        let definition = definition_text.trim();
                        if !definition.is_empty() {
                            definitions
                                .entry(synset.clone())
                                .or_insert_with(|| definition.to_string());
                        }
                    }
                    in_definition = false;
                }
                b"Synset" => current_synset = None,
                _ => {}
            },
            _ => {}
        }
        buffer.clear();
    }

    Ok((entries, definitions))
}

fn set_lemma<R: BufRead>(
    reader: &Reader<R>,
    event: &BytesStart<'_>,
    entry: Option<&mut PendingEntry>,
) -> Result<(), BuildError> {
    if let Some(entry) = entry {
        entry.lemma = attribute(reader, event, b"writtenForm")?;
        entry.part_of_speech = attribute(reader, event, b"partOfSpeech")?
            .map(|value| part_of_speech(&value).to_string());
    }
    Ok(())
}

fn push_sense<R: BufRead>(
    reader: &Reader<R>,
    event: &BytesStart<'_>,
    entry: Option<&mut PendingEntry>,
) -> Result<(), BuildError> {
    if let (Some(entry), Some(synset)) = (entry, attribute(reader, event, b"synset")?) {
        entry.synsets.push(synset);
    }
    Ok(())
}

fn attribute<R: BufRead>(
    reader: &Reader<R>,
    event: &BytesStart<'_>,
    key: &[u8],
) -> Result<Option<String>, BuildError> {
    for attribute in event.attributes().with_checks(false) {
        let attribute = attribute.map_err(quick_xml::Error::from)?;
        if attribute.key.as_ref() == key {
            return Ok(Some(
                attribute
                    .decode_and_unescape_value(reader.decoder())?
                    .into_owned(),
            ));
        }
    }
    Ok(None)
}

fn part_of_speech(value: &str) -> &str {
    match value {
        "n" => "noun",
        "v" => "verb",
        "a" | "s" => "adjective",
        "r" => "adverb",
        other => other,
    }
}

fn parse_cmudict<R: BufRead>(source: R) -> Result<BTreeMap<String, String>, BuildError> {
    let mut pronunciations = BTreeMap::new();
    for line in source.lines() {
        let line = line.map_err(BuildError::CmudictRead)?;
        let line = line.trim();
        if line.is_empty() || line.starts_with(";;;") {
            continue;
        }
        let mut fields = line.split_whitespace();
        let Some(raw_word) = fields.next() else {
            continue;
        };
        let word = raw_word
            .split_once('(')
            .map_or(raw_word, |(base, _)| base)
            .to_lowercase();
        let phonemes: Vec<&str> = fields.collect();
        let phonetic = arpabet_to_ipa(&phonemes);
        if !phonetic.is_empty() {
            pronunciations.entry(word).or_insert(phonetic);
        }
    }
    Ok(pronunciations)
}

fn arpabet_to_ipa(phonemes: &[&str]) -> String {
    let vowel_count = phonemes
        .iter()
        .filter(|phoneme| {
            phoneme
                .chars()
                .last()
                .is_some_and(|last| last.is_ascii_digit())
        })
        .count();
    let mut ipa = String::new();
    for phoneme in phonemes {
        let stress = phoneme.chars().last().filter(char::is_ascii_digit);
        let symbol = stress.map_or(*phoneme, |_| &phoneme[..phoneme.len() - 1]);
        if vowel_count > 1 {
            match stress {
                Some('1') => ipa.push('ˈ'),
                Some('2') => ipa.push('ˌ'),
                _ => {}
            }
        }
        let mapped = match (symbol, stress) {
            ("AA", _) => "ɑ",
            ("AE", _) => "æ",
            ("AH", Some('0')) => "ə",
            ("AH", _) => "ʌ",
            ("AO", _) => "ɔ",
            ("AW", _) => "aʊ",
            ("AY", _) => "aɪ",
            ("B", _) => "b",
            ("CH", _) => "tʃ",
            ("D", _) => "d",
            ("DH", _) => "ð",
            ("EH", _) => "ɛ",
            ("ER", Some('0')) => "ɚ",
            ("ER", _) => "ɝ",
            ("EY", _) => "eɪ",
            ("F", _) => "f",
            ("G", _) => "ɡ",
            ("HH", _) => "h",
            ("IH", _) => "ɪ",
            ("IY", _) => "i",
            ("JH", _) => "dʒ",
            ("K", _) => "k",
            ("L", _) => "l",
            ("M", _) => "m",
            ("N", _) => "n",
            ("NG", _) => "ŋ",
            ("OW", _) => "oʊ",
            ("OY", _) => "ɔɪ",
            ("P", _) => "p",
            ("R", _) => "ɹ",
            ("S", _) => "s",
            ("SH", _) => "ʃ",
            ("T", _) => "t",
            ("TH", _) => "θ",
            ("UH", _) => "ʊ",
            ("UW", _) => "u",
            ("V", _) => "v",
            ("W", _) => "w",
            ("Y", _) => "j",
            ("Z", _) => "z",
            ("ZH", _) => "ʒ",
            _ => "",
        };
        ipa.push_str(mapped);
    }
    ipa
}
