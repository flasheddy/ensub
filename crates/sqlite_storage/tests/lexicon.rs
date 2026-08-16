use ensub_sqlite::{BundledLexicon, SqliteLexicon};
use language_engine::Lexicon;
use rusqlite::Connection;
use tempfile::TempDir;

fn fixture() -> (TempDir, std::path::PathBuf) {
    let temp = TempDir::new().expect("temporary directory must create");
    let path = temp.path().join("lexicon.sqlite3");
    let connection = Connection::open(&path).expect("fixture database must open");
    connection
        .execute_batch(
            "CREATE TABLE metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL) STRICT;
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
             CREATE INDEX forms_lookup_idx ON forms(surface, priority, lexeme_id);
             CREATE TABLE senses (
                lexeme_id INTEGER NOT NULL REFERENCES lexemes(id),
                rank INTEGER NOT NULL,
                part_of_speech TEXT NOT NULL,
                definition TEXT NOT NULL,
                PRIMARY KEY(lexeme_id, rank)
             ) STRICT;
             INSERT INTO metadata VALUES ('schema_version', '1');
             INSERT INTO lexemes VALUES (1, 'go', 'ɡoʊ');
             INSERT INTO forms VALUES ('go', 1, 0), ('went', 1, 1);
             INSERT INTO senses VALUES
                (1, 0, 'verb', 'change location'),
                (1, 1, 'verb', 'move along'),
                (1, 2, 'noun', 'a turn in a game'),
                (1, 3, 'verb', 'become');",
        )
        .expect("fixture schema must create");
    drop(connection);
    (temp, path)
}

#[test]
fn lookup_resolves_inflected_form_to_lemma_ipa_and_three_ranked_senses() {
    let (_temp, path) = fixture();
    let lexicon = SqliteLexicon::open(&path).expect("fixture lexicon must open");

    let entry = lexicon
        .lookup("WENT")
        .expect("lookup must execute")
        .expect("went must resolve");

    assert_eq!(entry.lemma, "go");
    assert_eq!(entry.phonetic, "ɡoʊ");
    assert_eq!(entry.definitions.len(), 3);
    assert_eq!(entry.definitions[0].part_of_speech, "verb");
    assert_eq!(entry.definitions[0].text, "change location");
    assert_eq!(entry.definitions[2].text, "a turn in a game");
}

#[test]
fn lookup_returns_none_for_missing_surface() {
    let (_temp, path) = fixture();
    let lexicon = SqliteLexicon::open(&path).expect("fixture lexicon must open");

    assert!(lexicon
        .lookup("unknown")
        .expect("lookup must execute")
        .is_none());
}

#[test]
fn bundled_lexicon_extracts_verifies_and_reuses_versioned_cache() {
    let temp = TempDir::new().expect("temporary directory must create");

    let first = BundledLexicon::open(temp.path()).expect("bundled lexicon must install");
    let first_path = first.installed_path().to_path_buf();
    let went = first
        .lookup("went")
        .expect("lookup must execute")
        .expect("bundled lexicon must contain went");
    drop(first);
    let second = BundledLexicon::open(temp.path()).expect("installed lexicon must reopen");

    assert_eq!(went.lemma, "go");
    assert!(!went.phonetic.is_empty());
    assert!(!went.definitions.is_empty());
    assert_eq!(second.installed_path(), first_path);
}

#[test]
fn bundled_lexicon_has_pinned_schema_and_corpus_counts() {
    let temp = TempDir::new().expect("temporary directory must create");
    let lexicon = BundledLexicon::open(temp.path()).expect("bundled lexicon must install");
    let connection = Connection::open_with_flags(
        lexicon.installed_path(),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .expect("installed lexicon must open read-only");

    let integrity: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .expect("integrity check must execute");
    let schema: String = connection
        .query_row(
            "SELECT value FROM metadata WHERE key = 'schema_version'",
            [],
            |row| row.get(0),
        )
        .expect("schema metadata must query");
    let lexemes: i64 = connection
        .query_row("SELECT COUNT(*) FROM lexemes", [], |row| row.get(0))
        .expect("lexeme count must query");
    let forms: i64 = connection
        .query_row("SELECT COUNT(*) FROM forms", [], |row| row.get(0))
        .expect("form count must query");
    let senses: i64 = connection
        .query_row("SELECT COUNT(*) FROM senses", [], |row| row.get(0))
        .expect("sense count must query");

    assert_eq!(integrity, "ok");
    assert_eq!(schema, "1");
    assert_eq!((lexemes, forms, senses), (32_463, 49_207, 78_463));
}
