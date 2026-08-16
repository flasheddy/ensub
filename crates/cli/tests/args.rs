use std::path::PathBuf;

use clap::Parser;
use ensub_cli::{Cli, Command};

#[test]
fn parses_add_and_global_database_override() {
    let cli = Cli::try_parse_from([
        "esb",
        "--database",
        "/tmp/ensub-test.sqlite3",
        "add",
        "went",
        "--context",
        "She went home.",
        "--source",
        "book:1",
    ])
    .expect("add arguments must parse");

    assert_eq!(cli.database, Some(PathBuf::from("/tmp/ensub-test.sqlite3")));
    let Command::Add(arguments) = cli.command else {
        panic!("expected add command");
    };
    assert_eq!(arguments.word, "went");
    assert_eq!(arguments.context.as_deref(), Some("She went home."));
    assert_eq!(arguments.source.as_deref(), Some("book:1"));
}

#[test]
fn parses_parse_review_due_and_stats_contracts() {
    let parse = Cli::try_parse_from([
        "esb",
        "parse",
        "--source",
        "article:42",
        "--yes",
        "--include-stopwords",
        "--max-candidates",
        "25",
    ])
    .expect("parse arguments must parse");
    let review = Cli::try_parse_from(["esb", "review", "--limit", "10"])
        .expect("review arguments must parse");

    assert!(matches!(parse.command, Command::Parse(_)));
    assert!(matches!(review.command, Command::Review(_)));
    assert!(matches!(
        Cli::try_parse_from(["esb", "due"])
            .expect("due must parse")
            .command,
        Command::Due
    ));
    assert!(matches!(
        Cli::try_parse_from(["esb", "stats"])
            .expect("stats must parse")
            .command,
        Command::Stats
    ));
}

#[test]
fn parses_tui_with_optional_file_path() {
    let without_file = Cli::try_parse_from(["esb", "tui"]).expect("tui must parse without a file");
    let with_file =
        Cli::try_parse_from(["esb", "tui", "docs/article.md"]).expect("tui file must parse");

    let Command::Tui(without_file) = without_file.command else {
        panic!("expected tui command");
    };
    let Command::Tui(with_file) = with_file.command else {
        panic!("expected tui command");
    };
    assert_eq!(without_file.file_path, None);
    assert_eq!(with_file.file_path, Some(PathBuf::from("docs/article.md")));
}
