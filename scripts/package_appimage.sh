#!/usr/bin/env bash
# Build GalaxyCraft-<version>-x86_64.AppImage from a clean staged tree.
#
# The package holds the `sc_view` client (as `galaxycraft`), the desktop
# entry, AppStream metadata and icon.  No game data: the client finds the
# player's StarCraft: Remastered install (or takes --install / SC_INSTALL).
# Video (portraits, cinematics) needs an `ffmpeg` binary: set FFMPEG=PATH to
# bundle one (use an LGPL build and ship its licence), otherwise the system
# `ffmpeg` is used.
#
# Usage: scripts/package_appimage.sh [OUT_DIR]   (default: dist/)
# Env:   FFMPEG=/path/to/ffmpeg     bundle this ffmpeg
#        APPIMAGETOOL=/path/to/tool use this appimagetool instead of downloading
set -euo pipefail

root=$(git -C "$(dirname "$0")/.." rev-parse --show-toplevel)
out=$(realpath -m "${1:-$root/dist}")
work=${WORK:-$(mktemp -d)}
app_id=io.github.cotizcesar.GalaxyCraft
version=$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/engine-rust/Cargo.toml" | head -1)

"$root/scripts/stage_clean_tree.sh" "$work/src"
(
    cd "$work/src/engine-rust"
    CARGO_TARGET_DIR="$work/target" cargo build --release --locked --bin sc_view
)

appdir="$work/AppDir"
pack="$work/src/engine-rust/packaging"
rm -rf "$appdir"
install -Dm755 "$work/target/release/sc_view" "$appdir/usr/bin/galaxycraft"
install -Dm644 "$pack/$app_id.desktop" "$appdir/usr/share/applications/$app_id.desktop"
install -Dm644 "$pack/$app_id.metainfo.xml" "$appdir/usr/share/metainfo/$app_id.appdata.xml"
install -Dm644 "$pack/$app_id.svg" "$appdir/usr/share/icons/hicolor/scalable/apps/$app_id.svg"
install -Dm644 "$work/src/LICENSE" "$appdir/usr/share/doc/galaxycraft/LICENSE"
install -Dm644 "$work/src/NOTICE.md" "$appdir/usr/share/doc/galaxycraft/NOTICE.md"
if [ -n "${FFMPEG:-}" ]; then
    install -Dm755 "$FFMPEG" "$appdir/usr/bin/ffmpeg"
    # LGPL builds ship LICENSE.txt one level above bin/.
    lic=$(dirname "$FFMPEG")/../LICENSE.txt
    [ -f "$lic" ] && install -Dm644 "$lic" "$appdir/usr/share/doc/galaxycraft/ffmpeg-LICENSE.txt"
fi
cp "$pack/$app_id.desktop" "$appdir/"
cp "$pack/$app_id.svg" "$appdir/"
ln -sf "$app_id.svg" "$appdir/.DirIcon"
cat > "$appdir/AppRun" <<'EOF'
#!/bin/sh
# Prefer a bundled ffmpeg; keep the system one as the fallback.
here=${APPDIR:-$(dirname "$(readlink -f "$0")")}
export PATH="$here/usr/bin:$PATH"
exec "$here/usr/bin/galaxycraft" "$@"
EOF
chmod 755 "$appdir/AppRun"

tool=${APPIMAGETOOL:-}
if [ -z "$tool" ]; then
    tool="${XDG_CACHE_HOME:-$HOME/.cache}/galaxycraft/appimagetool-x86_64.AppImage"
    if [ ! -x "$tool" ]; then
        mkdir -p "$(dirname "$tool")"
        curl -fsSL -o "$tool" \
            https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage
        chmod 755 "$tool"
    fi
fi
mkdir -p "$out"
target="$out/GalaxyCraft-$version-x86_64.AppImage"
ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 "$tool" --no-appstream "$appdir" "$target"
echo "AppImage: $target"
