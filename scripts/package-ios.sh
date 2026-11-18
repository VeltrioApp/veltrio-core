#!/usr/bin/env bash
#
# iOS / tvOS artifact: veltrioFFI.xcframework (dynamic frameworks) plus the
# Swift bindings, zipped as dist/veltrio-core-apple-mobile.zip:
#
#   veltrioFFI.xcframework/        ios-arm64, ios-arm64_x86_64-simulator
#                                  (+ tvos-arm64, tvos-arm64_x86_64-simulator
#                                   when VELTRIO_TVOS=1)
#   Bindings/veltrio.swift
#
# The framework is named after the bindings' C module (veltrioFFI) so
# `import veltrioFFI` in veltrio.swift resolves to it with no extra map.
#
# tvOS is a tier-3 Rust target: it needs a nightly toolchain with the
# rust-src component (std is built from source with -Zbuild-std).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/dist/apple-mobile"
STAGE="$ROOT/target/apple-frameworks"
export IPHONEOS_DEPLOYMENT_TARGET="16.0"
export TVOS_DEPLOYMENT_TARGET="16.0"

rm -rf "$OUT" "$STAGE" && mkdir -p "$OUT/Bindings" "$STAGE"

build() { # <target> [nightly]
  echo "==> veltrio-core for $1"
  if [ "${2:-}" = nightly ]; then
    cargo +nightly build -Zbuild-std=std,panic_abort --manifest-path "$ROOT/Cargo.toml" \
      -p veltrio-core --release --locked --target "$1"
  else
    rustup target add "$1" >/dev/null
    cargo build --manifest-path "$ROOT/Cargo.toml" -p veltrio-core --release --locked --target "$1"
  fi
}

# <name> <platform: iPhoneOS|iPhoneSimulator|AppleTVOS|AppleTVSimulator> <min os> <dylib...>
framework() {
  local dir="$STAGE/$1/veltrioFFI.framework"; local platform="$2"; local minos="$3"; shift 3
  mkdir -p "$dir/Headers" "$dir/Modules"
  lipo -create "$@" -output "$dir/veltrioFFI"
  install_name_tool -id "@rpath/veltrioFFI.framework/veltrioFFI" "$dir/veltrioFFI"
  cp "$OUT/Bindings/veltrioFFI.h" "$dir/Headers/"
  cat > "$dir/Modules/module.modulemap" <<MAP
framework module veltrioFFI {
    umbrella header "veltrioFFI.h"
    export *
}
MAP
  cat > "$dir/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleExecutable</key><string>veltrioFFI</string>
  <key>CFBundleIdentifier</key><string>app.veltrio.core</string>
  <key>CFBundleName</key><string>veltrioFFI</string>
  <key>CFBundlePackageType</key><string>FMWK</string>
  <key>CFBundleShortVersionString</key><string>$(awk -F'"' '/^version/ {print $2; exit}' "$ROOT/Cargo.toml")</string>
  <key>CFBundleVersion</key><string>1</string>
  <key>CFBundleSupportedPlatforms</key><array><string>$platform</string></array>
  <key>MinimumOSVersion</key><string>$minos</string>
</dict></plist>
PLIST
}

T="$ROOT/target"; LIB="release/libveltrio_core.dylib"
build aarch64-apple-ios
build aarch64-apple-ios-sim
build x86_64-apple-ios
"$ROOT/scripts/bindings.sh" swift "$T/aarch64-apple-ios/$LIB" "$OUT/Bindings"
rm -f "$OUT/Bindings/veltrioFFI.modulemap"
framework ios iPhoneOS 16.0 "$T/aarch64-apple-ios/$LIB"
framework ios-sim iPhoneSimulator 16.0 "$T/aarch64-apple-ios-sim/$LIB" "$T/x86_64-apple-ios/$LIB"
ARGS=(-framework "$STAGE/ios/veltrioFFI.framework" -framework "$STAGE/ios-sim/veltrioFFI.framework")

if [ "${VELTRIO_TVOS:-0}" = 1 ]; then
  build aarch64-apple-tvos nightly
  build aarch64-apple-tvos-sim nightly
  build x86_64-apple-tvos nightly
  framework tvos AppleTVOS 16.0 "$T/aarch64-apple-tvos/$LIB"
  framework tvos-sim AppleTVSimulator 16.0 "$T/aarch64-apple-tvos-sim/$LIB" "$T/x86_64-apple-tvos/$LIB"
  ARGS+=(-framework "$STAGE/tvos/veltrioFFI.framework" -framework "$STAGE/tvos-sim/veltrioFFI.framework")
fi

xcodebuild -create-xcframework "${ARGS[@]}" -output "$OUT/veltrioFFI.xcframework" >/dev/null
rm -f "$OUT/Bindings/veltrioFFI.h"
(cd "$OUT" && rm -f ../veltrio-core-apple-mobile.zip && zip -qry ../veltrio-core-apple-mobile.zip .)
echo "==> dist/veltrio-core-apple-mobile.zip"
