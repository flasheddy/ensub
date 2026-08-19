# Platform Status

This document is the canonical lifecycle policy for Ensub client surfaces
after the v0.2.0 release milestone.

| Component | Lifecycle | Product role |
|---|---|---|
| `crates/web_player` | Feature complete and frozen at v0.2.0 | Installable PWA and reference prototype for the interactive podcast workflow |
| `crates/wasm_bridge` (`ensub-wasm`) | Maintenance and regression only | Browser adapter and regression boundary for the portable Rust engines |
| `core_engine` and `language_engine` | Shared, actively evolved core | Portable domain policy, parsing, cue selection, capture, and language contracts used across clients |
| Native Android client | Active client milestone | Kotlin / Jetpack Compose application using Media3 and UniFFI-backed Rust capabilities |

## Frozen PWA and WASM Scope

The v0.2.0 PWA/Web Player is a reference implementation, not the active target
for new client functionality. `crates/web_player` and `crates/wasm_bridge` will
not receive new client features, new browser-only product workflows, or new
browser presentation capabilities that do not protect the v0.2.0 baseline.

Maintenance work remains allowed when it is necessary to preserve that
baseline:

- security fixes;
- critical correctness and compatibility fixes;
- dependency and toolchain maintenance;
- regression fixes required by shared Rust-core refactoring; and
- test-fixture or test-harness maintenance that preserves established behavior.

The frozen designation does not mean that the PWA is abandoned. It remains a
buildable and testable reference for observable client behavior and for the
browser boundary around the shared Rust core.

## Regression Anchor Policy

All existing WASM, PWA unit, build, distribution, storage-migration, and browser
tests remain required regression anchors. Shared-core changes must continue to
run the applicable `ensub-wasm`, `web_player`, and offline-sandbox checks.

Existing coverage must not be deleted, weakened, skipped, or replaced merely
because native Android is the active client track. A test may change only when
an intentional shared contract change makes its former assertion invalid; that
change must preserve equivalent regression coverage and document the contract
change. The v0.2.0 release audit remains the baseline evidence for the frozen
surface: [v0.2.0 Release Audit](release-v0.2.0-audit.md).

## Active Native Android Track

The active client milestone is a native Android application built with Kotlin
and Jetpack Compose. Media3 owns Android playback, audio focus, background
service behavior, and system media controls. UniFFI exposes suitable portable
Rust capabilities from `core_engine` and `language_engine`; Android-specific
UI, lifecycle, media, and persistence integration stays in the Android client.

The Android client is a parallel platform adapter over the shared Rust core.
It does not replace or remove the frozen PWA, its WASM adapter, or their tests.
