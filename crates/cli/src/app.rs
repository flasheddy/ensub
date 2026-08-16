use std::io::{Read, Write};

use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, Utc};
use core_engine::{schedule_review, Capture, ReviewUpdate, StorageAdapter};
use language_engine::{
    capture_from_candidate, capture_from_entry, extract_candidates, Candidate, Lexicon,
    ParseOptions,
};

use crate::{AddArgs, Command, ParseArgs, Prompt, ReviewArgs, ReviewResponse};

pub fn execute<S, L, P, R, W, C>(
    command: &Command,
    storage: &mut S,
    lexicon: Option<&L>,
    prompt: &mut P,
    input: &mut R,
    output: &mut W,
    clock: &C,
) -> Result<()>
where
    S: StorageAdapter,
    L: Lexicon,
    P: Prompt,
    R: Read,
    W: Write,
    C: Fn() -> DateTime<Utc>,
{
    match command {
        Command::Add(args) => add(args, storage, require_lexicon(lexicon)?, output, clock()),
        Command::Parse(args) => parse(
            args,
            storage,
            require_lexicon(lexicon)?,
            prompt,
            input,
            output,
            clock(),
        ),
        Command::Review(args) => review(args, storage, prompt, output, clock()),
        Command::Due => {
            let count = storage
                .due_count(clock())
                .map_err(|error| anyhow!("failed to query due cards: {error}"))?;
            writeln!(output, "{count}").context("failed to write due count")
        }
        Command::Stats => stats(storage, output, clock()),
        Command::Tui(_) => bail!("the TUI must be launched by the full-screen terminal host"),
    }
}

fn require_lexicon<L: Lexicon>(lexicon: Option<&L>) -> Result<&L> {
    lexicon.context("this command requires the bundled lexicon")
}

fn add<S: StorageAdapter, L: Lexicon, W: Write>(
    args: &AddArgs,
    storage: &mut S,
    lexicon: &L,
    output: &mut W,
    captured_at: DateTime<Utc>,
) -> Result<()> {
    if args.source.is_some() && args.context.is_none() {
        bail!("--source requires --context");
    }
    let surface = args.word.trim();
    if surface.is_empty() {
        bail!("word cannot be empty");
    }
    let entry = lexicon
        .lookup(surface)
        .map_err(|error| anyhow!("offline dictionary lookup failed: {error}"))?
        .with_context(|| format!("{surface:?} was not found in the offline dictionary"))?;
    let capture = capture_from_entry(
        surface,
        args.context.as_deref(),
        args.source.as_deref().map_or("cli:add", |source| source),
        entry,
        captured_at,
    );
    storage
        .save_capture(&capture)
        .map_err(|error| anyhow!("failed to save capture: {error}"))?;
    writeln!(
        output,
        "captured {} /{}/\n{}",
        capture.word.lemma, capture.word.phonetic, capture.word.definition
    )
    .context("failed to write capture summary")
}

fn parse<S: StorageAdapter, L: Lexicon, P: Prompt, R: Read, W: Write>(
    args: &ParseArgs,
    storage: &mut S,
    lexicon: &L,
    prompt: &mut P,
    input: &mut R,
    output: &mut W,
    captured_at: DateTime<Utc>,
) -> Result<()> {
    if !args.yes && !prompt.is_interactive() {
        bail!("interactive candidate selection requires a terminal; use --yes to capture all");
    }

    let mut text = String::new();
    input
        .read_to_string(&mut text)
        .context("failed to read text from standard input")?;
    let report = extract_candidates(
        &text,
        lexicon,
        ParseOptions {
            include_stopwords: args.include_stopwords,
            max_candidates: args.max_candidates,
        },
    )
    .map_err(|error| anyhow!("failed to parse input text: {error}"))?;

    let selected = if args.yes {
        (0..report.candidates.len()).collect()
    } else {
        let labels: Vec<String> = report.candidates.iter().map(candidate_label).collect();
        prompt.select_candidates(&labels)?
    };
    let source = args.source.as_deref().map_or("cli:stdin", |source| source);
    let captures: Vec<Capture> = selected
        .into_iter()
        .map(|index| {
            report
                .candidates
                .get(index)
                .map(|candidate| capture_from_candidate(candidate, source, captured_at))
                .ok_or_else(|| anyhow!("candidate selection index {index} is out of range"))
        })
        .collect::<Result<_>>()?;
    storage
        .save_captures(&captures)
        .map_err(|error| anyhow!("failed to save parsed captures: {error}"))?;

    for capture in &captures {
        writeln!(
            output,
            "{} /{}/\n{}",
            capture.word.lemma, capture.word.phonetic, capture.word.definition
        )
        .context("failed to write parsed capture details")?;
    }

    writeln!(
        output,
        "captured {} cards ({} dictionary misses)",
        captures.len(),
        report.lookup_misses
    )
    .context("failed to write parse summary")
}

fn review<S: StorageAdapter, P: Prompt, W: Write>(
    args: &ReviewArgs,
    storage: &mut S,
    prompt: &mut P,
    output: &mut W,
    reviewed_at: DateTime<Utc>,
) -> Result<()> {
    if !prompt.is_interactive() {
        bail!("review requires an interactive terminal");
    }
    let mut cards = storage
        .due_reviews(reviewed_at)
        .map_err(|error| anyhow!("failed to query due reviews: {error}"))?;
    if let Some(limit) = args.limit {
        cards.truncate(limit);
    }
    let total = cards.len();

    for (index, card) in cards.into_iter().enumerate() {
        writeln!(output, "{}/{} {}", index + 1, total, card.word.lemma)
            .context("failed to write review prompt")?;
        if let Some(context) = card.contexts.first() {
            writeln!(output, "{}", context.sentence).context("failed to write review context")?;
            writeln!(output, "source: {}", context.source)
                .context("failed to write review source")?;
        }
        prompt.wait_for_reveal()?;
        writeln!(output, "/{}/\n{}", card.word.phonetic, card.word.definition)
            .context("failed to write review answer")?;
        let ReviewResponse::Rating(rating) = prompt.review_response()? else {
            break;
        };
        let replacement = schedule_review(&card.state, rating, reviewed_at)
            .context("failed to schedule reviewed card")?;
        match storage
            .commit_review(&card.state, &replacement, reviewed_at)
            .map_err(|error| anyhow!("failed to save review result: {error}"))?
        {
            ReviewUpdate::Updated => {
                let suffix = if replacement.interval_days == 1 {
                    "day"
                } else {
                    "days"
                };
                writeln!(
                    output,
                    "next interval: {} {suffix}",
                    replacement.interval_days
                )
                .context("failed to write review result")?;
            }
            ReviewUpdate::Conflict => bail!(
                "review state for {} changed in another process; retry the review",
                card.word.lemma
            ),
        }
    }
    Ok(())
}

fn stats<S: StorageAdapter, W: Write>(
    storage: &S,
    output: &mut W,
    as_of: DateTime<Utc>,
) -> Result<()> {
    let stats = storage
        .review_statistics(as_of)
        .map_err(|error| anyhow!("failed to query review statistics: {error}"))?;
    writeln!(output, "total: {}", stats.total_cards)?;
    writeln!(output, "due: {}", stats.due_cards)?;
    writeln!(output, "0d: {}", stats.intervals.new)?;
    writeln!(output, "1-6d: {}", stats.intervals.days_1_to_6)?;
    writeln!(output, "7-30d: {}", stats.intervals.days_7_to_30)?;
    writeln!(output, "31-90d: {}", stats.intervals.days_31_to_90)?;
    writeln!(output, "91+d: {}", stats.intervals.days_91_plus)?;
    Ok(())
}

fn candidate_label(candidate: &Candidate) -> String {
    format!(
        "{} -> {} /{}/  {}",
        candidate.surface, candidate.entry.lemma, candidate.entry.phonetic, candidate.sentence
    )
}
