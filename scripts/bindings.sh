#!/usr/bin/env bash
#
# Generate UniFFI bindings from a built library.
#   scripts/bindings.sh <swift|kotlin> <path to libveltrio_core.{dylib,so,a}> <out dir>
#
# UniFFI's library mode reads the exported metadata from the binary, so any
# target's build works (an Android .so for Kotlin, a macOS dylib for Swift).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
LANGUAGE="$1"; LIB="$2"; OUT="$3"
mkdir -p "$OUT"
# Run from the workspace: the bindgen asks `cargo metadata` for crate info.
cd "$ROOT"
cargo run --quiet --locked -p veltrio-core --bin uniffi-bindgen -- \
  generate --library "$LIB" --language "$LANGUAGE" --out-dir "$OUT"
