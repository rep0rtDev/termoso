#!/usr/bin/env bash
# Make `tauri build --bundles appimage` strip libwayland from the AppImage.
#
# The Tauri bundler keeps linuxdeploy and its plugins in ~/.cache/tauri. Since
# tauri-cli 2.12 it rewrites the GTK plugin there from an embedded copy on
# every build, so a patched GTK plugin cannot be pre-seeded any more — but the
# `appimage` output plugin is only downloaded when the file is missing. This
# installs linuxdeploy-plugin-appimage.sh under that name; it removes
# libwayland-* after the input plugins ran and then runs the stock output
# plugin. Run once before building the AppImage (release.yml does).
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
tools="${XDG_CACHE_HOME:-$HOME/.cache}/tauri"

mkdir -p "$tools"
install -m 0755 "$here/linuxdeploy-plugin-appimage.sh" "$tools/linuxdeploy-plugin-appimage.AppImage"
echo "installed $tools/linuxdeploy-plugin-appimage.AppImage"
