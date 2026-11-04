#!/usr/bin/env bash
# Build io.github.cotizcesar.GalaxyCraft as a single-file Flatpak bundle.
#
# Stages a clean tree (scripts/stage_clean_tree.sh), vendors the crates so
# flatpak-builder can build offline, then exports GalaxyCraft-<version>.flatpak.
# Needs flatpak-builder (or the org.flatpak.Builder app) and, from Flathub,
# org.freedesktop.Sdk//26.08 + org.freedesktop.Sdk.Extension.rust-stable//26.08
# (installed on demand with --install-deps-from=flathub).
#
# Usage: scripts/package_flatpak.sh [OUT_DIR]   (default: dist/)
set -euo pipefail

root=$(git -C "$(dirname "$0")/.." rev-parse --show-toplevel)
out=$(realpath -m "${1:-$root/dist}")
work=${WORK:-$(mktemp -d)}
app_id=io.github.cotizcesar.GalaxyCraft
version=$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/engine-rust/Cargo.toml" | head -1)

"$root/scripts/stage_clean_tree.sh" "$work/src"
(cd "$work/src/engine-rust" && cargo vendor --locked --quiet vendor >/dev/null)
cp "$root/engine-rust/packaging/flatpak/$app_id.yml" "$work/"

if command -v flatpak-builder >/dev/null; then
    builder=(flatpak-builder)
else
    builder=(flatpak run --filesystem="$work" --filesystem="$out" org.flatpak.Builder)
fi
mkdir -p "$out"
cd "$work"
"${builder[@]}" --user --force-clean --install-deps-from=flathub \
    --repo="$work/repo" "$work/build" "$work/$app_id.yml"
flatpak build-bundle "$work/repo" "$out/GalaxyCraft-$version.flatpak" "$app_id"
echo "Flatpak bundle: $out/GalaxyCraft-$version.flatpak"
