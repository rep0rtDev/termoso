#!/usr/bin/env bash
# Build the Rust core (`termoso-mobile`) for iOS and package it as the
# `TermosoCore` Swift package: static libraries → `TermosoCoreFFI.xcframework`
# plus the UniFFI-generated `TermosoCore.swift`.
#
#   ./build-core.sh                      # debug, simulator (arm64) only
#   ./build-core.sh --release --device   # release, device + simulator
#   ./build-core.sh --release --device --no-sim
#
# Requires: Xcode command line tools, rustup targets `aarch64-apple-ios` and/or
# `aarch64-apple-ios-sim` (the script adds the missing ones).
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/../.." && pwd)"
pkg="$here/TermosoCore"

profile=debug
device=0
sim=1
for arg in "$@"; do
  case "$arg" in
    --release) profile=release ;;
    --debug) profile=debug ;;
    --device) device=1 ;;
    --no-device) device=0 ;;
    --sim) sim=1 ;;
    --no-sim) sim=0 ;;
    *) echo "unknown argument: $arg" >&2; exit 2 ;;
  esac
done
if [[ $device -eq 0 && $sim -eq 0 ]]; then
  echo "nothing to build: pass --device and/or --sim" >&2
  exit 2
fi

targets=()
[[ $device -eq 1 ]] && targets+=(aarch64-apple-ios)
[[ $sim -eq 1 ]] && targets+=(aarch64-apple-ios-sim)

cargo_flags=(--locked -p termoso-mobile --lib)
[[ $profile == release ]] && cargo_flags+=(--release)

installed="$(rustup target list --installed)"
for t in "${targets[@]}"; do
  grep -qx "$t" <<<"$installed" || rustup target add "$t"
done

# Rust's iOS targets default to a 10.x/13.x floor; match the app.
export IPHONEOS_DEPLOYMENT_TARGET="${IPHONEOS_DEPLOYMENT_TARGET:-17.0}"

echo "==> cargo build ($profile): ${targets[*]}"
for t in "${targets[@]}"; do
  (cd "$root" && cargo build "${cargo_flags[@]}" --target "$t")
done

lib_name=libtermoso_mobile.a
first_lib="$root/target/${targets[0]}/$profile/$lib_name"

work="$(mktemp -d "${TMPDIR:-/tmp}/termoso-ios.XXXXXX")"
trap 'rm -rf "$work"' EXIT

echo "==> uniffi-bindgen (swift)"
(cd "$root" && cargo run -q --locked -p uniffi-bindgen -- generate \
  --library "$first_lib" --language swift --no-format \
  --out-dir "$work/bindings")

# UniFFI emits TermosoCore.swift + TermosoCoreFFI.h + TermosoCoreFFI.modulemap.
# The xcframework wants the module map named module.modulemap next to the header.
headers="$work/headers"
mkdir -p "$headers"
cp "$work/bindings/TermosoCoreFFI.h" "$headers/"
cp "$work/bindings/TermosoCoreFFI.modulemap" "$headers/module.modulemap"

xc_args=()
for t in "${targets[@]}"; do
  xc_args+=(-library "$root/target/$t/$profile/$lib_name" -headers "$headers")
done

echo "==> xcodebuild -create-xcframework"
rm -rf "$pkg/TermosoCoreFFI.xcframework"
xcodebuild -create-xcframework "${xc_args[@]}" -output "$pkg/TermosoCoreFFI.xcframework" >/dev/null

mkdir -p "$pkg/Sources/TermosoCore"
cp "$work/bindings/TermosoCore.swift" "$pkg/Sources/TermosoCore/TermosoCore.swift"

echo "==> done: $pkg/TermosoCoreFFI.xcframework + Sources/TermosoCore/TermosoCore.swift"
