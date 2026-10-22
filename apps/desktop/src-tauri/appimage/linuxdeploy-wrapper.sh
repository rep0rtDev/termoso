#!/usr/bin/env bash
# Stand-in for the linuxdeploy AppImage the Tauri bundler expects to find in
# ~/.cache/tauri (prepare.sh installs it under that exact file name). Runs the
# real linuxdeploy from the termoso/ subdirectory with `--plugin termoso`
# added after the bundler's own input plugins, so Termoso's plugin runs last
# and strips the libwayland copies the GTK plugin deployed.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
real="$here/termoso/linuxdeploy-$(uname -m).AppImage"
[ -x "$real" ] || { echo "linuxdeploy-wrapper: $real missing, run appimage/prepare.sh" >&2; exit 1; }

args=()
for arg in "$@"; do
    case "$arg" in
        --output|--output=*|-o) [ -n "${added:-}" ] || { args+=(--plugin termoso); added=1; } ;;
    esac
    args+=("$arg")
done

# linuxdeploy looks for plugins next to its own AppImage first, then on PATH;
# the bundler writes the GTK plugin to this directory, so it has to be on PATH too.
export PATH="$here/termoso:$here:$PATH"
exec "$real" "${args[@]}"
