use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "esb",
    version,
    about = "English immersion and spaced repetition"
)]
pub struct Cli {
    /// Override the user database path.
    #[arg(long, global = true, env = "ENSUB_DATABASE_PATH")]
    pub database: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Capture one word from the bundled offline dictionary.
    Add(AddArgs),
    /// Extract capture candidates from text read on standard input.
    Parse(ParseArgs),
    /// Review cards due at the start of this session.
    Review(ReviewArgs),
    /// Print the number of currently due cards.
    Due,
    /// Print collection and interval statistics.
    Stats,
    /// Open the full-screen reader and review panel.
    Tui(TuiArgs),
}

#[derive(Debug, Args)]
pub struct AddArgs {
    pub word: String,

    #[arg(long)]
    pub context: Option<String>,

    #[arg(long)]
    pub source: Option<String>,
}

#[derive(Debug, Args)]
pub struct ParseArgs {
    #[arg(long)]
    pub source: Option<String>,

    /// Capture every resolvable candidate without prompting.
    #[arg(long)]
    pub yes: bool,

    #[arg(long)]
    pub include_stopwords: bool,

    #[arg(long, default_value_t = 100)]
    pub max_candidates: usize,
}

#[derive(Debug, Args)]
pub struct ReviewArgs {
    #[arg(long)]
    pub limit: Option<usize>,
}

#[derive(Debug, Args)]
pub struct TuiArgs {
    /// Markdown or plain-text file to open; omit to enter a path in the reader.
    pub file_path: Option<PathBuf>,
}
