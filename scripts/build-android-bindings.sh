#!/bin/sh

set -eu

workspace_root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
jni_output=${1:-"$workspace_root/android/app/build/generated/jniLibs"}
kotlin_output=${2:-"$workspace_root/android/app/build/generated/source/uniffi/kotlin"}

command -v cargo-ndk >/dev/null 2>&1 || {
  echo "cargo-ndk is required; install it with: cargo install cargo-ndk" >&2
  exit 1
}

if [ -z "${ANDROID_HOME:-}" ] && [ -z "${ANDROID_SDK_ROOT:-}" ]; then
  echo "ANDROID_HOME or ANDROID_SDK_ROOT must point to the Android SDK" >&2
  exit 1
fi

mkdir -p "$jni_output" "$kotlin_output"

cd "$workspace_root"
cargo ndk \
  --platform 26 \
  --target arm64-v8a \
  --target x86_64 \
  --output-dir "$jni_output" \
  build --locked -p ensub-uniffi

cargo run --locked -p ensub-uniffi-bindgen -- \
  generate \
  --library \
  --language kotlin \
  --no-format \
  --out-dir "$kotlin_output" \
  "$jni_output/arm64-v8a/libensub_uniffi.so"
