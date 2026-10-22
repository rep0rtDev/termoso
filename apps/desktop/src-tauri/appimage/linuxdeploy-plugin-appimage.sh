#!/usr/bin/env bash
# linuxdeploy output plugin (type 0) that stands in for the stock `appimage`
# one: it first removes the libwayland-* copies the GTK plugin deployed, then
# hands over to the appimage plugin shipped inside linuxdeploy itself.
#
# A bundled libwayland shadows the system one and a current Mesa's libEGL then
# fails to load (missing wl_* symbols), so WebKitWebProcess aborts with
# EGL_BAD_PARAMETER and the window stays blank (tauri-apps/tauri#15665,
# mesa/mesa#11316). The host's copy must be used.
#
# prepare.sh installs this as ~/.cache/tauri/linuxdeploy-plugin-appimage.AppImage,
# the file the Tauri bundler downloads only when missing; linuxdeploy looks for
# plugins next to its own AppImage first, so this one wins over the built-in.
set -euo pipefail

appdir=""
for arg in "$@"; do
    case "$arg" in
        --plugin-api-version) echo 0; exit 0 ;;
        --plugin-type) echo output; exit 0 ;;
        --appdir=*) appdir="${arg#--appdir=}" ;;
        --appdir) : ;;
        *) appdir="${appdir:-$arg}" ;;
    esac
done
[ -d "$appdir" ] || { echo "linuxdeploy-plugin-appimage (termoso): AppDir not found: '$appdir'" >&2; exit 1; }

echo "Removing bundled libwayland from $appdir"
find "$appdir"/usr/lib* \( -type f -o -type l \) -name 'libwayland-*.so*' -print -delete

# $LINUXDEPLOY is set by linuxdeploy for its plugins and points at the running
# binary inside the extracted AppImage; the stock output plugin sits next to it.
real="$(dirname "${LINUXDEPLOY:?not run by linuxdeploy}")/linuxdeploy-plugin-appimage"
[ -x "$real" ] || { echo "linuxdeploy-plugin-appimage (termoso): built-in plugin not found at $real" >&2; exit 1; }
exec "$real" "$@"
