# Notice — no game assets

This repository is an independent, clean-room-oriented reimplementation of
StarCraft 1.23 gameplay and menu behaviour. It contains **no StarCraft game
assets**: no art, sprites, icons, audio, music, voices, video, maps, fonts,
screenshots, rules tables, decompiled code or retail binaries are included
or redistributed.

The client reads the player's own StarCraft installation at runtime through
`--install` / `SC_INSTALL`, and the installation is never modified.

Everything derived from retail data is generated locally from the player's
own install and is gitignored. That covers the developer fixtures in
`engine-rust/fixtures/` (only its `README.md`, `SHA256SUMS` and a synthetic
`audio/tone.ogg` are tracked), the text coverage masks in
`engine-rust/src/menu_masks/`, the calibration layers, the extracted assets,
memory dumps, Ghidra projects, decompiled handler bodies and all screenshot
evidence. `SHA256SUMS` only lists hashes so you can check your locally
generated fixtures.

`research/manifests/` lists the file names, sizes and SHA-256 hashes of the
reference installation so you can verify a local copy. It contains no file
contents.

StarCraft, Brood War and Blizzard Entertainment are trademarks or registered
trademarks of Blizzard Entertainment, Inc. This project is not affiliated
with or endorsed by Blizzard.

Code is licensed under the MIT License (see `LICENSE`).
