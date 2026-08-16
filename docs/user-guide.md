# User Guide

Ensub follows the same basic workflow on every surface:

1. Read or paste English text.
2. Inspect a dictionary-backed word in its sentence.
3. Capture the word and context.
4. Review the card when it becomes due.
5. Submit a recall quality from 0 through 5.

Native surfaces share one SQLite database by default. The contextual web app is an
independent browser-only environment and does not synchronize with native
data.

## Recall Ratings

Ensub uses the SM-2 quality scale presented by the CLI:

| Rating | Meaning |
|---:|---|
| `0` | Complete blackout |
| `1` | Incorrect; remembered after reveal |
| `2` | Incorrect; answer felt familiar |
| `3` | Correct with serious difficulty |
| `4` | Correct with hesitation |
| `5` | Perfect recall |

Ratings below 3 reset the repetition count and schedule a one-day relearning
interval. Passing intervals progress through 1 day, 6 days, and then the prior
interval multiplied by the card's ease factor. The ease factor never falls
below 1.3.

## CLI

Run `esb --help` or `esb <COMMAND> --help` for the executable's authoritative
option list.

### `esb add`

Capture one word using the bundled offline dictionary:

```bash
esb add <WORD> [--context <SENTENCE>] [--source <REFERENCE>]
```

Example:

```bash
esb add acquire \
  --context "Readers acquire vocabulary through repeated encounters." \
  --source "notes/immersion.md"
```

The output includes the resolved lemma, IPA pronunciation, and definition.
Capturing the same lemma is idempotent; a new context can be added without
resetting the existing review schedule. `--source` is accepted only with
`--context`.

### `esb parse`

Read all input from standard input, segment it into sentences and words, and
look up capture candidates:

```bash
cat article.txt | esb parse [OPTIONS]
```

| Option | Behavior |
|---|---|
| `--source <REFERENCE>` | Store a source label instead of `cli:stdin` |
| `--yes` | Capture every resolved candidate without selection prompts |
| `--include-stopwords` | Include common English stopwords |
| `--max-candidates <N>` | Limit resolved candidates; defaults to 100 |

Without `--yes`, the command requires an interactive terminal and opens a
multi-select prompt. Dictionary misses are counted but are not captured.

### `esb review`

Review cards that were due when the session started:

```bash
esb review [--limit <N>]
```

Press Enter to reveal each answer, choose a rating from 0 through 5, or choose
the quit item to end the session. Review writes use optimistic concurrency; if
another surface updates the same card first, the CLI reports a conflict rather
than overwriting the newer state.

### `esb due` and `esb stats`

`esb due` prints only the current due-card count, making it suitable for shell
prompts and status scripts:

```bash
due_count="$(esb due)"
```

`esb stats` prints total and due cards plus interval buckets: `0d`, `1-6d`,
`7-30d`, `31-90d`, and `91+d`.

### Database Override

All CLI commands accept a global `--database <PATH>` option and the equivalent
`ENSUB_DATABASE_PATH` environment variable:

```bash
esb --database /tmp/ensub-demo.sqlite3 stats
ENSUB_DATABASE_PATH=/tmp/ensub-demo.sqlite3 esb due
```

Place the global option before the subcommand for consistent shell usage.

## TUI Reader

Launch with an optional Markdown or plain-text path:

```bash
esb tui [FILE_PATH]
```

The primary pane renders the document with a focused word. On wide terminals,
the vocabulary panel shows its dictionary entry and SRS state. The status bar
shows mode, path or latest notice, reading progress, and due count.

### Reader Keys

| Key | Action |
|---|---|
| `h` / `l` | Move to the previous or next word on the visual line |
| `j` / `k` | Move down or up one visual line |
| `gg` / `G` | Jump to the beginning or end |
| `Ctrl+u` / `Ctrl+d` | Move half a page up or down |
| `c` | Capture the focused word and surrounding sentence |
| `r` | Open the quick-review overlay |
| `o` | Open the file-path prompt |
| `Tab` | Toggle the side panel on wide terminals; open its overlay on narrow terminals |
| `q` / `Esc` | Exit from Reader mode |
| `Ctrl+c` | Exit from any mode |

The vocabulary overlay closes with `Tab`, `q`, or `Esc`.

### File Prompt and Review Overlay

The file prompt supports ordinary text editing, arrows, Home, End, Backspace,
and Delete. Press Enter to load the path. Esc returns to an open document or
exits if no document has been loaded.

In quick review, press Enter or Space to reveal the answer, then press `0`
through `5` to rate it. Press `q` or Esc to return to the reader.

## COSMIC Desktop GUI

Launch `ensub-gui` for the complete desktop application. Its five pages are:

| Page | Capability |
|---|---|
| Dashboard | Due/interval statistics, 30-day review activity, and recent review history |
| Library | Search, sort, paginate, and inspect captured cards and contexts |
| Reader | Read Markdown/plain text, inspect inline words, and capture sentence context |
| Parse Text | Paste text, choose candidates, and capture them atomically |
| Review | Reveal, skip, and rate due cards |

### Global Navigation

Global shortcuts apply when a text input is not actively consuming the event:

| Key | Action |
|---|---|
| `1` | Dashboard |
| `2` | Library |
| `3` | Reader |
| `4` | Parse Text |
| `5` | Review |
| `Tab` / `Shift+Tab` | Cycle pages forward or backward |
| `Esc` | Release active widget focus back to the page canvas |

When an active text input captures Tab, focus advances between widgets instead
of changing pages.

### Reader Navigation

The Reader keeps the document in a scrollable 65% pane and the focused word in
a persistent 35% inspector at widths of 750 pixels or more. Below 750 pixels,
the inspector stacks beneath the document. Click any dictionary candidate to
focus it directly.

| Key | Action |
|---|---|
| `w` / `l` / Right arrow | Focus the next word |
| `b` / `h` / Left arrow | Focus the previous word |
| `j` / Down arrow | Scroll down |
| `k` / Up arrow | Scroll up |
| `c` / Enter | Capture the focused word and sentence |
| `o` | Open the native Markdown/plain-text file picker |

The inspector shows the dictionary lemma, part of speech, IPA pronunciation,
definitions, capture/due status, surrounding sentence, and capture action.

### Capture HUD

Run `ensub-gui --capture` to open the compact capture window. It loads clipboard
text, parses dictionary-backed candidates, and captures the selected items to
the native database. Esc closes the HUD.

The GUI accepts `--database <PATH>` or `ENSUB_DATABASE=<PATH>` for development
database overrides. This environment variable currently differs from the CLI's
`ENSUB_DATABASE_PATH`.

## COSMIC Panel Applet

`ensub-applet` displays `ESB <COUNT>` and refreshes its due count every 30
seconds. Open the popover to:

- reveal and rate the next due card;
- see the word's context, pronunciation, and definition;
- launch `ensub-gui --capture` for clipboard capture.

The badge caps display at `99+`. The applet uses the default native database
path and expects `ensub-gui` beside the applet binary or available on `PATH`.

## Contextual Web Assistant

Enter a target word or phrase, the sentence where it appeared, and optional
surrounding context. "Analyze & Save" returns the lemma, part of speech,
context-specific definition, nuance, and confidence, then adds the complete
encounter to Recent Captures.

The web app creates an anonymous Supabase session automatically. Its history is
private to that browser identity and remains separate from native SQLite. A
network connection is required for authentication, analysis, and history.

See the [Contextual Web App README](../crates/web_site/README.md) for backend
configuration, build, preview, privacy, and deployment instructions.
