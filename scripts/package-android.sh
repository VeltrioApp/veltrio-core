#!/usr/bin/env bash
#
# Android artifact (phones and Android TV): one .so per ABI and the Kotlin
# bindings, zipped as dist/veltrio-core-android.zip:
#
#   jniLibs/<abi>/libveltrio_core.so   arm64-v8a, armeabi-v7a, x86_64
#   kotlin/uniffi/veltrio/veltrio.kt   (needs net.java.dev.jna:jna, aar)
#
# Needs the Android NDK (ANDROID_NDK_HOME or ANDROID_NDK_ROOT; GitHub's
# Ubuntu runners set the latter) and cargo-ndk.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/dist/android"
# Android 8.0. Keep in sync with the apps' minSdk.
API="${VELTRIO_ANDROID_API:-26}"

rm -rf "$OUT" && mkdir -p "$OUT/jniLibs" "$OUT/kotlin"
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android >/dev/null
echo "==> veltrio-core for Android (API $API)"
cargo ndk --platform "$API" -t arm64-v8a -t armeabi-v7a -t x86_64 -o "$OUT/jniLibs" \
  build --manifest-path "$ROOT/Cargo.toml" -p veltrio-core --release --locked
# The bindings come from a debug build for the host: the release .so files
# are stripped (profile.release), which takes UniFFI's metadata with it, and
# the bindgen then writes nothing.
cargo build --manifest-path "$ROOT/Cargo.toml" -p veltrio-core --locked
HOST_LIB=""
for lib in "$ROOT"/target/debug/libveltrio_core.{dylib,so}; do [ -f "$lib" ] && HOST_LIB="$lib" && break; done
"$ROOT/scripts/bindings.sh" kotlin "$HOST_LIB" "$OUT/kotlin"
test -s "$OUT/kotlin/uniffi/veltrio/veltrio.kt" || { echo "no Kotlin bindings generated" >&2; exit 1; }

(cd "$OUT" && rm -f ../veltrio-core-android.zip && zip -qry ../veltrio-core-android.zip .)
echo "==> dist/veltrio-core-android.zip"
