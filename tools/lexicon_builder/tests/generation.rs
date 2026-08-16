use std::io::{self, BufRead, Cursor, Read};

use ensub_lexicon_builder::{
    build_lexicon, compress_lexicon, export_browser_lexicon, export_browser_lexicon_from_zstd,
    BuildError,
};
use ensub_sqlite::SqliteLexicon;
use flate2::read::GzDecoder;
use language_engine::{BrowserLexicon, Lexicon};
use tempfile::TempDir;

const MINI_OEWN: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<LexicalResource>
  <Lexicon id="oewn">
    <LexicalEntry id="go-v">
      <Lemma writtenForm="go" partOfSpeech="v"/>
      <Sense id="go-1" synset="go-synset-1"/>
      <Sense id="go-2" synset="go-synset-2"/>
    </LexicalEntry>
    <LexicalEntry id="mouse-n">
      <Lemma writtenForm="mouse" partOfSpeech="n"/>
      <Sense id="mouse-1" synset="mouse-synset-1"/>
    </LexicalEntry>
    <Synset id="go-synset-1" partOfSpeech="v">
      <Definition>change location</Definition>
    </Synset>
    <Synset id="go-synset-2" partOfSpeech="v">
      <Definition>move along</Definition>
    </Synset>
    <Synset id="mouse-synset-1" partOfSpeech="n">
      <Definition>a small rodent</Definition>
    </Synset>
  </Lexicon>
</LexicalResource>"#;

const MINI_CMU: &str = "go G OW1\nwent W EH1 N T\nmouse M AW1 S\nmice M AY1 S\n";

#[test]
fn generates_lookup_database_with_ipa_senses_and_inflected_forms() {
    let temp = TempDir::new().expect("temporary directory must create");
    let output = temp.path().join("lexicon.sqlite3");

    let report = build_lexicon(
        Cursor::new(MINI_OEWN.as_bytes()),
        Cursor::new(MINI_CMU.as_bytes()),
        &output,
    )
    .expect("miniature lexicon must generate");
    let lexicon = SqliteLexicon::open(&output).expect("generated lexicon must open");

    let went = lexicon
        .lookup("went")
        .expect("lookup must execute")
        .expect("went must resolve");
    let mice = lexicon
        .lookup("mice")
        .expect("lookup must execute")
        .expect("mice must resolve");

    assert_eq!(went.lemma, "go");
    assert_eq!(went.phonetic, "ɡoʊ");
    assert_eq!(went.definitions.len(), 2);
    assert_eq!(mice.lemma, "mouse");
    assert_eq!(mice.phonetic, "maʊs");
    assert_eq!(report.lexemes, 2);
    assert!(report.forms >= 4);
    assert_eq!(report.senses, 3);
}

#[test]
fn compression_round_trip_returns_sha256_digest() {
    let temp = TempDir::new().expect("temporary directory must create");
    let input = temp.path().join("input.bin");
    let compressed = temp.path().join("input.bin.zst");
    std::fs::write(&input, b"deterministic lexicon bytes").expect("fixture bytes must write");

    let digest = compress_lexicon(&input, &compressed).expect("compression must succeed");
    let decoded = zstd::stream::decode_all(
        std::fs::File::open(&compressed).expect("compressed file must open"),
    )
    .expect("compressed file must decode");

    assert_eq!(decoded, b"deterministic lexicon bytes");
    assert_eq!(digest.len(), 64);
    assert!(digest
        .chars()
        .all(|character| character.is_ascii_hexdigit()));
}

#[test]
fn browser_export_is_deterministic_and_matches_sqlite_lookup() {
    let temp = TempDir::new().expect("temporary directory must create");
    let database = temp.path().join("lexicon.sqlite3");
    let first_output = temp.path().join("lexicon-v1.postcard.gz");
    let second_output = temp.path().join("lexicon-v1-copy.postcard.gz");
    build_lexicon(
        Cursor::new(MINI_OEWN.as_bytes()),
        Cursor::new(MINI_CMU.as_bytes()),
        &database,
    )
    .expect("miniature lexicon must generate");

    let first =
        export_browser_lexicon(&database, &first_output).expect("browser lexicon must export");
    let second = export_browser_lexicon(&database, &second_output)
        .expect("second browser lexicon must export");
    let first_compressed = std::fs::read(&first_output).expect("first export must read");
    let second_compressed = std::fs::read(&second_output).expect("second export must read");
    let mut decoded = Vec::new();
    GzDecoder::new(first_compressed.as_slice())
        .read_to_end(&mut decoded)
        .expect("browser export must decompress");
    let browser = BrowserLexicon::decode(&decoded).expect("browser export must decode");
    let sqlite = SqliteLexicon::open(&database).expect("SQLite lexicon must open");

    assert_eq!(first, second);
    assert_eq!(first_compressed, second_compressed);
    assert_eq!(first.lexemes, 2);
    assert!(first.forms >= 4);
    assert_eq!(
        browser.lookup("went").expect("browser lookup must run"),
        sqlite.lookup("went").expect("SQLite lookup must run")
    );
    assert_eq!(
        browser.lookup("mice").expect("browser lookup must run"),
        sqlite.lookup("mice").expect("SQLite lookup must run")
    );
    assert_eq!(first.sha256.len(), 64);
}

#[test]
fn browser_export_refuses_to_replace_an_existing_asset() {
    let temp = TempDir::new().expect("temporary directory must create");
    let database = temp.path().join("lexicon.sqlite3");
    let output = temp.path().join("lexicon-v1.postcard.gz");
    build_lexicon(
        Cursor::new(MINI_OEWN.as_bytes()),
        Cursor::new(MINI_CMU.as_bytes()),
        &database,
    )
    .expect("miniature lexicon must generate");
    std::fs::write(&output, b"existing").expect("fixture output must write");

    let error = export_browser_lexicon(&database, &output)
        .expect_err("existing browser asset must be preserved");

    assert!(matches!(error, BuildError::OutputExists(path) if path == output));
}

#[test]
fn compressed_sqlite_asset_exports_without_leaving_a_database_copy() {
    let temp = TempDir::new().expect("temporary directory must create");
    let database = temp.path().join("lexicon.sqlite3");
    let compressed_database = temp.path().join("lexicon.sqlite3.zst");
    let browser_output = temp.path().join("lexicon-v1.postcard.gz");
    build_lexicon(
        Cursor::new(MINI_OEWN.as_bytes()),
        Cursor::new(MINI_CMU.as_bytes()),
        &database,
    )
    .expect("miniature lexicon must generate");
    compress_lexicon(&database, &compressed_database).expect("database must compress");
    std::fs::remove_file(&database).expect("uncompressed fixture must remove");

    let report = export_browser_lexicon_from_zstd(&compressed_database, &browser_output)
        .expect("compressed database must export");

    assert_eq!(report.lexemes, 2);
    assert!(browser_output.exists());
    assert!(!database.exists());
    let remaining_databases = std::fs::read_dir(temp.path())
        .expect("temporary directory must list")
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|value| value == "sqlite3")
        })
        .count();
    assert_eq!(remaining_databases, 0);
}

struct FailingReader;

impl Read for FailingReader {
    fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "damaged CMUdict",
        ))
    }
}

impl BufRead for FailingReader {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "damaged CMUdict",
        ))
    }

    fn consume(&mut self, _amount: usize) {}
}

#[test]
fn cmudict_read_error_aborts_generation() {
    let temp = TempDir::new().expect("temporary directory must create");
    let output = temp.path().join("lexicon.sqlite3");

    let error = build_lexicon(Cursor::new(MINI_OEWN.as_bytes()), FailingReader, &output)
        .expect_err("damaged CMUdict must fail generation");

    assert!(matches!(error, BuildError::CmudictRead(_)));
    assert!(!output.exists());
}
