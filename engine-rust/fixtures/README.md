# Developer fixtures (not in the repository)

The unit tests and the install-less fallback (`GameData::bundled()`) read
StarCraft's own rules tables, scripts and layouts.  They are Blizzard data,
so they are **gitignored**: generate them from your own installation.  Only
this README, `SHA256SUMS` and `audio/tone.ogg` (a synthetic 1 s tone) are
tracked.

`build.rs` mirrors every embedded file into `$OUT_DIR/embed/` and writes an
empty placeholder for any that are missing, emitting a `cargo:warning`.  This
lets the crate, and especially `sc_assets`, build from a fresh clone.  Until
the files exist, `GameData::bundled()` returns an error and the
fixture-backed tests fail.  `sc_view --install DIR` does not need them.

## One-time setup

```bash
# from engine-rust/, with SC_INSTALL pointing at StarCraft: Remastered
cargo run --release --bin sc_assets -- --install "$SC_INSTALL" fixtures --out fixtures
python3 tools/extract_damage_table.py "$SC_INSTALL/x86/StarCraft.exe" fixtures
python3 tools/oracle.py fixtures
(cd fixtures && sha256sum -c SHA256SUMS)
cargo test
```

The full suite also reads other local, gitignored data: `assets/` (from
`sc_assets --install DIR extract --out assets`), `docs/evidence/` (the REA
evidence files) and `calibration/`.

## Where each file comes from

| File(s) | Source | Tool |
|---|---|---|
| `*.dat`, `sfxdata.tbl`, `stat_txt.tbl`, `iscript.bin`, `aiscript.bin`, `bwscript.bin`, `melee.trg` | CASC (`arr/`, `rez/`, `scripts/`, `triggers/`) | `sc_assets fixtures` (`retail::FIXTURE_SOURCES`) |
| `ui/*.ui.json`, `ui/flc.json` | CASC (`locales/enus/assets/rez/`, `rez/flc.json`) | `sc_assets fixtures` |
| `damage_multipliers.bin` | `StarCraft.exe`, file offset `0xb05d7c` (100 bytes) | `tools/extract_damage_table.py` |
| `oracle_*.tsv` | derived from the tables above | `tools/oracle.py fixtures` |
| `button_sets.in` | menu-time memory dump, offset `0xb2ddf8` | `../scripts/gen_button_sets.py DUMP` |
| `hotkeys.json` | CEF hotkey panel JS + `OnBindingsChanged` from the live client | `research/tools/extract_hotkeys_ws.py`; method in `research/docs/hotkeys-data.md` |
| `../src/menu_masks/*.bin` | text coverage measured on the menu reference captures | `../scripts/extract_menu_masks.py` |
| `calibration/` | reference-derived corrections (optional) | `../scripts/make_calibration.py` |

You can only regenerate the last four rows if you have a live client, a dump
or the captures.  Without them, the parts that depend on those files degrade
as follows:
- the command card shows no buttons;
- the Hotkeys panel is empty;
- the opt-in `GC_TEXT_MASKS=1` menu masks draw nothing (the default font renderer is unaffected);
- no calibration is applied.

`SHA256SUMS` (written by `tools/oracle.py`) pins `units.dat`, `weapons.dat`,
`flingy.dat` and `damage_multipliers.bin` for build 1.23.10.13515.
