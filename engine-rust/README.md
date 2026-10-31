# GalaxyCraft

A Rust reconstruction of the **core gameplay rules** of
StarCraft 1.23.10.13515, built from the shipped executable and its CASC data.

The project is a deterministic rules engine **and a playable graphical
client**: it loads the retail tables and a real map, spawns units, mines,
builds, researches, pathfinds, fights, casts spells, evaluates map triggers
and runs a small computer opponent.  `sc_view` decodes the retail
CV5/VX4EX/VR4/WPE terrain chain and GRP sprites and presents a mouse/keyboard
RTS client.  It is still not a byte-for-byte replacement for
`StarCraft.exe`.

## What is reconstructed

* **All gameplay tables**: `units.dat` (228), `weapons.dat` (130),
  `flingy.dat` (209), `orders.dat` (189), `techdata.dat` (44),
  `upgrades.dat` (61), `sprites.dat` (517), `images.dat` (999) and the
  1,539-entry `stat_txt.tbl` string table.
* **Damage pipeline**: upgrade bonus, half-point floor, Defensive Matrix,
  Plasma Shields, armour, damage-type/size multipliers, status effects
  (hallucination, acid spores) and splash divisors. This follows the
  recovered routine at runtime `0x7a13ce30` / `0x7a13cf90`.
* **Weapon rules**: cooldowns, ranges, air/ground and
  mechanical/organic/building target flags, multi-shot weapons, splash rings,
  acquisition range (`max(target_acq, ground_range/32, air_range/32)`).
* **Projectiles**: all ten `weapons.dat` bullet behaviours (fly, follow,
  appear at unit/position/source, persist, self-destruct, bounce, attack
  target position, extend to max range), lifetime, homing, the 1/3/9 bounce
  divisors and the full hit-type table (splash, lockdown, nuclear, parasite,
  broodlings, EMP, irradiate, plague, stasis, Dark Swarm, Yamato, feedback,
  optical flare, Maelstrom, corrosive acid).
* **Movement**: 8-pixel walk grid from the tileset's `vf4` flags, octile A*
  with unit footprints and building obstacles, flying direct paths, and the
  exact per-tick `iscript.bin` Walking cycle (the Zergling's alternating
  speeds are preserved).  The open list follows the recovered
  `0x7a305590` compare-only-`f` heap, so equal-cost routes use the retail
  discovery order rather than a coordinate tie-break.
* **Orders**: the recovered 8-slot order queue with `orders.dat` queueable
  metadata, move/attack-move/attack/hold/patrol/gather/return/build/train/
  research/upgrade/cast/transport/siege/burrow orders.  All 189 rows carry
  their recovered handler address, behaviour class and dispatch lane from
  `0x7a178340`; the 128 carry-gated rows run on the recovered 8-tick
  `CUnit+0x85` counter while movement continues every tick.  The 137 unique
  fixed handler targets have one transcribed Rust routine each in
  `src/handlers.rs`, with the decompiled body, callee order and `CUnit` byte
  writes diffed against the recovery evidence by `cargo test`.
* **Region graph**: the tile-level region builder from `0x7a302c10` (bbox
  centres, minitile edge adjacency, the size-6 small-region merge pass and
  the 2500/5000 region guards) and two-level pathfinding through region
  centres.
* **Visibility**: per-player visible/explored masks, terrain-height sight
  blocking, cloak, personal cloaking energy drain, burrow and detectors
  (7-tile building radius, mobile sight range, Scanner Sweep/Parasite reveal).
* **Special units**: nuclear strike with the retail 2/3 max-HP damage floor,
  Spider Mine arming/detonation, Carrier interceptors, Reaver scarabs, SCV
  repair and Medic auto-healing.
* **Economy**: unit costs, build times (`1/15 s`), supply in half-points,
  worker mining (8 minerals per trip, 1,500 patch capacity), gas requires a
  completed refinery/assimilator/extractor, production queues and refunds.
* **Creep and power**: Zerg structures (except the Hatchery and Extractor)
  require creep; colonies extend it, creep grows one ring per 8 ticks and
  recedes after the source dies. Protoss structures only function inside a
  Pylon power field; production pauses outside power.
* **Unit collision**: ground units separate along their `units.dat` boxes
  instead of stacking.
* **Attack-move resume**: a unit that kills its target continues to the
  attack-move destination instead of stopping.
* **Terran lift/land**: Command Centers and the other liftable Terran
  structures become air units, move to a new location, leave their add-on
  grounded and inactive, and land on a validated buildable footprint
  (production is disabled while airborne).
* **Terran burning**: structures below one third HP lose 20/256 HP per tick
  (1.875 HP/s) and are destroyed if not repaired.
* **Construction details**: Zerg building morphs (Lair/Hive/colonies) keep
  the structure, Drones are consumed by the structures they start, Terran
  add-ons attach to their parent and die with it, and upgraded Zerg
  structures satisfy their base building's requirements.
* **Tech tree**: the hard-coded producer and prerequisite switch recovered
  from the executable (Barracks→Command Center, Factory→Barracks, Spire→Lair,
  Gateway→Nexus, …), plus research/upgrade building gates and level
  requirements.
* **Spells/abilities**: Stim Packs, siege mode, cloak, burrow, Yamato Gun,
  Defensive Matrix, EMP, Irradiate, Psionic Storm, Hallucination, Stasis,
  Recall, Archon/Dark Archon merge, Feedback, Maelstrom, Mind Control,
  Disruption Web, Spawn Broodlings, Parasite, Infestation, Plague, Consume,
  Ensnare, Restoration, Optical Flare, Medic healing and Lockdown.
* **Maps**: raw `staredit\scenario.chk` parsing (DIM/ERA/MTXM/UNIT/MRGN/STR/
  OWNR/SIDE/TRIG), tileset walkability, start locations, resources,
  locations and trigger execution. All **323** shipped `.scm`/`.scx` maps
  load and simulate, including protected ladder maps whose section order and
  optional headers have been scrambled (the parser resynchronises on the
  required sections) and Remastered maps that store flags in the high bits
  of `ERA`.
* **Transport loading/unloading**, bunkers (four infantry slots whose
  occupants fire from the bunker, cargo dies with the building), Nydus Canal
  paired teleport, Shield Battery (2 shields per energy), Zerg larva spawning
  and morphing.
* **iscript animation VM**: the full 0x00..0x44 opcode dispatcher, waits,
  jumps, calls, frames, flips, turns, overlays/sounds, walking `move`
  sequences, attack release events (`attack`, `attackwith`, `attackmelee`,
  `domissiledmg`) and `castspell`, driven per unit per tick.
* **Campaign AI scripts**: `aiscript.bin`/`bwscript.bin` table parsing, the
  full opcode stream/disassembler and a deterministic interpreter for every
  shipped opcode family (town, build/train, attack, defense, expansion,
  transports, bunkers, nukes, harassment, scouting, child threads).
* **Triggers**: the complete classic 2,400-byte `TRIG` VM: every action
  1..=59 (preserve/wait resumption, `UPRP` Create Unit with Properties,
  kill/remove counts, percentage HP/energy/shield edits, score categories,
  invincibility, doodad state, move/patrol/attack orders, alliances and the
  presentation/leaderboard records) and every condition 0..=23 with the CHK
  `Exactly` comparison value 10.
* **Campaign player slots**: `OWNR`/`SIDE` recover every live slot, including
  non-contiguous owners (Enslavers uses 0/3/4/6), so campaign enemy bases are
  loaded with their authored race.  Spare `OWNR` slots on melee maps do not
  create phantom players.
* **Briefing/results and progression**: mission name, briefing and objectives
  come from the map's `STR` section; victory resolves the trigger's
  `Next Scenario` string (stored one past the STR index) to the next bundled
  mission.  The results panel shows time, kills, losses, razing and income.
* **Full retail campaign**: all 68 non-empty story missions in
  `sd/campaign/<episode>/<mission>/staredit/scenario.chk` (Terran, Zerg,
  Protoss and all three Brood War episodes) are enumerated in retail order
  and playable from the campaign tab.  Each mission opens its shipped
  `est*.txt` establishing-shot script over the real `glue/pal*` backdrop art
  (DDS decoded by `src/dds.rs`), with page-by-page continuation.  The client
  follows the `OWNR == 6` human seat instead of assuming player 0, and
  advances to the next mission of the episode on victory.  The same campaign
  corpus is simulated by `sc_campaign --install` (391/391 maps, 0 failed).
* **Client rendering**: mission briefing, result overlay, animated procedural
  creep over the gameplay maturity grid, minimap units and camera.
* **Audio**: nine shipped CASC WAV effects (UI, acknowledgements, mission
  start, victory/defeat) decoded natively and mixed by a small ALSA backend;
  the mixer degrades to a no-op on headless systems.
* **A deterministic AI** that mines, maintains supply, follows a short build
  order and attacks.

## Retail installation as the asset source (no retail files in the repo)

The engine can read the player's own installation directly, the way a
decompilation project's runtime loads the original game data.  Nothing in
`src/retail.rs` contains retail bytes, and the client does not need a bundled
`assets/` directory when `--install` is used:

```bash
# Verify the live install: CASC paths/keys, 323 MPQ maps, table hashes.
cargo run --release --bin sc_assets -- \
  --install "/path/to/StarCraft" verify

# Read a map straight out of Maps/*.scm and the tables/tileset from CASC.
cargo run --release --bin sc_match -- \
  --install "/path/to/StarCraft" --map Bottleneck --ticks 2000

# Play with every sprite, tileset, effect and map sourced from the install.
cargo run --release --bin sc_view -- --install "/path/to/StarCraft"
```

The first run materialises the subset the client needs into
`$XDG_CACHE_HOME/galaxycraft/retail-<build>` (override with `--cache DIR` or
`GALAXYCRAFT_CACHE`).  `sc_assets extract --out DIR` does the same explicitly.
The cache is derived from the user's files and can be deleted at any time.

`src/retail.rs` implements, from the on-disk formats:

* the CASC text build config, `%02x%08x.idx` local key index, `data.NNN`
  archives and BLTE `N`/`Z` frames;
* the ENCODING manifest (`CKey → EKey`) and StarCraft's text ROOT manifest
  (`path|cKey`);
* the shipped MPQ v1 map archives, including protected variants:
  encrypted hash/block tables, `KEY_V2` file keys, hash-table size
  inflation, `FILE_IMPLODE` sector streams and PKWARE DCL sectors;
* the payload name used for MPQ file keys (`scenario.chk`, not the full
  `staredit\scenario.chk` path).

The result is checked against the original in `tests/retail.rs`: 323/323
shipped MPQ maps decompress and parse, 322 contain `TRIG`, the Bottleneck CHK
is byte-identical to the retail extraction, and all documented DAT/GRP/WAV
hashes match.

## What is not reconstructed

The stock Terran/Zerg/Protoss console HUD, main menu, pause menu, score panel,
in-game leaderboard, mission statistics and title intro are drawn from the
retail DDS/GRP surfaces and the exact `*.ui.json` rectangles; the cosmetic
20th-anniversary/Carbot skins are not used.  The creep overlay is rendered from
the recovered maturity grid with a deterministic organic texture (CASC ships
no creep GRP), so the mat animates correctly but is not a pixel copy of the
retail texture.  Music and the complete voice sets are decoded from the
player's installation with `--install` (23 Ogg tracks and 1,122 voice files);
nine small WAV effects remain bundled for no-install runs.  The same loader
can read any of them by path with `sc_assets path <CASC path>`.

Gameplay is complete: the 137 internal hero/AI/doodad handler bodies are
transcribed one-to-one in `src/handlers.rs`, with their verbatim pseudocode,
callee sequences and fingerprints diffed against the recovery evidence by
`tests/handlers.rs`; SC:R EUD-only trigger memory paths are parsed but not
executed because all 323 shipped maps use only classic trigger ids
(`tools/trigger_ids.py`).  The only remaining non-gameplay limits are the
cosmetic 20th-anniversary/Carbot skins, pixel-exact creep, and decoded WebM
not used beyond title/briefing.  The complete list of open items and their
evidence is in [docs/EVIDENCE.md](docs/EVIDENCE.md).

![Title intro](docs/retail_menu.png)

![Playable graphical client with the retail console](docs/playable.png)

![Selected unit wireframe and exact StatData fields](docs/selection.png)

Campaign briefing and the animated creep overlay:

![Mission briefing](docs/briefing.png)

![Creep rendered from the maturity grid](docs/creep.png)

The bundled map pack and the 1:1 start menu:

![Start menu](docs/menu.png)

![Select Game Type dialog](docs/gametype.png)

Victory score screen and the in-game leaderboard:

![Score screen](docs/score.png)

![Leaderboard](docs/leaderboard.png)

## Quick start

```bash
cargo test

# Play using the player's own retail installation (no bundled art/audio):
cargo run --release --bin sc_view -- --install "/path/to/StarCraft"
# Or use a pre-extracted asset directory and a specific map:
cargo run --release --bin sc_view -- --chk mapa.chk --tiles assets/TileSet

# Headless screenshots of every retail UI surface:
for s in intro menu briefing mission selection pause leaderboard stats score; do
  cargo run --release --bin sc_view -- --install "/path/to/StarCraft" \
    --shot "/tmp/$s.bmp" --shot-screen "$s"
done

# Verify that CASC and all 323 MPQ maps are read from the install:
cargo run --release --bin sc_assets -- --install "/path/to/StarCraft" verify

cargo run --bin sc_sim -- stats zealot
cargo run --bin sc_sim -- duel marine zealot
cargo run --bin sc_match -- --ticks 4000 --verbose
```

`sc_view` opens ALSA for the bundled effects; set `SC_NO_AUDIO=1` on a
headless box.  Controls: menu Up/Down + Enter, `R` chooses race, `I` replays
the title intro; in game drag/shift-click select, right click
move/attack/gather, `A` attack-move, `P` patrol, `S` stop, `H` hold,
`B`/number keys build or train, `Ctrl+1..9` control groups, arrows scroll,
`Space` centre selection, `Home` centre base, `F` fog, `L` leaderboard,
`F10` mission statistics, `+/-` speed, `Tab` idle worker, `Esc` pause.  The
pause menu offers resume, restart, statistics, map menu and quit.  `sc_shot`
renders a frame headlessly to a BMP for regression checks.

Run a real map (extract the CHK and tileset first, see below):

```bash
cargo run --release --bin sc_match -- \
  --chk bottleneck.chk \
  --vf4 platform.vf4 \
  --races terran,protoss \
  --ticks 20000 --verbose

# Rules-level fog of war and a shipped campaign AI for player 0:
cargo run --release --bin sc_match -- --ticks 20000 --fog --ai-script Ter3

# Inventory the trigger/unit ids actually used by the shipped map corpus:
cargo run --release --bin sc_audit -- --maps /tmp/sc_maps

# Load and step every extracted shipped map (coverage harness):
cargo run --release --bin sc_campaign -- \
  --maps /tmp/sc_maps --tiles /tmp/tiles/TileSet --ticks 1200
```

## Layout

The executable completeness checklist is in
[docs/COVERAGE.md](docs/COVERAGE.md).

| Path | Purpose |
|------|---------|
| `src/data.rs` | Column-major DAT parsers, unit/weapon/flingy types, provenance helpers |
| `src/tables.rs` | Orders, tech, upgrades, sprites, images, string table |
| `src/iscript.rs` | `iscript.bin` reader and walking-speed recovery |
| `src/combat.rs` | Damage pipeline, shields, status branches, fixed-tick battle sim |
| `src/projectile.rs` | Bullet behaviours, lifetime, bounce and impact state |
| `src/vision.rs` | Per-player visible/explored masks, cloak and detector rules |
| `src/audio.rs` | WAV decoder, effect bank and ALSA software mixer |
| `src/retail.rs` | Read-only CASC + MPQ installation reader, asset materialiser |
| `build.rs`, `third_party/pklib/` | Vendored MIT PKWARE DCL explode for MPQ maps |
| `src/render.rs` | CV5/VX4EX/VR4/WPE terrain, GRP decoder, sprites, fog, minimap, framebuffer text |
| `src/dds.rs` | DXT1/DXT3/DXT5/uncompressed DDS decoder, `.dds.grp` frame archives, alpha blitting |
| `src/ui.rs` | Remastered `*.ui.json` dialog/control parser |
| `src/ui_art.rs` | Race console triptych, command icons/wireframes, menu and score surfaces |
| `src/sfx.rs` | `sfxdata.tbl`/`.dat` parser (voice id -> CASC path) |
| `src/voice.rs` | Unit voice-folder map, ready/what/pissed/yes cue selection and lazy WAV cache |
| `src/audio.rs` | WAV + Ogg Vorbis decode, software mixer with looping music |
| `src/economy.rs` | Costs, supply, production and mining primitives |
| `src/map.rs` | CHK parser, tileset walkability, map units/locations |
| `src/path.rs` | Tile-level region graph (recovered builder), region-centre routing, octile A* with footprints and obstacles |
| `src/tech.rs` | Hard-coded build/research tree |
| `src/sim.rs` | World, units, orders, spells, projectiles, production, mining, transports |
| `src/orders.rs` | All 189 `orders.dat` rows: recovered handler VA, dispatch lane and behaviour class |
| `src/handlers.rs` | 137 transcribed order-handler routines plus the VA registry, body fingerprints and dispatcher |
| `src/trigger.rs` | Complete CHK trigger VM (conditions 0..=23, actions 1..=59, `UPRP`) |
| `src/aiscript.rs` | `aiscript.bin`/`bwscript.bin` parser, disassembler and interpreter |
| `src/ai.rs` | Deterministic computer opponent |
| `src/ids.rs` | Numeric ids for units/weapons/tech/upgrades/orders |
| `src/bin/sc_match.rs` | Headless match runner |
| `src/bin/sc_campaign.rs` | All-shipped-maps load/simulate coverage harness |
| `src/bin/sc_audit.rs` | Trigger/unit/tileset id histograms over the shipped map corpus |
| `src/bin/sc_regions.rs` | Print the recovered tile-level region graph for a CHK |
| `src/bin/sc_orders.rs` | Print/validate all 189 recovered order rows and dispatch lanes |
| `src/bin/sc_handlers.rs` | Print/validate the 137 transcribed handler routines and their evidence coverage |
| `src/bin/sc_view.rs` | Playable graphical RTS client (minifb, retail console/menu/score, mouse/keyboard, HUD, minimap) |
| `src/bin/sc_shot.rs` | Headless BMP frame renderer used by graphics tests |
| `src/bin/sc_assets.rs` | Verify/extract CASC + MPQ data from a retail install |
| `src/bin/sc_list.rs` | List CASC paths in an installation (prefix/contains/suffix filters) |
| `tests/retail.rs` | Original-install CASC/MPQ integration tests |
| `assets/` | Retail tileset chain, `Units.pal`, 80 GRP sprites, nine retail WAV effects and the bundled map pack |
| `tools/casc_extract.cpp` | CascLib extractor for `arr\*.dat`, scripts and strings |
| `tools/mpq_extract.cpp` | StormLib extractor for `staredit\scenario.chk` |
| `tools/oracle.py` | Independent table/damage oracle |
| `tools/trigger_ids.py` | Inventory of trigger ids used by the shipped maps |
| `tools/gen_handlers.py` | Regenerate `src/handlers.rs` and the 137-row handler evidence manifest |

## Data provenance

`fixtures/` contains the small rules tables used by the unit tests.  The
runtime no longer depends on it: `--install` reads `arr/*.dat`,
`scripts/*.bin`, `rez/stat_txt.tbl`, the tilesets, unit GRPs, effects and all
323 maps from the user's own CASC/MPQ installation.  The bundled `assets/`
directory is a developer convenience and is not required by `sc_view
--install`; it can be deleted.

The standalone extractors in `tools/` are still available for development
(CascLib/StormLib are MIT-licensed), but the shipped Rust path does not need
them:

```bash
g++ -O2 -std=c++17 -I CascLib/src tools/casc_extract.cpp -o casc_tool \
    -L CascLib/build -lcasc -Wl,-rpath,$PWD/CascLib/build
./casc_tool "/path/to/StarCraft" extract fixtures 'arr\*.dat'

g++ -O2 -std=c++17 -I StormLib/src tools/mpq_extract.cpp -o mpq_tool \
    -L StormLib/build -lstorm -lz -lbz2
./mpq_tool "Maps/(2)Bottleneck.scm" extract bottleneck.chk 'staredit\scenario.chk'
```

The executable and the game's art/audio/maps are not redistributed. The retail `.text` on disk is encrypted;
the logic below was recovered from a read-only snapshot of the running
process and from the executable's data sections. See
[docs/EVIDENCE.md](docs/EVIDENCE.md).

## Testing against the original

`cargo test` runs **241** tests.  They include:

1. unit tests for damage order, shields, armour, size multipliers, floor,
   production, mining, pathfinding, tech gates, triggers, transports and AI;
2. projectile tests for travel time, instant weapons, homing, cloak/detector
   visibility, fog, spider mines, carriers, nuclear damage and the order
   queue's `orders.dat` metadata;
3. campaign-script parser/disassembler tests over all 293 `aiscript.bin` and
   156 `bwscript.bin` entries, a full-opcode decode check, and a real `TMCu`
   run that builds and trains units;
4. iscript tests that decode every shipped animation, replay every Walking
   cycle through the VM, and verify the Marine attack release tick;
5. graphics tests that decode a retail Marine GRP, render a real tileset tile,
   build the minimap, verify the playable-race sprite bank, animate the creep
   mat from the maturity grid and load every bundled map;
6. full-table differential tests against `tools/oracle.py` for all 228 units,
   130 weapons, 209 flingy rows and 189 order rows, covering every scalar column;
7. 74,880 damage differential cases (all 130 weapons × upgrades × sizes × armour × shield states);
8. reconstruction invariants for the recovered order-queue size, walking
   cycles, acquisition range, time scaling, build tree, weapon-id mapping,
   order families and all classic trigger condition/action ids;
9. handler-table diffs that require 137 distinct routine pointers, the
   verbatim decompiled body and callee order for every target, the same rows,
   lane and family as `ORDER_SPECS`, and entry of all 137 routines from the
   public `World::issue_order_id` path;
9. the shipped `triggers\melee.trg` command/opponents records;
10. campaign slot/briefing/progression tests: non-contiguous `OWNR` owners,
    mission copy from `STR`, and the one-based `Next Scenario` string;
11. audio tests that decode all nine retail WAV effects and resample them to
    the mixer rate, plus the disabled-device fallback;
12. optional provenance checks against the extracted DATs and the executable's
    damage table;
13. a read-only runtime cross-check (`tools/verify_runtime_tables.py`) that
    compares the game's own expanded `.data` arrays with the fixtures. Against
    the 1.23.10 runtime snapshot all **73/73** arrays matched byte-for-byte;
14. a map-coverage harness (`sc_campaign`) that loads and steps **323/323**
    shipped maps, including protected ladder maps and the campaign maps, with
    the trigger VM and AI scripts active;
15. `tests/retail.rs`, which opens the player's CASC storage and MPQ map
    archives directly, checks the documented hashes, decompresses all 323
    protected/unprotected maps, and renders a frame from a materialised cache.
    Run it with `SC_INSTALL=/path/to/StarCraft cargo test --test retail`.

```bash
SC_DATA_DIR=/path/to/extracted/arr \
SC_EXE="/path/to/StarCraft/x86/StarCraft.exe" cargo test

# Optional full match on an extracted retail map:
SC_MAP_CHK=bottleneck.chk SC_VF4=platform.vf4 cargo test --release \
  --test reconstruction full_match_on_a_real_map_when_available

# Optional all-maps load/step test:
SC_MAP_DIR=/tmp/sc_maps SC_TILE_DIR=/tmp/tiles/TileSet \
  cargo test --release --test reconstruction every_shipped_map
```
