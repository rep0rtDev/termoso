#!/usr/bin/env bash
# Install Termoso's linuxdeploy GTK plugin where the Tauri bundler looks for
# it, so `tauri build --bundles appimage` uses it instead of downloading the
# upstream one. Run once before building the AppImage (release.yml does).
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
tools="${XDG_CACHE_HOME:-$HOME/.cache}/tauri"

mkdir -p "$tools"
install -m 0755 "$here/linuxdeploy-plugin-gtk.sh" "$tools/linuxdeploy-plugin-gtk.sh"
echo "installed $tools/linuxdeploy-plugin-gtk.sh"
