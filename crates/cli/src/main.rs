use std::io;
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use clap::Parser;
use ensub_cli::{execute, lexicon_cache_dir, resolve_database_path, Cli, Command, TerminalPrompt};
use ensub_sqlite::{BundledLexicon, SqliteStorage};
use ensub_tui::TuiConfig;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let database_path = resolve_database_path(cli.database)?;
    let mut storage = SqliteStorage::open(&database_path)
        .with_context(|| format!("failed to open database {}", database_path.display()))?;
    let lexicon = open_lexicon_if_needed(&cli.command)?;
    if let Command::Tui(arguments) = &cli.command {
        let lexicon = lexicon
            .as_ref()
            .context("the TUI requires the bundled lexicon")?;
        let working_directory =
            std::env::current_dir().context("failed to resolve the current working directory")?;
        return ensub_tui::run(
            TuiConfig {
                initial_file: arguments.file_path.clone(),
                working_directory,
            },
            &mut storage,
            lexicon,
            now,
        )
        .context("TUI session failed");
    }
    let mut prompt = TerminalPrompt::stderr();
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();

    execute(
        &cli.command,
        &mut storage,
        lexicon.as_ref(),
        &mut prompt,
        &mut input,
        &mut output,
        &now,
    )
}

fn open_lexicon_if_needed(command: &Command) -> Result<Option<BundledLexicon>> {
    if !matches!(
        command,
        Command::Add(_) | Command::Parse(_) | Command::Tui(_)
    ) {
        return Ok(None);
    }
    let cache_dir: PathBuf = lexicon_cache_dir()?;
    BundledLexicon::open(&cache_dir)
        .map(Some)
        .with_context(|| format!("failed to open bundled lexicon in {}", cache_dir.display()))
}

fn now() -> DateTime<Utc> {
    DateTime::<Utc>::from(std::time::SystemTime::now())
}
