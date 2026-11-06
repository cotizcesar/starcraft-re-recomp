# GalaxyCraft (`engine-rust`)

A deterministic Rust reconstruction of StarCraft 1.23.10.13515 and a
playable client, `sc_view`, that reads the player's own installation.  No
retail bytes are in this crate.  What works and what is missing:
[`../PROGRESS.md`](../PROGRESS.md).  Crate rules: [`AGENTS.md`](AGENTS.md).

## Requirements

- Rust **1.99.1** (pinned by `../rust-toolchain.toml`; edition 2024).
- A C compiler (`build.rs` builds the vendored PKWARE DCL decoder).
- `ffmpeg` for video (menu clips, portraits, films); override the binary with
  `GALAXYCRAFT_FFMPEG`.
- To play or run the retail tests: your own StarCraft: Remastered install.

## Quick start

```bash
cargo build --release
cargo run --release --bin sc_view -- --install "/path/to/StarCraft"

# Headless capture of one screen (no window, no audio)
SC_NO_AUDIO=1 cargo run --release --bin sc_view -- --install "$SC_INSTALL" \
  --shot /tmp/pause.bmp --shot-screen pause
```

Supported window sizes: 16:9 from 1280x720 (the default) upwards, and
ultrawide (pillarboxed). A window narrower than 16:9 (4:3, 16:10) shows the
16:9 view letterboxed between black bars. `--width` / `--height` set the
window or capture size.

Without `--install`, `sc_view` and `sc_assets` look in the platform's usual
folders (Battle.net default, Wine/Proton prefixes) or use `SC_INSTALL`.  The
first run materialises what the client needs into
`$XDG_CACHE_HOME/galaxycraft/retail-<build>` (`--cache DIR` or
`GALAXYCRAFT_CACHE` override it); it can be deleted at any time.

In game: drag or shift-click to select, right click to move/attack/gather,
`A` attack-move, `P` patrol, `S` stop, `H` hold, `Ctrl+1..9` groups, `F5`
save, `F6` save replay, `F10` statistics, `Esc` pause menu.  In the LAN
lobby (main menu → game type → `L`): `S` scan, `H` host, `Enter` join.

## Tests

```bash
cargo test                                    # the whole suite
cargo clippy --all-targets -- -D warnings     # must be 0 warnings
SC_INSTALL=/path/to/StarCraft cargo test      # adds the retail-install tests
make verify                                   # test + clippy + orders + oracle + audit
```

Most unit tests read small tables from `fixtures/`, which are Blizzard data
and are not committed: generate them once with
`cargo run --release --bin sc_assets -- --install "$SC_INSTALL" fixtures`
(see [`fixtures/README.md`](fixtures/README.md)).  Tests that need the
install, fixtures or retail replays print `skip` and pass when those are
absent.  Visual and flow gates live in [`../scripts/`](../scripts/README.md);
their committed outputs are in [`docs/gates/`](docs/gates/).

## What is reconstructed

Rules (all deterministic, fixed-seed LCG): damage (74,880 differential
cases), 10 bullet behaviours, octile A* with the recovered heap order and
region graph, economy, tech tree, every spell family, transports, bunkers,
vision and detection, the `iscript` VM (0x00–0x44), the 189-row order table
with 137 transcribed handlers, the trigger VM (conditions 0–23, actions
1–59) and the campaign AI interpreter (`aiscript` 293 + `bwscript` 156
scripts).  Tables loaded: `units.dat` 228, `weapons.dat` 130, `flingy.dat`
209, `orders.dat` 189, `techdata.dat` 44, `upgrades.dat` 61, `sprites.dat`
517, `images.dat` 999, `stat_txt.tbl` 1,539 strings.

Client: Remastered menus, campaign flow (establishing shots, Terran ready
room, victory dialog, score screen), HD sprites, terrain, creep and
lighting, race consoles, portraits, music and voices, saves, replays and the
retail LAN flow (connection, games, create, 8-slot lobby with teams,
computers and chat; the in-game part lives in `src/bin/sc_view/mp.rs`).

Retail install access (`src/retail.rs`, checked by `tests/retail.rs` and
`sc_assets verify`): CASC 52,498 paths / 14,382 index / 43,091 encoding
entries; 323/323 MPQ maps (322 with `TRIG`); the Bottleneck CHK is
byte-identical to the retail extraction; all 391 maps (323 + 68 campaign)
load and simulate (`sc_campaign --install`).

## Binaries (`src/bin/`)

| Binary | Purpose |
|---|---|
| `sc_view` | The playable client |
| `sc_lan` | Headless lockstep LAN match (2–8 peers) |
| `sc_match` | Headless match runner |
| `sc_campaign` | Load and simulate every shipped map |
| `sc_assets` | Verify the install, write fixtures, extract assets, list paths |
| `sc_list`, `sc_dump` | List / dump the CASC path namespace |
| `sc_replaytool` | Create, inspect and verify `.gcrep` / `.gcsave` |
| `sc_replay` | Inspect retail `.rep` files |
| `sc_shot` | Render one map frame to a BMP |
| `sc_sim` | Command-line demo of the rules (`stats`, `duel`) |
| `sc_audit` | Ids used by the shipped map corpus |
| `sc_orders`, `sc_handlers`, `sc_regions` | Print and validate orders, handlers, region graph |
| `sc_animsheet` | Contact sheets of HD animation containers |

## Source map (`src/`)

| Area | Modules |
|---|---|
| Rules | `sim`, `combat`, `projectile`, `economy`, `tech`, `path`, `vision`, `orders`, `handlers`, `iscript`, `trigger`, `aiscript`, `ai`, `ids` |
| Data | `data`, `tables`, `map`, `retail`, `retail_replay`, `sfx`, `button_sets`, `hotkey_data` |
| Persistence and network | `replay`, `profile`, `campaign_progress`, `net`, `multiplayer` |
| Graphics | `render`, `dds`, `hd_anim`, `hd_sprites`, `hd_lights`, `text`, `ui`, `ui_art`, `calibration`, `menu_masks`, `scene` |
| Front end and campaign | `menu`, `campaign`, `ready_room`, `credits`, `buttons`, `transmission` |
| Audio and video | `audio`, `voice`, `video`, `cinematic`, `portrait` |

`tools/`: `oracle.py` (table oracle), `verify_runtime_tables.py` (73/73
runtime arrays), `extract_damage_table.py`, `trigger_ids.py`,
`gen_handlers.py`, and optional CascLib/StormLib extractors (not needed by
the Rust path).

## Documentation

- [`docs/EVIDENCE.md`](docs/EVIDENCE.md): dated evidence for every
  reconstruction pass (historical record).
- [`docs/save-format.md`](docs/save-format.md), [`docs/replays.md`](docs/replays.md): own file formats.
- [`docs/ui-alignment.md`](docs/ui-alignment.md): measured UI geometry.
- [`docs/controller-support.md`](docs/controller-support.md): planned gamepad support.
- [`docs/cef-host.md`](docs/cef-host.md), [`packaging/README.md`](packaging/README.md).
