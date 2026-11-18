#!/usr/bin/env bash
#
# Windows artifact: the x64 DLL, its import library and symbols, and the C#
# bindings, zipped as dist/veltrio-core-windows-x64.zip:
#
#   veltrio_core.dll  veltrio_core.dll.lib  veltrio_core.pdb
#   Bindings/veltrio.cs        (namespace uniffi.veltrio, internal types:
#                               compiled into the app's own assembly)
#
# UniFFI has no first-party C# generator; NordSecurity's uniffi-bindgen-cs
# is used, at the release matching the core's UniFFI 0.29:
#   cargo install uniffi-bindgen-cs --git https://github.com/NordSecurity/uniffi-bindgen-cs \
#     --tag v0.10.0+v0.29.4 --locked
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
echo "==> C# bindings"
uniffi-bindgen-cs --library "$ROOT/target/$TARGET/release/veltrio_core.dll" --out-dir "$OUT/Bindings"
# The core has a type named Stream (an addon stream). In the generated
# namespace it hides System.IO.Stream, which the bindings' own byte-buffer
# helpers use unqualified, and the file stops compiling. Qualify those uses.
CS="$OUT/Bindings/veltrio.cs"
sed -i.bak -E \
  -e 's/\(this Stream stream/(this System.IO.Stream stream/' \
  -e 's/^( *)Stream stream;/\1System.IO.Stream stream;/' \
  -e 's/BigEndianStream\(Stream stream\)/BigEndianStream(System.IO.Stream stream)/' \
  "$CS" && rm -f "$CS.bak"
if grep -nE '\(this Stream |^ *Stream stream;|BigEndianStream\(Stream ' "$CS"; then
  echo "error: unqualified System.IO.Stream left in the C# bindings (generator changed?)" >&2
  exit 1
fi
(cd "$OUT" && rm -f ../veltrio-core-windows-x64.zip && 7z a -tzip -bso0 ../veltrio-core-windows-x64.zip . )
echo "==> dist/veltrio-core-windows-x64.zip"
