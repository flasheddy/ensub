# Android UniFFI Spike 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a minimal Android application that parses the bundled Ensub demo fixture through Rust/UniFFI, plays its audio with in-activity Media3, and synchronizes Compose transcript state through Rust cue selection.

**Architecture:** Add a thin immutable UniFFI facade directly over `core_engine` and `language_engine`, generate Kotlin and two Android native libraries into Gradle build directories, and isolate generated bindings behind a Kotlin `TranscriptEngine`. Keep Android lifecycle and playback in Kotlin while Rust owns fixture parsing and cue policy.

**Tech Stack:** Rust 2021, UniFFI 0.32, cargo-ndk, Kotlin 2.2.21, Jetpack Compose BOM 2025.05.01, Media3 1.7.1, Android Gradle Plugin 8.13.2, Gradle 8.13, JDK 17.

---

### Task 1: UniFFI Facade Contract

**Files:**
- Modify: `Cargo.toml`
- Create: `bindings/ensub-uniffi/Cargo.toml`
- Create: `bindings/ensub-uniffi/src/lib.rs`
- Create: `bindings/ensub-uniffi/tests/facade.rs`
- Create: `tools/uniffi_bindgen/Cargo.toml`
- Create: `tools/uniffi_bindgen/src/main.rs`

- [ ] Add the binding and generator packages as workspace members and pin `uniffi = "0.32.0"` in workspace dependencies.
- [ ] Write facade tests against `crates/web_player/assets/demo-fixture.json` for twelve cues, 29,500 ms overlap, 40,000 ms gap, and negative-position rejection.
- [ ] Run `cargo test -p ensub-uniffi --test facade` and confirm compilation fails because the exported DTOs and session do not exist.
- [ ] Implement `EpisodeDto`, `TranscriptCueDto`, `TranscriptSyncDto`, `BindingError`, and immutable `TranscriptSession` using `parse_podcast_fixture` and `active_cue_indices`.
- [ ] Add `uniffi::setup_scaffolding!()` and a generator executable that calls `uniffi::uniffi_bindgen_main()`.
- [ ] Rerun the focused test and confirm all four contracts pass.

### Task 2: Reproducible Android Native Build

**Files:**
- Create: `scripts/build-android-bindings.sh`
- Modify: `.gitignore`

- [ ] Write the script to require `cargo-ndk`, build `ensub-uniffi` for `arm64-v8a` and `x86_64`, and generate Kotlin from the arm64 `.so` into caller-supplied build directories.
- [ ] Add only generated Android build outputs to `.gitignore`; keep source and Gradle wrapper files trackable.
- [ ] Run the script into a temporary directory and confirm both ABI libraries and one generated Kotlin binding exist.
- [ ] Run a symbol inspection and confirm the generated library exports UniFFI symbols for the `ensub_uniffi` namespace.

### Task 3: Android Gradle And Application Shell

**Files:**
- Create: `android/settings.gradle.kts`
- Create: `android/build.gradle.kts`
- Create: `android/gradle.properties`
- Create: `android/gradle/libs.versions.toml`
- Create: `android/gradlew`
- Create: `android/gradlew.bat`
- Create: `android/gradle/wrapper/gradle-wrapper.jar`
- Create: `android/gradle/wrapper/gradle-wrapper.properties`
- Create: `android/app/build.gradle.kts`
- Create: `android/app/src/main/AndroidManifest.xml`

- [ ] Configure Java 17, compile SDK 37.0, minimum SDK 26, Compose, Media3, coroutines, lifecycle, test dependencies, generated UniFFI Kotlin sources, generated `jniLibs`, and the shared PWA asset directory.
- [ ] Register `buildRustBindings` and make Android `preBuild` depend on it.
- [ ] Generate a Gradle 8.13 wrapper and run `./gradlew help` to prove plugin resolution and project configuration.

### Task 4: Kotlin Engine Boundary

**Files:**
- Create: `android/app/src/main/kotlin/dev/ensub/android/player/TranscriptEngine.kt`
- Create: `android/app/src/test/kotlin/dev/ensub/android/player/TranscriptEngineTest.kt`
- Create: `android/app/src/androidTest/kotlin/dev/ensub/android/NativeBindingInstrumentedTest.kt`

- [ ] Write a failing Kotlin unit test for conversion from generated-style transcript records and sync results into application models without unsigned fields.
- [ ] Run `./gradlew :app:testDebugUnitTest` and confirm the application engine adapter/model layer is missing.
- [ ] Implement application-owned episode, cue, and sync models plus `UniFfiTranscriptEngine`, delegating fixture parsing and synchronization to the generated `TranscriptSession`.
- [ ] Add an instrumentation test that loads `demo-fixture.json`, calls the real generated binding, and asserts twelve cues plus active indices `[2, 3]` at 29,500 ms.
- [ ] Rerun JVM unit tests to green and build the instrumentation APK.

### Task 5: In-Activity Media3 State

**Files:**
- Create: `android/app/src/main/kotlin/dev/ensub/android/player/DemoPlayerViewModel.kt`
- Create: `android/app/src/test/kotlin/dev/ensub/android/player/PlaybackStateMapperTest.kt`

- [ ] Write failing tests proving an engine sync snapshot maps overlapping active cues, gap context, duration, position, and playing state into immutable `PlayerUiState`.
- [ ] Run the focused Kotlin test and confirm the mapper is missing.
- [ ] Implement the pure state mapper, then implement `DemoPlayerViewModel` with `ExoPlayer`, `asset:///demo.mp3`, a 100 ms in-activity polling coroutine, play/pause, seek, cue seek, error state, and deterministic release in `onCleared`.
- [ ] Rerun Kotlin unit tests and confirm the state mapping contracts pass.

### Task 6: Compose Player Screen

**Files:**
- Create: `android/app/src/main/kotlin/dev/ensub/android/MainActivity.kt`
- Create: `android/app/src/main/kotlin/dev/ensub/android/player/DemoPlayerScreen.kt`
- Create: `android/app/src/main/kotlin/dev/ensub/android/ui/theme/EnsubTheme.kt`

- [ ] Implement a compact Material 3 screen with episode metadata, stable play/pause control, elapsed/duration labels, seek slider, loading/error states, and a keyed transcript list.
- [ ] Highlight every active overlapping cue, retain a restrained preceding-cue state during gaps, and make each cue seekable through the view model.
- [ ] Use lifecycle-aware state collection and keep Media3 and generated binding types out of composables.
- [ ] Run Compose compilation, Android lint, unit tests, and debug APK assembly.

### Task 7: Spike Verification And Documentation

**Files:**
- Create: `android/README.md`
- Modify: `docs/development.md`
- Modify: `scripts/verify.sh`

- [ ] Document JDK, SDK, NDK, Rust targets, `cargo-ndk`, native generation, unit tests, instrumentation build/run, and debug install commands.
- [ ] Add an opt-in Android verification mode rather than adding NDK work to the default repository verification path.
- [ ] Run `cargo fmt --all -- --check`, focused facade tests, `cargo check --workspace`, and `cargo clippy -p ensub-uniffi --all-targets -- -D warnings`.
- [ ] Run Android unit tests, lint, debug APK assembly, and instrumentation APK assembly; run connected tests if an emulator is available.
- [ ] Run the frozen WASM/PWA regression commands only if shared core behavior changed; otherwise confirm no frozen-surface file changed.
- [ ] Run `scripts/verify.sh secrets`, `git diff --check`, and inspect the complete final diff.
