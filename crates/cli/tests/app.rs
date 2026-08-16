use std::collections::HashMap;
use std::convert::Infallible;
use std::io::Cursor;

use anyhow::Result;
use chrono::{DateTime, TimeZone, Utc};
use core_engine::{LibraryStorageAdapter, ReviewHistoryQuery, ReviewRating, StorageAdapter};
use ensub_cli::{
    execute, AddArgs, Command, ParseArgs, Prompt, ReviewArgs, ReviewResponse, TuiArgs,
};
use ensub_sqlite::SqliteStorage;
use language_engine::{Definition, Lexicon, LexiconEntry};

struct FakeLexicon {
    entries: HashMap<String, LexiconEntry>,
}

impl FakeLexicon {
    fn new() -> Self {
        let entries = [
            ("went", "go", "ɡoʊ"),
            ("go", "go", "ɡoʊ"),
            ("mice", "mouse", "maʊs"),
            ("home", "home", "hoʊm"),
        ]
        .into_iter()
        .map(|(surface, lemma, phonetic)| {
            (
                surface.to_string(),
                LexiconEntry {
                    lemma: lemma.to_string(),
                    phonetic: phonetic.to_string(),
                    definitions: vec![Definition {
                        part_of_speech: "verb".to_string(),
                        text: format!("definition of {lemma}"),
                    }],
                },
            )
        })
        .collect();
        Self { entries }
    }
}

impl Lexicon for FakeLexicon {
    type Error = Infallible;

    fn lookup(&self, surface: &str) -> Result<Option<LexiconEntry>, Self::Error> {
        Ok(self.entries.get(&surface.to_lowercase()).cloned())
    }
}

#[derive(Default)]
struct ScriptedPrompt {
    interactive: bool,
    selected: Vec<usize>,
    responses: Vec<ReviewResponse>,
}

impl Prompt for ScriptedPrompt {
    fn is_interactive(&self) -> bool {
        self.interactive
    }

    fn select_candidates(&mut self, _labels: &[String]) -> Result<Vec<usize>> {
        Ok(self.selected.clone())
    }

    fn wait_for_reveal(&mut self) -> Result<()> {
        Ok(())
    }

    fn review_response(&mut self) -> Result<ReviewResponse> {
        if self.responses.is_empty() {
            return Ok(ReviewResponse::Quit);
        }
        Ok(self.responses.remove(0))
    }
}

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 8, 14, 10, 0, 0)
        .single()
        .expect("test timestamp must be valid")
}

fn execute_command(
    command: &Command,
    storage: &mut SqliteStorage,
    lexicon: &FakeLexicon,
    prompt: &mut ScriptedPrompt,
    input: &str,
) -> Result<String> {
    let mut output = Vec::new();
    execute(
        command,
        storage,
        Some(lexicon),
        prompt,
        &mut Cursor::new(input.as_bytes()),
        &mut output,
        &now,
    )?;
    Ok(String::from_utf8(output).expect("command output must be UTF-8"))
}

#[test]
fn add_persists_complete_card_and_reports_due_count() {
    let mut storage = SqliteStorage::open_in_memory().expect("database must open");
    let lexicon = FakeLexicon::new();
    let mut prompt = ScriptedPrompt::default();

    let output = execute_command(
        &Command::Add(AddArgs {
            word: "went".to_string(),
            context: Some("She went home.".to_string()),
            source: Some("book:1".to_string()),
        }),
        &mut storage,
        &lexicon,
        &mut prompt,
        "",
    )
    .expect("add must succeed");

    assert!(output.contains("go /ɡoʊ/"));
    assert_eq!(storage.due_count(now()).expect("count must query"), 1);
    let card = storage
        .due_reviews(now())
        .expect("cards must query")
        .remove(0);
    assert_eq!(card.word.term, "went");
    assert_eq!(card.word.lemma, "go");
    assert_eq!(card.contexts[0].source, "book:1");
}

#[test]
fn parse_yes_captures_all_unique_resolvable_lemmas_atomically() {
    let mut storage = SqliteStorage::open_in_memory().expect("database must open");
    let lexicon = FakeLexicon::new();
    let mut prompt = ScriptedPrompt::default();
    let command = Command::Parse(ParseArgs {
        source: Some("article:1".to_string()),
        yes: true,
        include_stopwords: false,
        max_candidates: 100,
    });

    let output = execute_command(
        &command,
        &mut storage,
        &lexicon,
        &mut prompt,
        "The mice went home. Went again.",
    )
    .expect("parse must succeed");

    assert!(output.contains("captured 3 cards"));
    assert!(output.contains("mouse /maʊs/"));
    assert!(output.contains("verb: definition of mouse"));
    assert_eq!(storage.due_count(now()).expect("count must query"), 3);
}

#[test]
fn parse_without_yes_or_terminal_refuses_before_writing() {
    let mut storage = SqliteStorage::open_in_memory().expect("database must open");
    let lexicon = FakeLexicon::new();
    let mut prompt = ScriptedPrompt::default();
    let command = Command::Parse(ParseArgs {
        source: None,
        yes: false,
        include_stopwords: false,
        max_candidates: 100,
    });

    let result = execute_command(
        &command,
        &mut storage,
        &lexicon,
        &mut prompt,
        "Mice went home.",
    );

    assert!(result.is_err());
    assert_eq!(storage.due_count(now()).expect("count must query"), 0);
}

#[test]
fn review_reveals_rates_and_reschedules_due_card() {
    let mut storage = SqliteStorage::open_in_memory().expect("database must open");
    let lexicon = FakeLexicon::new();
    let mut prompt = ScriptedPrompt {
        interactive: true,
        responses: vec![ReviewResponse::Rating(
            ReviewRating::try_from(4).expect("rating must be valid"),
        )],
        ..ScriptedPrompt::default()
    };
    execute_command(
        &Command::Add(AddArgs {
            word: "went".to_string(),
            context: Some("She went home.".to_string()),
            source: Some("book:review".to_string()),
        }),
        &mut storage,
        &lexicon,
        &mut ScriptedPrompt::default(),
        "",
    )
    .expect("add must succeed");

    let output = execute_command(
        &Command::Review(ReviewArgs { limit: None }),
        &mut storage,
        &lexicon,
        &mut prompt,
        "",
    )
    .expect("review must succeed");

    assert!(output.contains("1/1 go"));
    assert!(output.contains("source: book:review"));
    assert!(output.contains("next interval: 1 day"));
    assert_eq!(storage.due_count(now()).expect("count must query"), 0);
    assert_eq!(
        storage
            .review_history(&ReviewHistoryQuery::default())
            .expect("review history must query")
            .total,
        1
    );
}

#[test]
fn add_rejects_dictionary_miss_and_source_without_context() {
    let mut storage = SqliteStorage::open_in_memory().expect("database must open");
    let lexicon = FakeLexicon::new();

    let miss = execute_command(
        &Command::Add(AddArgs {
            word: "unlisted".to_string(),
            context: None,
            source: None,
        }),
        &mut storage,
        &lexicon,
        &mut ScriptedPrompt::default(),
        "",
    );
    let invalid_source = execute_command(
        &Command::Add(AddArgs {
            word: "went".to_string(),
            context: None,
            source: Some("book:1".to_string()),
        }),
        &mut storage,
        &lexicon,
        &mut ScriptedPrompt::default(),
        "",
    );

    assert!(miss.is_err());
    assert!(invalid_source.is_err());
    assert_eq!(storage.due_count(now()).expect("count must query"), 0);
}

#[test]
fn due_and_stats_have_stable_machine_readable_output() {
    let mut storage = SqliteStorage::open_in_memory().expect("database must open");
    let lexicon = FakeLexicon::new();
    execute_command(
        &Command::Add(AddArgs {
            word: "went".to_string(),
            context: None,
            source: None,
        }),
        &mut storage,
        &lexicon,
        &mut ScriptedPrompt::default(),
        "",
    )
    .expect("add must succeed");

    let due = execute_command(
        &Command::Due,
        &mut storage,
        &lexicon,
        &mut ScriptedPrompt::default(),
        "",
    )
    .expect("due must succeed");
    let stats = execute_command(
        &Command::Stats,
        &mut storage,
        &lexicon,
        &mut ScriptedPrompt::default(),
        "",
    )
    .expect("stats must succeed");

    assert_eq!(due, "1\n");
    assert_eq!(
        stats,
        "total: 1\ndue: 1\n0d: 1\n1-6d: 0\n7-30d: 0\n31-90d: 0\n91+d: 0\n"
    );
}

#[test]
fn line_executor_rejects_tui_dispatch() {
    let mut storage = SqliteStorage::open_in_memory().expect("database must open");
    let lexicon = FakeLexicon::new();

    let result = execute_command(
        &Command::Tui(TuiArgs { file_path: None }),
        &mut storage,
        &lexicon,
        &mut ScriptedPrompt::default(),
        "",
    );

    assert!(result
        .expect_err("headless executor must reject TUI")
        .to_string()
        .contains("full-screen terminal host"));
}
