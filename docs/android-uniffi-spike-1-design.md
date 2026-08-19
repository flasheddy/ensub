# Android UniFFI Spike 1 Design

## Goal

Prove the complete local integration path from portable Rust through generated
UniFFI Kotlin bindings into a Jetpack Compose screen. The screen must play the
existing bundled demo MP3 with Media3, render the existing demo transcript, and
derive active-cue state from the Rust cue-selection policy as playback moves.

This is an integration spike, not the first production Android release. It
answers whether Ensub can reuse its Rust parsing and synchronization behavior
without making Android depend on the frozen WASM adapter.

## Scope

Spike 1 includes:

- a Rust 2021 `ensub-uniffi` facade over `core_engine` and `language_engine`;
- generated Kotlin bindings produced from UniFFI library metadata;
- Android `arm64-v8a` and `x86_64` native libraries;
- one Kotlin/Compose Android application module;
- in-activity Media3 playback of the existing synthetic demo MP3;
- transcript rendering and tap-to-seek;
- periodic active-cue synchronization through Rust; and
- Rust, Kotlin unit, and Android instrumentation contract tests.

Spike 1 excludes `MediaSessionService`, notifications, background playback,
lock-screen controls, production SQLite, migrations, downloads, RSS fetching,
lexicon lookup, capture, SRS review, and Play Store packaging. Those exclusions
are architectural boundaries, not incomplete tasks in this spike.

## Rust Facade

`bindings/ensub-uniffi` is a platform adapter. It depends inward on
`core_engine` and `language_engine`; neither portable crate depends on UniFFI,
Kotlin, JNI, Android, Media3, or an async runtime. It does not depend on or copy
DTOs from `crates/wasm_bridge`.

The facade exports one immutable UniFFI object:

```text
TranscriptSession.fromFixture(sourceUrl, fixtureBytes)
  -> episode(): EpisodeDto
  -> cues(): List<TranscriptCueDto>
  -> syncAt(positionMs): TranscriptSyncDto
```

The constructor calls `language_engine::parse_podcast_fixture` and retains the
resulting `TranscriptDocument`. Subsequent `syncAt` calls use
`TranscriptDocument::active_cue_indices`, so Kotlin never reimplements overlap,
end-boundary, or gap behavior. The object is immutable and safe to call from a
Kotlin-owned playback loop; no callback crosses the FFI boundary.

UniFFI-generated scaffolding is part of the binding crate. A small workspace
tool, `ensub-uniffi-bindgen`, exposes UniFFI's pinned binding-generator CLI so
developers and CI do not depend on a globally installed `uniffi-bindgen`.
The crate-local `uniffi.toml` enables Android cleaner generation so min-SDK
devices fall back to JNA rather than calling API-33 cleaner classes directly.

## FFI DTO Contract

All numeric DTO fields use signed 64-bit values. This maps naturally to Kotlin
`Long` and avoids exposing Kotlin unsigned APIs in application state.

```text
EpisodeDto
  feedTitle: String
  episodeTitle: String
  durationMs: Long

TranscriptCueDto
  index: Long
  id: String
  sourceCueId: String?
  startMs: Long
  endMs: Long
  text: String

TranscriptSyncDto
  activeCueIndices: List<Long>
  anchorCueIndex: Long?
  precedingCueIndex: Long?
```

`activeCueIndices` contains every overlapping cue in source order.
`anchorCueIndex` is the first active cue. During a transcript gap, the active
list and anchor are empty while `precedingCueIndex` identifies the most recent
completed cue. At a cue's `endMs`, that cue is no longer active, matching the
portable half-open interval contract.

The exported error type has stable categories for invalid fixtures, negative
playback positions, and numeric conversion overflow. Human-readable details
are carried for fixture failures, but Android presentation does not parse error
messages to make decisions.

## Android Project

The Android project lives outside the Cargo crate tree:

```text
android/
  settings.gradle.kts
  build.gradle.kts
  gradle.properties
  gradle/libs.versions.toml
  gradle/wrapper/
  app/
    build.gradle.kts
    src/main/AndroidManifest.xml
    src/main/kotlin/dev/ensub/android/
      MainActivity.kt
      player/DemoPlayerScreen.kt
      player/DemoPlayerViewModel.kt
      player/TranscriptEngine.kt
      ui/theme/EnsubTheme.kt
    src/test/kotlin/dev/ensub/android/player/
    src/androidTest/kotlin/dev/ensub/android/
```

Gradle uses the repository's `crates/web_player/assets` directory as a read-only
Android asset source. The fixture is loaded with `AssetManager`; Media3 plays
`asset:///demo.mp3`. The synthetic source URL passed to Rust uses the reserved
`.invalid` domain and is never fetched.

`DemoPlayerViewModel` owns the in-activity `ExoPlayer`, the UniFFI session, and a
short coroutine polling loop. It publishes immutable `PlayerUiState` through a
`StateFlow`. `DemoPlayerScreen` owns presentation only: playback controls,
position slider, episode metadata, cue list, active highlighting, and cue seek
commands. `onCleared` stops polling and releases the player.

`TranscriptEngine` is a small Kotlin interface around the generated session.
The production adapter delegates to UniFFI. Kotlin unit tests use a fake engine
to test state mapping without loading a native library; the instrumentation test
loads the real `.so` and proves fixture bytes cross the FFI boundary.

## Native Build Flow

`scripts/build-android-bindings.sh` is the single native build entry point. It:

1. builds `ensub-uniffi` with `cargo-ndk` for `arm64-v8a` and `x86_64`;
2. writes native libraries under Gradle's generated `jniLibs` directory; and
3. runs the workspace `ensub-uniffi-bindgen` tool against the arm64 library to
   write Kotlin sources under Gradle's generated source directory.

The Android `preBuild` task depends on this script. Generated Kotlin and native
libraries stay under `android/app/build` and are not committed. UniFFI and its
generator are pinned to the same workspace dependency version.

## Test Strategy

Rust facade tests use the committed demo fixture and prove:

- the fixture produces twelve stable DTOs and expected episode metadata;
- 29,500 ms reports both overlapping cues in source order;
- 40,000 ms reports the preceding cue during the intentional gap; and
- a negative position returns the typed playback-position error.

Kotlin unit tests prove that playback snapshots map to immutable UI state and
that cue seeks are forwarded as millisecond positions. Android instrumentation
loads the real fixture through `AssetManager`, constructs the generated
`TranscriptSession`, and asserts the same overlap contract through JNI.

The spike passes when the Rust facade tests, host workspace checks, Android unit
tests, Android lint/build, and instrumentation APK build pass. Running the
instrumentation test on an emulator is required when an emulator is available;
building the test APK is the minimum CI gate for initial scaffolding.

## Follow-Up Boundary

Spike 2 will move playback behind `MediaSessionService` and add notification,
background, audio-focus, headset, and lock-screen behavior. It must reuse the
same `TranscriptEngine` contract rather than widening the Rust facade for
Android lifecycle concerns. Storage and network ingestion remain deferred until
both integration spikes pass.
