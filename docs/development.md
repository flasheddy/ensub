# Development

This guide describes the current workspace workflow. Repository-wide coding
rules remain authoritative in [`AGENTS.md`](../AGENTS.md).

## Toolchain

- Rust edition: 2021
- Minimum declared Rust version: 1.93
- Cargo resolver: 2
- Native database: bundled SQLite through `rusqlite`
- Desktop toolkit: pinned `libcosmic` Git revision
- Web target: `wasm32-unknown-unknown`
- Static-site tooling: Bun 1.3.14 and Node.js built-ins; no registry dependencies

The committed `Cargo.lock` is part of the application workspace and must be
used for reproducible installation and validation.

## Workspace Map

```text
crates/
  cli/                esb binary and command orchestration
  core_engine/        domain, SRS, and storage contracts
  cosmic_applet/      COSMIC panel applet
  cosmic_gui/         COSMIC desktop application and HUD
  language_engine/    parsing, morphology, lexicons, documents
  llm_client/          optional OpenAI-compatible disambiguation adapter
  sqlite_storage/     native persistence and bundled lexicon
  theme/              semantic RGB themes and CSS exporter
  tui/                Ratatui reader and review panel
  wasm_bridge/        browser API and snapshot persistence
  web_site/           static contextual vocabulary application
packaging/             desktop integration and icons
tools/
  lexicon_builder/    reproducible dictionary asset generation
```

The detailed dependency graph is in [Architecture](architecture.md).

## Common Commands

Build or run individual surfaces during development:

```bash
cargo run -p ensub-cli -- --help
cargo run -p ensub-cli -- tui article.md
cargo run -p ensub-gui --bin ensub-gui
cargo run -p ensub-gui --bin ensub-gui -- --capture
cargo build -p ensub-applet
```

The applet is designed to run inside COSMIC's panel host. Install its desktop
metadata from `packaging/` for an end-to-end panel test.

## Required Native Validation

Run these commands from the repository root before reporting Rust work
complete:

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo tree -p core_engine
cargo tree -p language_engine
```

The dependency trees must preserve these boundaries:

- `core_engine` contains no UI, platform, database driver, WASM, or async
  runtime dependency.
- `language_engine` contains no UI or native database dependency.

Focused commands are useful while iterating, but do not replace the full
workspace sequence:

```bash
cargo test -p core_engine
cargo test -p language_engine
cargo test -p ensub-llm
cargo test -p ensub-sqlite
cargo test -p ensub-cli
cargo test -p ensub-tui
cargo test -p ensub-theme
cargo test -p ensub-gui
cargo test -p ensub-applet
```

Production Rust paths must not use `.unwrap()`, `.expect()`, or `panic!`.
Tests may use them to make fixture failures explicit. Library crates define
typed errors with `thiserror`; binary application boundaries use `anyhow`.

## WASM Validation

Install the target and `wasm-pack`, then run:

```bash
cargo check -p ensub-wasm --target wasm32-unknown-unknown
cargo clippy -p ensub-wasm --target wasm32-unknown-unknown -- -D warnings
wasm-pack test --headless --firefox crates/wasm_bridge
cargo tree -p ensub-wasm --target wasm32-unknown-unknown
```

The WASM tree must not contain `rusqlite`, `libcosmic`, `ensub-gui`, or
`ensub-applet`.

## Static Web Site

The site scripts test and assemble the vanilla browser application into `dist`:

```bash
cd crates/web_site
bun install --frozen-lockfile
bun test
bun run build
bun run verify:dist
bun run serve
```

The static build requires Rust 1.93 and runs the `ensub-theme-css` exporter to
create `dist/theme.css` before copying the HTML, component CSS, JavaScript, and
retirement service worker required by GitHub Pages. Generated theme CSS stays
ignored and is not checked in. Supabase schema and Edge Function source remain
deployment inputs rather than browser assets. See the web app README for
backend setup and secret names.

## Offline Lexicon Development

Normal builds use committed generated assets and do not download source
corpora. Regeneration requires explicitly obtained Open English WordNet and
CMUdict inputs:

```bash
cargo run -p ensub-lexicon-builder -- \
  <oewn.xml.gz> \
  <cmudict.dict> \
  <output.sqlite3> \
  <output.sqlite3.zst>
```

Export a browser artifact from the compressed native database:

```bash
cargo run -p ensub-lexicon-builder -- \
  export-browser \
  <input.sqlite3.zst> \
  <output.postcard.gz>
```

Any committed regenerated asset must be accompanied by updated provenance,
checksums, corpus counts, attribution, and tests. See [Offline Lexicon](lexicon.md).

## Documentation Changes

When changing behavior:

1. Update the closest crate-level Rust documentation and focused tests.
2. Update the relevant user or development guide.
3. Keep the root README concise and link to detail rather than duplicating it.
4. Verify Markdown links and every command shown against the current CLI or
   build scripts.

Generate local API documentation with:

```bash
cargo doc --workspace --no-deps
```

## Release Build

Build all installable native binaries with the lockfile:

```bash
cargo build --release --locked \
  -p ensub-cli \
  -p ensub-gui \
  -p ensub-applet
```

The current packaging script installs the GUI and applet integration files.
It does not package `esb`, publish archives, or build the web site. Those are
still separate release operations.
