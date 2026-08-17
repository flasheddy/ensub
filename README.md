<p align="center">
  <img src="packaging/icons/dev.ensub.Ensub.svg" width="96" height="96" alt="Ensub application icon">
</p>

# Ensub (`esb`)

Ensub is a local-first English immersion and spaced-repetition application suite. It
turns words encountered in documents or pasted text into vocabulary cards with
an offline definition, pronunciation, source context, and deterministic SM-2
review schedule.

The same Rust domain and language engines power a command-line interface, a
terminal reader, a native COSMIC desktop application and panel applet, and
portable WASM bindings. Native data stays in SQLite. The separate Ensub Core
sandbox is an offline browser reference harness backed by `localStorage`. The
optional Ensub Context companion uses an anonymous Supabase session and an
OpenAI-compatible model without exposing provider credentials to the browser.

## Included Surfaces

The repository includes these implemented surfaces:

| Surface | Entry point | Current capability |
|---|---|---|
| CLI | `esb` | Add and parse vocabulary, review due cards, inspect due counts and statistics |
| TUI | `esb tui [FILE]` | Read Markdown or plain text, inspect and capture words, run quick reviews |
| COSMIC GUI | `ensub-gui` | Dashboard, vocabulary library, document reader, text capture, and review sessions |
| COSMIC applet | `ensub-applet` | Due-count badge, one-card review popover, and clipboard capture HUD |
| Ensub Player | `crates/web_player` | Installable podcast audio workspace with synchronized, locally cached transcripts |
| Ensub Core sandbox | `crates/web_sandbox` | Offline real-lexicon parsing, capture, SRS review, snapshots, and multi-tab coordination |
| Ensub Context | `crates/web_site` | Optional online contextual analysis and private cloud-backed capture history |

The native desktop packaging targets Linux with COSMIC Desktop.

## Quick Start

Ensub requires Rust `1.93` or newer. From a source checkout, install the CLI and
its TUI reader with:

```bash
cargo install --path crates/cli --locked
```

Capture a word and its source sentence:

```bash
esb add immersion \
  --context "Immersion turns ordinary reading into deliberate practice." \
  --source "reading-notes"
```

Extract and capture every dictionary-backed candidate from standard input:

```bash
printf '%s\n' "Reading authentic material builds durable vocabulary." \
  | esb parse --yes --source "terminal-example"
```

The first captured card is immediately due:

```bash
esb due
esb review
esb stats
```

Open the terminal reader with a Markdown or plain-text document:

```bash
esb tui article.md
```

Ensub embeds its native lexicon, creates the SQLite schema automatically, and
uses the platform-standard local data and cache directories. No dictionary
download or database setup is required.

## Native Desktop

Build the COSMIC application and applet:

```bash
cargo build --release -p ensub-cli -p ensub-gui -p ensub-applet
```

For a user-local installation of `esb`, desktop binaries, entries, applet entry,
metadata, and icons:

```bash
PREFIX="$HOME/.local" sh packaging/install.sh
```

Ensure `$HOME/.local/bin` is on `PATH`. The applet entry can then be added from
COSMIC panel settings. Distribution-specific COSMIC and graphics development
packages may be required to compile `libcosmic`.

Create the two deterministic local v0.1.0-rc1 archives and `SHA256SUMS` with:

```bash
sh packaging/build-release.sh
```

This command writes only under `target/release-artifacts`; it does not publish
or upload artifacts.

See [Getting Started](docs/getting-started.md) for development launch commands
and the contextual web assistant setup.

## Documentation

| Guide | Contents |
|---|---|
| [Getting Started](docs/getting-started.md) | Prerequisites, installation, first capture, and surface launch commands |
| [User Guide](docs/user-guide.md) | CLI options, TUI keys, GUI navigation, applet, and web workflows |
| [Architecture](docs/architecture.md) | Crate boundaries, data flow, storage adapters, and concurrency model |
| [Development](docs/development.md) | Workspace layout, validation commands, tests, web builds, and release builds |
| [Data and Privacy](docs/data-and-privacy.md) | Native and browser storage, path overrides, concurrency, backup, and reset behavior |
| [Offline Lexicon](docs/lexicon.md) | Corpus provenance, generated artifacts, extraction, and regeneration |
| [Ensub Player](crates/web_player/README.md) | PWA build, local cache, direct browser fetching, and preview |
| [Ensub Core Sandbox](crates/web_sandbox/README.md) | Offline WASM build, verification, and local preview |
| [Ensub Context](crates/web_site/README.md) | Supabase setup, LLM secrets, build, preview, and privacy |

API documentation can be generated locally with:

```bash
cargo doc --workspace --no-deps --open
```

## Architecture at a Glance

Portable policy is separated from platform I/O:

```text
core_engine       domain records, SM-2 scheduling, storage contracts
language_engine   tokenization, morphology, documents, lexicon contracts
ensub-theme       portable semantic RGB themes and CSS export
ensub-sqlite      native SQLite storage and bundled offline lexicon
ensub-cli         command dispatch and terminal prompts
ensub-tui         terminal reader and quick-review state machine
ensub-gui         native COSMIC desktop application and capture HUD
ensub-applet      native COSMIC panel applet
ensub-wasm        browser bindings and local snapshot storage
web_player        installable podcast and synchronized-transcript workspace
web_sandbox       offline Ensub Core reference harness
web_site          optional online Ensub Context companion
```

`core_engine` has no UI, database, platform, async-runtime, or WASM dependency.
Native and browser adapters depend inward on its `StorageAdapter` contract.
Visual frontends adapt the shared `ensub-theme` semantic colors to their own
toolkits; Catppuccin Mocha Mauve is the default preset.

## Development

Run the repository's primary checks from the workspace root:

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

WASM and static-site development has additional target and browser checks;
they are listed in [Development](docs/development.md).

## Lexicon Attribution

Definitions and parts of speech are derived from Open English WordNet 2025.
Pronunciations are derived from CMUdict 0.7b and converted from ARPAbet to broad
General American IPA. The generated lexicon contains 32,463 lexemes, 49,207
surface forms, and 78,463 ranked senses.

Pinned source versions, checksums, transformation notes, and third-party
license notices are under
[`crates/sqlite_storage/assets`](crates/sqlite_storage/assets/README.md).

## License

Ensub source code is available under either the [MIT License](LICENSE-MIT) or
the [Apache License 2.0](LICENSE-APACHE). Bundled lexical data retains its
source licenses and attribution requirements as documented in the lexicon
provenance files.
