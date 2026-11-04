# Packages

Release packages of the GalaxyCraft client (`sc_view`, installed as
`galaxycraft`).  None contains game data: the client reads your own
StarCraft: Remastered install at run time, including the command-card table
(`x86/StarCraft.exe`) and the Hotkeys panel data (`hotkeypanel.js`,
`stat_txt.tbl`).

| Package | Built by | Install folder found automatically | Video (`ffmpeg`) |
|---|---|---|---|
| Linux AppImage | `scripts/package_appimage.sh` | Battle.net/Wine prefixes in `~`, `/opt/StarCraft` | bundled LGPL build (release), else the system one |
| Linux Flatpak | `scripts/package_flatpak.sh` | the same, read-only (`--filesystem=home:ro`) | the runtime's |
| Windows zip | `scripts/package_windows.sh` | `%ProgramFiles(x86)%\StarCraft` | bundled `ffmpeg.exe` (release) |
| macOS app | `scripts/package_macos.sh` | `/Applications/StarCraft` | Homebrew (`brew install ffmpeg`) |

Any other folder: start with `--install "<StarCraft folder>"` or set
`SC_INSTALL`.  The Flatpak only sees your home folder; for an install
elsewhere run
`flatpak override --user --filesystem=/path/to/StarCraft:ro io.github.cotizcesar.GalaxyCraft`.
The macOS app is not signed: open it with right click > Open the first time.

Derived data (extracted art, saves, replays, profiles) goes to the platform
cache: `$XDG_CACHE_HOME` or `~/.cache` (Linux), `%LOCALAPPDATA%` (Windows),
`~/Library/Caches` (macOS), each under `galaxycraft/`.

## Building

Every script stages a clean copy of the repository first
(`scripts/stage_clean_tree.sh`: tracked and non-ignored files only; it fails
if a fixture, mask or extracted asset would be included) and writes to
`dist/` (gitignored) unless given another folder.

```bash
scripts/package_appimage.sh            # needs curl (fetches appimagetool once)
FFMPEG=/path/to/lgpl/bin/ffmpeg scripts/package_appimage.sh
scripts/package_flatpak.sh             # flatpak-builder or org.flatpak.Builder
scripts/package_windows.sh             # rustup target x86_64-pc-windows-gnu + mingw-w64
scripts/package_macos.sh               # on macOS, both Apple targets installed
```

`.github/workflows/release.yml` runs all four on a `v*` tag and publishes the
release with SHA-256 sums.

Files here: the desktop entry, AppStream metainfo and SVG icon (Linux), the
Flatpak manifest (`flatpak/`) and the app bundle `Info.plist` (`macos/`).
