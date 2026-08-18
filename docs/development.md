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
- Static-site tooling: Bun 1.3.14, Playwright 1.62.1, and Chromium

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
  web_player/         installable podcast and transcript workspace
  web_sandbox/        offline Ensub Core reference harness
  web_site/           optional online Ensub Context application
packaging/             local release scripts and desktop integration
scripts/               canonical verification entry point
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
WASM_PACK_CACHE="$PWD/target/wasm-pack-cache" \
  wasm-pack test --headless --firefox crates/wasm_bridge
cargo tree -p ensub-wasm --target wasm32-unknown-unknown
```

The WASM tree must not contain `rusqlite`, `libcosmic`, `ensub-gui`, or
`ensub-applet`.

## Ensub Player

The player build compiles the shared WASM package, generates theme CSS, copies
the versioned browser lexicon sidecars, and verifies a fully precached PWA:

```bash
cd crates/web_player
bun install --frozen-lockfile
bun test
bun run build
bun run verify:dist
bun run test:browser
```

The browser suite covers demo feed import, DOM audio controls, cue highlighting,
manual-follow behavior, Rust UTF-16 token rendering, keyboard lookup, explicit
and repeated media capture, persistence, offline reload, and desktop/mobile
layout. WASM storage tests also cover v1-to-v2 migration, exact-byte recovery
after failed writes, unknown lookup, and multi-context lemma association.

## Ensub Core Offline Sandbox

The Core build compiles `ensub-wasm`, verifies/decompresses the committed
browser lexicon, generates a content-addressed service worker, and rejects
remote endpoint or cloud credential references:

```bash
cd crates/web_sandbox
bun install --frozen-lockfile
bun test
bun run build
bun run verify:dist
bun run test:browser
```

The Playwright suite uses the real lexicon, records 30 warm parser samples and
requires p95 below 100 ms, exercises reload persistence and SRS review, and
tests multi-tab Web Locks behavior plus a controlled offline reload.

## Ensub Context

Context scripts test and assemble the optional online companion into `dist`:

```bash
cd crates/web_site
bun test
bun run build
bun run verify:dist
bun run serve
```

The static build requires Rust 1.93 and runs the `ensub-theme-css` exporter to
create `dist/theme.css` before copying the HTML, component CSS, JavaScript, and
retirement service worker. Generated theme CSS stays
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

## Verification and Local Release

Run an individual verification gate or the complete local sequence:

```bash
sh scripts/verify.sh rust
sh scripts/verify.sh wasm
sh scripts/verify.sh web
sh scripts/verify.sh release-smoke
sh scripts/verify.sh secrets
sh scripts/verify.sh all
```

Create only the deterministic v0.1.0-rc1 native archives with:

```bash
sh packaging/build-release.sh
```

This stages `esb`, the GUI/applet, metadata, icons, licenses, lexicon
provenance, and third-party notices under a temporary `DESTDIR`; validates and
smoke-tests that staged tree; then writes two archives and `SHA256SUMS` under
`target/release-artifacts`. No verification or packaging mode publishes.
