# Notice — no game assets

This repository is an independent, clean-room-oriented reimplementation of
the StarCraft 1.23 gameplay/menu behavior.  It contains **no StarCraft game
assets**: no art, sprites, icons, audio, music, voices, video, maps, fonts,
screenshots, or retail binaries are included or redistributed.

The client reads the player's own StarCraft installation at runtime through
`--install` / `SC_INSTALL`; the installation is never modified.

Small functional metadata files under `engine-rust/fixtures/` (action ids,
key codes, card positions, icon filenames and UI layout rectangles) are
included only for interoperability.  They contain no images, fonts or audio.
The 1-bit alpha coverage masks under `engine-rust/src/menu_masks/` are
measured text coverage, not retail artwork or font files.  They contain no
source pixels or glyph outlines.  `engine-rust/fixtures/calibration/` and
all screenshot evidence are deliberately ignored because they are derived
from retail captures.

StarCraft, Brood War and Blizzard Entertainment are trademarks or registered
trademarks of Blizzard Entertainment, Inc.  This project is not affiliated
with or endorsed by Blizzard.

Code is licensed under MIT OR Apache-2.0 (see `LICENSE-MIT` and
`LICENSE-APACHE`).
