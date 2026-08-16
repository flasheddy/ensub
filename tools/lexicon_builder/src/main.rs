use std::ffi::OsString;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use ensub_lexicon_builder::{build_lexicon, compress_lexicon, export_browser_lexicon_from_zstd};
use flate2::read::GzDecoder;

fn main() -> Result<()> {
    let arguments: Vec<OsString> = std::env::args_os().collect();
    if let [_, command, input, output] = arguments.as_slice() {
        if command == "export-browser" {
            let input = PathBuf::from(input);
            let output = PathBuf::from(output);
            let report = export_browser_lexicon_from_zstd(&input, &output).with_context(|| {
                format!("failed to export browser lexicon from {}", input.display())
            })?;
            println!(
                "exported {} lexemes and {} forms ({} bytes compressed, sha256 {})",
                report.lexemes, report.forms, report.compressed_bytes, report.sha256
            );
            return Ok(());
        }
    }
    if arguments.len() != 5 {
        bail!(concat!(
            "usage:\n",
            "  ensub-lexicon-builder <oewn.xml.gz> <cmudict.dict> <output.sqlite3> <output.sqlite3.zst>\n",
            "  ensub-lexicon-builder export-browser <input.sqlite3.zst> <output.postcard.gz>"
        ));
    }

    let oewn_path = PathBuf::from(&arguments[1]);
    let cmudict_path = PathBuf::from(&arguments[2]);
    let database_path = PathBuf::from(&arguments[3]);
    let compressed_path = PathBuf::from(&arguments[4]);
    let oewn = File::open(&oewn_path)
        .with_context(|| format!("failed to open {}", oewn_path.display()))?;
    let cmudict = File::open(&cmudict_path)
        .with_context(|| format!("failed to open {}", cmudict_path.display()))?;

    let report = build_lexicon(
        BufReader::new(GzDecoder::new(oewn)),
        BufReader::new(cmudict),
        &database_path,
    )
    .context("failed to generate lexicon database")?;
    let digest = compress_lexicon(&database_path, &compressed_path)
        .context("failed to compress lexicon database")?;

    println!(
        "generated {} lexemes, {} forms, {} senses; compressed sha256 {}",
        report.lexemes, report.forms, report.senses, digest
    );
    Ok(())
}
