# Ensub (`esb`) Agent Guidelines

## Project Scope

- Use Rust 2021 for every crate in this workspace.
- Keep domain models, scheduling policy, and storage interfaces in `crates/core_engine`.
- Cargo workspace packages are `core_engine`, `language_engine`, `ensub-theme`,
  `ensub-sqlite`, `ensub-cli`, `ensub-tui`, `ensub-wasm`, `ensub-gui`,
  `ensub-applet`, `ensub-llm`, `ensub-lexicon-builder`, `ensub-uniffi`, and
  `ensub-uniffi-bindgen`.
- `android/` is a first-class client project but is not a Cargo workspace
  package. `bindings/ensub-uniffi/` is its supported Rust entry point.
- Keep unrelated changes out of focused work.

## Platform Lifecycle

- Milestone 6 / v0.3.0 Native Android is the active client track. The delivered
  Spike 1 scaffold remains the baseline; Spike 2 background playback is the
  current implementation target.
- The v0.2.0 PWA/Web Player and `ensub-wasm` are frozen reference surfaces.
  Changes under `crates/web_player` and `crates/wasm_bridge` are limited to
  maintenance, compatibility, security, and regression work.
- Do not delete, skip, weaken, or replace PWA/WASM tests to accommodate Android.
  Shared-core contract changes must keep the applicable browser regression
  baseline green.
- Treat [Platform Status](docs/platform-status.md) as the lifecycle authority
  and [.private/prd.md](.private/prd.md) as the active Milestone 6 behavioral
  specification. Do not duplicate their detailed matrices in agent guidance.
- Additional subtree rules live in `android/AGENTS.md` and
  `bindings/ensub-uniffi/AGENTS.md`.

## Governance Triggers

Re-audit `.private/prd.md`, `docs/platform-status.md`, `docs/architecture.md`,
`docs/development.md`, and scoped `AGENTS.md` files when a change affects:

- Cargo workspace membership or top-level client layout;
- Rust, Kotlin, and UniFFI ownership boundaries;
- active or frozen platform status;
- SQLite schema ownership or migration policy;
- canonical `scripts/verify.sh` modes or CI gates; or
- Milestone 6 exit criteria and activation of a later milestone.

Report any required milestone transition to the owner before changing PRD
status, archiving a PRD, or modifying project agent policy.

## Architecture

- `core_engine` must not depend on UI, CLI, desktop, WASM, database-driver, or async-runtime crates.
- Platform adapters depend on `core_engine`; `core_engine` never depends on an adapter.
- Implement native SQLite and browser storage behind `StorageAdapter` outside `core_engine`.
- Keep parsing, morphology, and lexicon contracts portable in `language_engine`.
- Keep `rusqlite`, platform data paths, and bundled native lexicon extraction in `ensub-sqlite` or binary crates.
- Keep scheduling and due-date calculations pure by accepting timestamps as arguments. Do not read the system clock inside domain logic.
- Rust owns podcast and transcript parsing, time math, cue selection, token and
  lexicon policy, capture construction, scheduling, and SQLite persistence.
- Kotlin/Android owns Compose UI, Media3 playback and lifecycle, notifications,
  audio focus, Android permissions, connectivity, HTTP transport, and disposable
  media caching. Kotlin must not issue SQL or reproduce Rust domain policy.
- Keep `ensub-uniffi` a thin native-client facade over portable Rust engines and
  storage adapters. Do not place Android framework types, UI state machines,
  transport clients, or a second copy of domain policy in the facade.
- Android-specific UI, Media3, lifecycle, JNI/UniFFI, database-driver, and async-
  runtime dependencies must not enter `core_engine` or `language_engine`.

## Rust Conventions

- Use `thiserror` for errors defined by library crates and `anyhow` at binary application boundaries.
- Do not use `panic!`, `unwrap`, or `expect` in production paths. Return typed, actionable errors instead.
- Prefer owned `String`, `Vec<T>`, and straightforward cloning over explicit lifetime-heavy designs unless borrowing materially simplifies the implementation.
- Simple borrowed function parameters are acceptable when they do not introduce stored references or explicit lifetime parameters.
- Add or update focused tests for every behavior change.

## Validation

Run these commands before reporting Rust work complete:

1. `cargo fmt --all -- --check`
2. `cargo check --workspace`
3. `cargo clippy --workspace --all-targets -- -D warnings`
4. `cargo test --workspace`
5. `cargo tree -p core_engine`
6. `cargo tree -p language_engine`

Run target-specific checks for every active crate affected by the change.

For changes under `android/` or `bindings/ensub-uniffi/`, also run:

1. `cargo fmt --all -- --check`
2. `sh scripts/verify.sh android`
3. applicable connected Android tests when behavior depends on a service,
   manifest integration, lifecycle, permissions, or the native library

Do not hardcode a developer-specific Android SDK path. Set `ANDROID_HOME` or
`ANDROID_SDK_ROOT` in the environment. Report connected or physical-device
checks that could not be run.

For `ensub-wasm`, also run target check and Clippy for `wasm32-unknown-unknown`,
browser tests through `wasm-pack`, the static web build, and dependency-tree
inspection confirming that native SQLite and UI crates do not enter the WASM graph.

Before committing any change, run `sh scripts/verify.sh secrets` and
`git diff --check`. Stage named files only and review the staged diff.
