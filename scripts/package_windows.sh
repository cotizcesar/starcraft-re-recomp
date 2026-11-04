#!/usr/bin/env bash
# Build GalaxyCraft-<version>-windows-x86_64.zip (cross-compiled with
# mingw-w64 from a clean staged tree).
#
# The zip holds galaxycraft.exe (the `sc_view` client, GUI subsystem), the
# licence and notice.  No game data: the client finds the StarCraft:
# Remastered install (Battle.net's default folder) or takes --install.
# Set FFMPEG=path/to/ffmpeg.exe to ship an (LGPL) ffmpeg next to the client
# for video; its LICENSE.txt one level above bin/ is copied too.
#
# Needs: rustup target x86_64-pc-windows-gnu, x86_64-w64-mingw32-gcc.
# Usage: scripts/package_windows.sh [OUT_DIR]   (default: dist/)
set -euo pipefail

root=$(git -C "$(dirname "$0")/.." rev-parse --show-toplevel)
out=$(realpath -m "${1:-$root/dist}")
work=${WORK:-$(mktemp -d)}
target=x86_64-pc-windows-gnu
version=$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/engine-rust/Cargo.toml" | head -1)
name="GalaxyCraft-$version-windows-x86_64"

"$root/scripts/stage_clean_tree.sh" "$work/src"
(
    cd "$work/src/engine-rust"
    CARGO_TARGET_DIR="$work/target" cargo build --release --locked --target "$target" --bin sc_view
)

dir="$work/$name"
rm -rf "$dir"
mkdir -p "$dir"
cp "$work/target/$target/release/sc_view.exe" "$dir/galaxycraft.exe"
cp "$work/src/LICENSE" "$dir/LICENSE.txt"
cp "$work/src/NOTICE.md" "$dir/NOTICE.md"
if [ -n "${FFMPEG:-}" ]; then
    cp "$FFMPEG" "$dir/ffmpeg.exe"
    lic=$(dirname "$FFMPEG")/../LICENSE.txt
    [ -f "$lic" ] && cp "$lic" "$dir/ffmpeg-LICENSE.txt"
fi
cat > "$dir/README.txt" <<EOF
GalaxyCraft $version - reconstructed StarCraft 1.23 engine

Run galaxycraft.exe.  It plays from your own StarCraft: Remastered install,
found in "Program Files (x86)\\StarCraft" by default; for another folder run
  galaxycraft.exe --install "D:\\Games\\StarCraft"
or set the SC_INSTALL environment variable.  No game data is included.
Project: https://github.com/cotizcesar/starcraft-re-recomp
EOF

mkdir -p "$out"
rm -f "$out/$name.zip"
(cd "$work" && python3 -m zipfile -c "$out/$name.zip" "$name")
echo "Windows zip: $out/$name.zip"
