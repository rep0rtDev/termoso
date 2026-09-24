#!/usr/bin/env bash
# Sanity-check a built Termoso AppImage: it must not ship libwayland (see
# linuxdeploy-plugin-gtk.sh) and its GTK hook must leave GDK_BACKEND
# overridable. Usage: verify.sh path/to/Termoso.AppImage
set -euo pipefail

appimage="$(readlink -f "${1:?usage: verify.sh <AppImage>}")"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

cd "$work"
"$appimage" --appimage-extract > /dev/null
root="$work/squashfs-root"

status=0
bundled="$(find "$root"/usr/lib* -name 'libwayland-*.so*' 2> /dev/null || true)"
if [ -n "$bundled" ]; then
    echo "::error::AppImage still bundles libwayland:" >&2
    echo "$bundled" >&2
    status=1
fi

hook="$root/apprun-hooks/linuxdeploy-plugin-gtk.sh"
if ! grep -q 'GDK_BACKEND="${GDK_BACKEND:-x11}"' "$hook"; then
    echo "::error::$hook forces GDK_BACKEND (Termoso linuxdeploy GTK plugin not used?)" >&2
    grep -n GDK_BACKEND "$hook" >&2 || true
    status=1
fi

for lib in libwebkit2gtk-4.1.so.0 libgtk-3.so.0; do
    [ -n "$(find "$root"/usr/lib* -name "$lib" -print -quit 2> /dev/null)" ] || { echo "::error::$lib missing from AppImage" >&2; status=1; }
done

[ "$status" -eq 0 ] && echo "ok: $(basename "$appimage") ships no libwayland, GDK_BACKEND overridable"
exit "$status"
