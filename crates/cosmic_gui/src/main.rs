use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use ensub_gui::{run, GuiFlags};
use ensub_sqlite::{lexicon_cache_dir, resolve_database_path};

#[derive(Debug, Parser)]
#[command(name = "ensub-gui")]
struct Args {
    #[arg(long)]
    capture: bool,

    #[arg(long, env = "ENSUB_DATABASE")]
    database: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    run(GuiFlags {
        database_path: resolve_database_path(args.database)?,
        lexicon_cache_dir: lexicon_cache_dir()?,
        capture_mode: args.capture,
    })?;
    Ok(())
}
