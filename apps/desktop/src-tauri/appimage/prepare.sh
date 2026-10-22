#!/usr/bin/env bash
# Make `tauri build --bundles appimage` strip libwayland from the AppImage.
#
# The Tauri bundler keeps linuxdeploy in ~/.cache/tauri and, since tauri-cli
# 2.12, rewrites its GTK plugin there from an embedded copy on every build —
# a patched plugin cannot be pre-seeded any more. It does reuse an existing
# linuxdeploy binary, though, so this installs linuxdeploy-wrapper.sh under
# the name the bundler looks for; the wrapper runs the real linuxdeploy with
# Termoso's plugin (linuxdeploy-plugin-termoso.sh) appended. Run once before
# building the AppImage (release.yml does).
set -euo pipefail

# Must match LINUXDEPLOY_COMMIT_HASH in tauri-bundler's appimage/linuxdeploy.rs.
# If a newer bundler changes it, the wrapper is bypassed and verify.sh fails.
linuxdeploy_commit=07333c6

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
arch="$(uname -m)"
tools="${XDG_CACHE_HOME:-$HOME/.cache}/tauri"
own="$tools/termoso"
real="$own/linuxdeploy-$arch.AppImage"

mkdir -p "$own"
if [ ! -x "$real" ]; then
    url="https://github.com/tauri-apps/binary-releases/releases/download/linuxdeploy-$linuxdeploy_commit/linuxdeploy-$arch.AppImage"
    echo "downloading $url"
    curl -fsSL --retry 5 -o "$real.tmp" "$url"
    chmod 0755 "$real.tmp"
    mv "$real.tmp" "$real"
fi
install -m 0755 "$here/linuxdeploy-plugin-termoso.sh" "$own/linuxdeploy-plugin-termoso.sh"
install -m 0755 "$here/linuxdeploy-wrapper.sh" "$tools/linuxdeploy-$linuxdeploy_commit-$arch.AppImage"
echo "installed $tools/linuxdeploy-$linuxdeploy_commit-$arch.AppImage -> $real + plugin termoso"
