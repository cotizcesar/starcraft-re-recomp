#!/usr/bin/env bash
# Build GalaxyCraft-<version>-macos-universal.zip: a universal (arm64 +
# x86_64) GalaxyCraft.app from a clean staged tree.  Runs on macOS.
#
# No game data: the client finds the StarCraft: Remastered install
# (/Applications/StarCraft) or takes --install / SC_INSTALL.  Video uses
# Homebrew's ffmpeg (`brew install ffmpeg`), found even when the app is
# started from Finder.  The app is not signed: open it with right click >
# Open the first time.
#
# Needs: rustup targets aarch64-apple-darwin and x86_64-apple-darwin;
# rsvg-convert (brew install librsvg) for the icon, optional.
# Usage: scripts/package_macos.sh [OUT_DIR]   (default: dist/)
set -euo pipefail

root=$(git -C "$(dirname "$0")/.." rev-parse --show-toplevel)
out=$(cd "$(dirname "${1:-$root/dist}")" && pwd)/$(basename "${1:-$root/dist}")
work=${WORK:-$(mktemp -d)}
version=$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/engine-rust/Cargo.toml" | head -1)
pack="$root/engine-rust/packaging"

"$root/scripts/stage_clean_tree.sh" "$work/src"
bins=()
for target in aarch64-apple-darwin x86_64-apple-darwin; do
    (
        cd "$work/src/engine-rust"
        CARGO_TARGET_DIR="$work/target" cargo build --release --locked --target "$target" --bin sc_view
    )
    bins+=("$work/target/$target/release/sc_view")
done

app="$work/GalaxyCraft.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
lipo -create -output "$app/Contents/MacOS/galaxycraft" "${bins[@]}"
sed "s/@VERSION@/$version/g" "$pack/macos/Info.plist" > "$app/Contents/Info.plist"
cp "$work/src/LICENSE" "$work/src/NOTICE.md" "$app/Contents/Resources/"
if command -v rsvg-convert >/dev/null && command -v iconutil >/dev/null; then
    set_dir="$work/GalaxyCraft.iconset"
    mkdir -p "$set_dir"
    for size in 16 32 128 256 512; do
        rsvg-convert -w $size -h $size "$pack/io.github.cotizcesar.GalaxyCraft.svg" -o "$set_dir/icon_${size}x${size}.png"
        rsvg-convert -w $((size * 2)) -h $((size * 2)) "$pack/io.github.cotizcesar.GalaxyCraft.svg" -o "$set_dir/icon_${size}x${size}@2x.png"
    done
    iconutil -c icns "$set_dir" -o "$app/Contents/Resources/GalaxyCraft.icns"
fi

mkdir -p "$out"
zip="$out/GalaxyCraft-$version-macos-universal.zip"
rm -f "$zip"
ditto -c -k --keepParent "$app" "$zip"
lipo -info "$app/Contents/MacOS/galaxycraft"
echo "macOS zip: $zip"
