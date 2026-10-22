#!/usr/bin/env bash
# linuxdeploy input plugin (type 0) run after the GTK plugin: removes the
# libwayland-* copies it deploys. A bundled libwayland shadows the system one
# and a current Mesa's libEGL then fails to load (missing wl_* symbols), so
# WebKitWebProcess aborts with EGL_BAD_PARAMETER and the window stays blank
# (tauri-apps/tauri#15665, mesa/mesa#11316). The host's copy must be used.
set -euo pipefail

appdir=""
for arg in "$@"; do
    case "$arg" in
        --plugin-api-version) echo 0; exit 0 ;;
        --plugin-type) echo input; exit 0 ;;
        --appdir=*) appdir="${arg#--appdir=}" ;;
        --appdir) : ;;
        *) appdir="${appdir:-$arg}" ;;
    esac
done
[ -d "$appdir" ] || { echo "linuxdeploy-plugin-termoso: AppDir not found: '$appdir'" >&2; exit 1; }

echo "Removing bundled libwayland from $appdir"
find "$appdir"/usr/lib* \( -type f -o -type l \) -name 'libwayland-*.so*' -print -delete
