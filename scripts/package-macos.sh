#!/usr/bin/env bash
#
# macOS artifact: a universal (arm64 + x86_64) dylib and its Swift
# bindings, zipped as dist/veltrio-core-macos.zip:
#
#   Lib/libveltrio_core.dylib      install name @rpath/libveltrio_core.dylib
#   Bindings/veltrio.swift
#   Bindings/veltrioFFI.h
#   Bindings/module.modulemap
#
# A dylib rather than a static library: the macOS app links MPVKit, whose
# Libdovi is itself a Rust staticlib carrying its own Rust std, and two of
# those in one binary collide at link time.
#
# VELTRIO_ARCHS="arm64" builds a single slice for quicker local runs.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/dist/macos"
ARCHS="${VELTRIO_ARCHS:-arm64 x86_64}"
# Keep in sync with the macOS app's deployment target.
export MACOSX_DEPLOYMENT_TARGET="14.0"

rm -rf "$OUT" && mkdir -p "$OUT/Lib" "$OUT/Bindings"
DYLIBS=()
for arch in $ARCHS; do
  case "$arch" in
    arm64)  target="aarch64-apple-darwin" ;;
    x86_64) target="x86_64-apple-darwin" ;;
    *) echo "error: unknown arch $arch" >&2; exit 1 ;;
  esac
  echo "==> veltrio-core for $target"
  rustup target add "$target" >/dev/null
  cargo build --manifest-path "$ROOT/Cargo.toml" -p veltrio-core --release --locked --target "$target"
  DYLIBS+=("$ROOT/target/$target/release/libveltrio_core.dylib")
done

lipo -create "${DYLIBS[@]}" -output "$OUT/Lib/libveltrio_core.dylib"
install_name_tool -id "@rpath/libveltrio_core.dylib" "$OUT/Lib/libveltrio_core.dylib"
"$ROOT/scripts/bindings.sh" swift "${DYLIBS[0]}" "$OUT/Bindings"
mv "$OUT/Bindings/veltrioFFI.modulemap" "$OUT/Bindings/module.modulemap"

(cd "$OUT" && rm -f ../veltrio-core-macos.zip && zip -qry ../veltrio-core-macos.zip .)
echo "==> dist/veltrio-core-macos.zip ($(lipo -archs "$OUT/Lib/libveltrio_core.dylib"))"
