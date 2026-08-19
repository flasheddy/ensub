# v0.2.0 Release Audit

This document records the Milestone 5.6 evidence used before creating the
`v0.2.0` tag. All fixtures are synthetic. The audit does not authorize or
perform a tag, push, upload, deployment, or hosted release.

## Acceptance Criteria

| PRD 13 | Status | Automated evidence |
|---|---|---|
| 1. Core v0.1 baseline documented | Pass | Root README and changelog record stable v0.1.0 and verified `v0.1.0-rc1` baseline. |
| 2. CORS and no-CORS feeds | Pass | `player.spec.mjs` uses `fixture-server.mjs` for a real cross-origin allowed feed and a blocked response; the visible status names CORS only as a possible cause. |
| 3. VTT/SRT equivalence and typed failures | Pass | `language_engine/tests/transcript.rs`, `language_engine/tests/portable_ingestion.rs`, and `wasm_bridge/tests/ingestion.rs`. |
| 4. Cue synchronization and follow behavior | Pass | `core_engine/tests/media.rs`, `wasm_bridge/tests/player.rs`, `audio-host.test.mjs`, and Player browser seek/follow/scroll tests cover overlap, gap, seek, rate events, manual follow, stable scrolling, and active rows. |
| 5. Offline token lookup | Pass | Player offline lookup tests use Rust UTF-16 spans and the bundled compressed lexicon. |
| 6. Structured capture and identity reconciliation | Pass | `wasm_bridge/tests/player.rs`, `wasm_bridge/tests/learning.rs`, and the no-GUID-to-GUID cross-origin browser test preserve the episode row and idempotent capture. |
| 7. Audio slice rules | Pass | Focused `core_engine` media tests cover padding, zero clamp, duration clamp, multi-cue bounds, overflow, and invalid ranges. |
| 8. v0.1 migration | Pass | The committed `browser-snapshot-v1.json` golden input is exercised by native storage tests and the Firefox WASM migration test; failed writes preserve exact original bytes. |
| 9. Explicit contextual disambiguation | Pass | Controller, adapter, Rust validation, browser consent/payload, zero-automatic-call, and provider-failure tests. |
| 10. In-player review | Pass | Rust learning/storage tests plus Player review controller and browser prompt/replay/rating/advance tests. |
| 11. Missing audio fallback | Pass | `audio-host.test.mjs`, `review-controller.test.mjs`, and cross-origin missing-audio fixture retain text and allow rating. |
| 12. Complete offline PWA loop | Pass | One Player browser test installs the service worker, reloads offline, opens the cached transcript, performs local lookup and capture, and completes an SRS rating. |
| 13. WASM dependency boundary | Pass | Target-specific `cargo tree` plus `scripts/verify.sh wasm` reject native SQLite/UI, HTTP, async runtime, and LLM crates. |
| 14. Scope boundary | Pass | Workspace/release artifacts add no mobile, cloud-sync database, desktop GUI/applet feature, or browser extension deliverable. |

## Section 15 Release Gates

| Release gate | Status | Enforcement and coverage |
|---|---|---|
| All PRD acceptance criteria pass | Pass | Criteria 1-14 are mapped above to committed automated coverage and documentation. |
| No placeholders, mock dictionary/persistence, or production panic path | Pass | `scripts/verify.sh hardening` scans production files and runs compiler-backed `clippy::panic`, `unwrap_used`, and `expect_used` lints without test targets. |
| Rust format/check/Clippy/test and portable trees pass | Pass | Canonical six-command workspace suite; `core_engine` and `language_engine` trees are inspected independently. |
| WASM target, Firefox, static PWA, browser E2E, and WASM tree pass | Pass | Target check/Clippy, Firefox `wasm-pack`, Player build/`verify:dist`, Chromium Playwright, and prohibited dependency scan. |
| Migration and rollback/recovery documented and tested | Pass | Golden v0.1 migration, corrupt/newer snapshot preservation, failed-write exact bytes, IndexedDB transaction-completion tests, and Local data export/reset recovery UI. |
| Transport and offline limits documented | Pass | Player README, Getting Started, User Guide, and Data and Privacy cover direct CORS-bound fetches, cache limits, and offline boundaries. |
| Privacy documentation is complete | Pass | Data and Privacy covers local podcast metadata, full cached transcripts, logical audio references, exact export/reset scope, and optional endpoint disclosure/minimal payload. |
| Secret scanner passes across source, changes, fixtures, and generated Player assets | Pass | `scripts/verify.sh secrets`, `verify:dist`, hardening distribution scan, synthetic-only fixtures, and zero-provider-call browser test. |

## Tag Preconditions

1. Run the maintained seven-mode v0.2.0 verification sequence from a clean
   local branch with `scripts/verify.sh`: `rust`, `wasm`, `web`,
   `release-smoke`, `hardening`, `secrets`, and `release`. Retain the command
   results with this audit.
2. Run `sh scripts/verify.sh hardening` and review the complete working-tree
   diff, including generated version metadata changes.
3. Confirm no unapproved remote operation is included. Tagging, pushing, and
   publishing require separate explicit authorization.
