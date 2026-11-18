#!/usr/bin/env bash
#
# Windows artifact: the x64 DLL, its import library and symbols, zipped as
# dist/veltrio-core-windows-x64.zip:
#
#   veltrio_core.dll  veltrio_core.dll.lib  veltrio_core.pdb
#
# No bindings yet: UniFFI has no first-party C# generator. The Windows app
# will pick one (uniffi-bindgen-cs) once its stack is decided.
#
# Runs in Git Bash on a Windows runner (MSVC toolchain).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/dist/windows"
TARGET="x86_64-pc-windows-msvc"

rm -rf "$OUT" && mkdir -p "$OUT"
rustup target add "$TARGET" >/dev/null
echo "==> veltrio-core for $TARGET"
cargo build --manifest-path "$ROOT/Cargo.toml" -p veltrio-core --release --locked --target "$TARGET"
for f in veltrio_core.dll veltrio_core.dll.lib veltrio_core.pdb; do
  [ -f "$ROOT/target/$TARGET/release/$f" ] && cp "$ROOT/target/$TARGET/release/$f" "$OUT/"
done
(cd "$OUT" && rm -f ../veltrio-core-windows-x64.zip && 7z a -tzip -bso0 ../veltrio-core-windows-x64.zip . )
echo "==> dist/veltrio-core-windows-x64.zip"
