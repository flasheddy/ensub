# Ensub Android Agent Guidelines

The repository-root `AGENTS.md` applies here. This file adds rules for the
Kotlin, Compose, Media3, and Android build surface.

## Milestone Scope

- The checked-in application is the delivered Spike 1 baseline: it uses the
  bundled synthetic fixture, a `DemoPlayerViewModel`-owned `ExoPlayer`, and
  UniFFI cue synchronization. Treat that player as migration code; do not
  expand its ownership or create another Activity- or ViewModel-owned player.
- Spike 2 is the active target. New playback work must converge on the service-
  owned architecture in `.private/prd.md`; do not describe an unimplemented
  service behavior as delivered.
- Do not pull Spike 3 production SQLite, network ingestion, media caching, or
  Spike 4 review features into a Spike 2 change unless the task explicitly
  includes them.
- `android/README.md` documents the runnable Spike 1 baseline. Update it when
  implementation status or build commands actually change.

## Ownership Boundaries

- `dev.ensub.android.player.EnsubMediaSessionService` is the Spike 2 owner of the
  sole authoritative Media3 player, media session, current item, position, and
  queue. Activity, Compose, and ViewModel code connect through
  `MediaController`; they must not create a competing player.
- Media3 is the sole audio-focus owner. Do not add a second
  `AudioManager.requestAudioFocus` path, focus listener, or app-owned duck-volume
  multiplier.
- Kotlin owns Android lifecycle, system media controls, notifications,
  permissions, route events, HTTP transport, connectivity, and disposable
  Media3 caching.
- Map generated UniFFI DTOs into immutable application-owned Kotlin state. Do
  not expose Rust implementation details to Compose or parse error text for
  control flow.
- Kotlin must not parse RSS, WebVTT, or SRT; tokenize transcripts; select active
  cues; calculate audio ranges; implement SM-2; issue SQL; or own a schema
  version. Add or extend a typed UniFFI operation instead.
- Rust-produced string offsets exposed to Kotlin must use the documented mobile
  DTO convention. Do not expose UTF-8 byte offsets as Compose string indices.

## Playback And Synchronization

- Render immutable state derived from the authoritative service/controller
  snapshot. Activity recreation or controller reconnection must not imply a
  user pause, reset position, or create another player.
- Pass Media3 integer-millisecond positions to the UniFFI transcript session.
  Synchronize immediately after attach, seek, discontinuity, and item change.
- Run periodic cue observation only while transcript UI is visible. Do not keep
  a high-frequency Activity or service polling loop alive in the background.
- Preserve Rust overlap, gap, anchor, and preceding-cue semantics. Do not add a
  Kotlin fallback algorithm for cue selection.
- Keep Media3 1.7.1 audio-focus behavior aligned with the active PRD matrices.
  A Media3 version change that alters focus or session behavior requires updated
  contract tests and documentation.

## Platform And Privacy

- The merged manifest must declare `android.permission.FOREGROUND_SERVICE`,
  `android.permission.FOREGROUND_SERVICE_MEDIA_PLAYBACK`, and
  `android.permission.POST_NOTIFICATIONS`. Register
  `dev.ensub.android.player.EnsubMediaSessionService` with
  `android:foregroundServiceType="mediaPlayback"`, `android:exported="true"`,
  and the `androidx.media3.session.MediaSessionService` intent action.
- Accept standard transport commands only from the same application or trusted
  controllers. App-specific custom commands are same-UID only; reject all
  other controllers without exposing paths, URLs, transcript data, or storage
  calls.
- A denied `POST_NOTIFICATIONS` permission must not crash, loop, or gate service-
  owned playback.
- Store durable learning state only through the Rust-owned storage facade. Do
  not add Room, direct SQLite, or preferences as a second persistence model.
  Spike 2 proves the PRD progress cadence through its injectable sink; Spike 3
  supplies the production Rust-owned SQLite sink through UniFFI.
- Preserve `android:allowBackup="false"`. When durable Android storage lands,
  add and test the PRD's API 31+ data-extraction and legacy backup exclusions.
- Store disposable Media3 bytes only under
  `context.cacheDir/ensub-media-v1`. Do not place media cache data in
  `filesDir`, external storage, or another durable or backup domain.
- Do not log private URLs, transcript or capture text, database paths, UniFFI
  source chains, credentials, or production/personal data. Use synthetic data in
  tests and screenshots.

## Build Outputs And Dependencies

- Generated Kotlin bindings and native `.so` files stay under
  `android/app/build`; do not commit them.
- APKs, test APKs, Gradle caches, `local.properties`, and generated reports are
  build outputs and remain untracked.
- Keep SDK paths in `ANDROID_HOME`, `ANDROID_SDK_ROOT`, or local untracked
  configuration. Never commit a machine-specific absolute path.
- Keep the shared demo assets read-only under `crates/web_player/assets`; do not
  copy or fork the frozen fixture into the Android source tree.
- Use the version catalog and existing Gradle conventions. Add a dependency
  only when the platform implementation requires it and keep versions pinned.
- Keep blocking I/O, native parsing, and database work off the Compose main
  thread. Do not use unscoped background work such as `GlobalScope`.

## Validation

From the repository root, run:

1. `cargo fmt --all -- --check`
2. `sh scripts/verify.sh android`
3. `sh scripts/verify.sh secrets`
4. `git diff --check`

`scripts/verify.sh android` runs the `ensub-uniffi` Rust tests and Clippy,
generates bindings for `arm64-v8a` and `x86_64`, runs JVM tests and lint, and
builds the application and instrumentation APKs.

Run `./gradlew :app:connectedDebugAndroidTest` from `android/` for changes that
depend on JNI loading, manifest integration, a media service, permissions, or
Android lifecycle behavior. Prefer pure JVM tests for state mappers, fake-clock
policies, and command routing. Spike 2 acceptance requires connected coverage
on API 26 and API 37. Record notification, lock-screen, incoming-call, wired-
headset, Bluetooth, and detached-UI behavior as physical-device evidence; do
not represent build-only or emulator coverage as physical verification.
