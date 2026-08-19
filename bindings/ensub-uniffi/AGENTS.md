# Ensub UniFFI Facade Agent Guidelines

The repository-root `AGENTS.md` applies here. This crate is the supported Rust
boundary for native clients; it is an adapter, not a second domain engine.

## Dependency And Ownership Rules

- Depend inward on portable Rust engines and native storage adapters. Do not add
  Android, Kotlin, Compose, Media3, JNI, UI, HTTP-client, or async-runtime
  dependencies to this crate.
- Keep parsing, cue selection, time and range math, tokenization, capture,
  scheduling, and persistence policy in their owning Rust crates. The facade may
  validate, compose, and project those capabilities into mobile DTOs.
- Preserve `#![forbid(unsafe_code)]`. Do not add handwritten JNI when UniFFI can
  express the contract.
- Prefer coarse-grained session or repository objects over chatty calls and
  high-frequency callbacks across FFI.

## Exported Contract

- Export owned, versionable UniFFI records using mobile-safe `String`, `Vec<T>`,
  integer, enum, and optional fields. Do not expose borrowed data, lifetimes,
  Rust paths, SQL, driver types, or serialization layouts.
- Validate signed/unsigned and width conversions at the facade boundary. Return
  a typed numeric error rather than truncating or wrapping time values, offsets,
  counters, or indices.
- Use UTF-16 code-unit offsets for DTO fields that Kotlin applies to a `String`.
  Keep portable UTF-8 byte offsets internal to Rust.
- Treat exported DTO, enum, object, method, and error changes as native-client
  compatibility changes. Update Rust and generated-Kotlin contract tests
  together.
- Generated Kotlin and native libraries are build outputs. Do not commit files
  produced under `android/app/build`.

## Errors And Privacy

- Follow Appendix A of `.private/prd.md` for `MobileErrorCategory`, generated
  `MobileOperation`, `RetryAdvice`, precedence, and conflict mapping as the
  production facade expands beyond the Spike 1 errors.
- Kotlin control flow must never require parsing an error message. Map core
  `ReviewUpdate::Conflict` to the stable mobile conflict category.
- Do not send Rust source chains, SQL, database paths, raw private URLs,
  transcript or capture text, credentials, response bodies, or database values
  across UniFFI error detail.
- Do not use `panic!`, `unwrap`, or `expect` in exported production paths.
  Return typed, actionable errors. Tests may use `expect` for synthetic fixture
  setup.

## Testing And Validation

- Add focused Rust tests for every DTO projection, validation rule, error
  mapping, and portable behavior exposed by the facade.
- Add generated-Kotlin contract coverage when Kotlin-visible names, types,
  errors, or object lifecycle change.
- For changes confined to this facade, run from the repository root:

  1. `cargo fmt --all -- --check`
  2. `cargo test -p ensub-uniffi`
  3. `cargo clippy -p ensub-uniffi --all-targets -- -D warnings`
  4. `sh scripts/verify.sh android`
  5. `sh scripts/verify.sh secrets`
  6. `git diff --check`

- Run the full workspace and applicable PWA/WASM regression gates when a facade
  change also modifies shared-core behavior.
