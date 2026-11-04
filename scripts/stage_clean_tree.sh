#!/usr/bin/env bash
# Copy the repository's tracked and non-ignored files into DEST.
#
# Release packages are built from this staged tree only, so nothing that
# .gitignore keeps local (retail-derived fixtures, menu masks, extracted
# assets, captures) can end up inside a package: the packaged client reads
# all game data from the player's own install at run time.
#
# Usage: scripts/stage_clean_tree.sh DEST
set -euo pipefail

dest=${1:?usage: stage_clean_tree.sh DEST}
root=$(git -C "$(dirname "$0")/.." rev-parse --show-toplevel)

rm -rf "$dest"
mkdir -p "$dest"
# -c tracked, -o untracked, --exclude-standard honours .gitignore.  Files
# deleted in the working tree are skipped.
git -C "$root" ls-files -z -co --exclude-standard |
    while IFS= read -r -d '' f; do
        [ -e "$root/$f" ] || continue
        mkdir -p "$dest/$(dirname "$f")"
        cp -p "$root/$f" "$dest/$f"
    done

# Guard: no developer fixture data may be staged.
leaked=$(find "$dest/engine-rust/fixtures" -type f \
    ! -name README.md ! -name SHA256SUMS ! -path '*/audio/*' 2>/dev/null || true)
if [ -n "$leaked" ] || [ -d "$dest/engine-rust/src/menu_masks" ] || [ -d "$dest/engine-rust/assets" ]; then
    echo "stage_clean_tree: retail-derived files were staged:" >&2
    echo "$leaked" >&2
    exit 1
fi
echo "staged $(find "$dest" -type f | wc -l) files into $dest"
