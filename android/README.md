# Ensub Android Spike 1

This project proves that an Android client can reuse Ensub's portable Rust
parsing and cue-selection policy through UniFFI. It plays the committed
synthetic demo with in-activity Media3 and renders a Compose transcript whose
active cues come from Rust.

The spike does not include background playback, `MediaSessionService`, lock
screen controls, production storage, downloads, or network feed ingestion.

## Prerequisites

- JDK 17
- Android SDK with API 37.0 and build tools
- Android NDK
- Rust 1.93 or newer
- `aarch64-linux-android` and `x86_64-linux-android` Rust targets
- `cargo-ndk`

Configure the SDK and Rust targets:

```bash
export ANDROID_HOME=/path/to/Android/Sdk
rustup target add aarch64-linux-android x86_64-linux-android
cargo install cargo-ndk
```

## Build And Test

From the repository root, run the complete opt-in gate:

```bash
sh scripts/verify.sh android
```

Or run individual Gradle tasks:

```bash
cd android
./gradlew :app:testDebugUnitTest
./gradlew :app:lintDebug
./gradlew :app:assembleDebug :app:assembleDebugAndroidTest
```

`preBuild` calls `scripts/build-android-bindings.sh`. The script builds
`ensub-uniffi` for `arm64-v8a` and `x86_64`, then generates Kotlin from the
arm64 library metadata. All generated files stay under `android/app/build`.

The application and test APKs are written to:

```text
android/app/build/outputs/apk/debug/app-debug.apk
android/app/build/outputs/apk/androidTest/debug/app-debug-androidTest.apk
```

With an emulator or device attached, execute the JNI contract test:

```bash
cd android
./gradlew :app:connectedDebugAndroidTest
```

The Android module reads `demo-fixture.json` and `demo.mp3` directly from
`crates/web_player/assets`; it does not copy or fork the frozen PWA fixture.
