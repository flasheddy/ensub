# Ensub (`esb`) Agent Guidelines

## Project Scope

- Use Rust 2021 for every crate in this workspace.
- Keep domain models, scheduling policy, and storage interfaces in `crates/core_engine`.
- Workspace packages are `core_engine`, `language_engine`, `ensub-theme`, `ensub-sqlite`, `ensub-cli`, `ensub-tui`, `ensub-wasm`, `ensub-gui`, `ensub-applet`, `ensub-llm`, and `ensub-lexicon-builder`.
- Keep unrelated changes out of focused work.

## Architecture

- `core_engine` must not depend on UI, CLI, desktop, WASM, database-driver, or async-runtime crates.
- Platform adapters depend on `core_engine`; `core_engine` never depends on an adapter.
- Implement native SQLite and browser storage behind `StorageAdapter` outside `core_engine`.
- Keep parsing, morphology, and lexicon contracts portable in `language_engine`.
- Keep `rusqlite`, platform data paths, and bundled native lexicon extraction in `ensub-sqlite` or binary crates.
- Keep scheduling and due-date calculations pure by accepting timestamps as arguments. Do not read the system clock inside domain logic.

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

For `ensub-wasm`, also run target check and Clippy for `wasm32-unknown-unknown`,
browser tests through `wasm-pack`, the static web build, and dependency-tree
inspection confirming that native SQLite and UI crates do not enter the WASM graph.
