# Evidence ledger

> Historical record: one section per reconstruction pass, with the numbers
> measured on that date.  Current status: [`PROGRESS.md`](../../PROGRESS.md).

This document records what was recovered from the retail StarCraft
1.23.10.13515 installation, how it was recovered, and what remains unknown.
The raw REA evidence records used for the first table/damage pass are stored
under `docs/evidence/`.

## Target

| Field | Value |
|---|---|
| Retail install | `StarCraft/` (Blizzard CASC storage) |
| 32-bit game | `x86/StarCraft.exe` |
| 32-bit SHA-256 | `32dbbdd001dd381cb1b3a719b7ad1fc918a9d4bc99661c675e00254efecca827` |
| 64-bit launcher/game | `x86_64/StarCraft.exe` |
| 64-bit SHA-256 | `ce3ab05dc9a6aa35418947e5d95e19651ad6fbcd5e2c1856cd729c5b7c31281c` |
| Version | 1.23.10.13515 |
| Data | `Data/data` + `Data/indices` (CASC), ~5.4 GB |
| Maps | `Maps/*.scm` (MPQ archives containing `staredit\scenario.chk`) |
| Toolchain | Ghidra 12.1.4 (JDK 25), capstone 5, pefile, CascLib, StormLib |

The 32-bit image is the classic engine used for the gameplay reconstruction.
Its image base at runtime was `0x79fa0000`; the on-disk `.text` is encrypted
(entropy ≈ 8.0 bits/byte), so static analysis of code is only meaningful after
a read-only snapshot of the running process.

### Runtime image snapshot

A live 32-bit `StarCraft.exe -launch` process (Wine/Proton) was mapped at
`0x79fa0000`. A read-only `pread` of `/proc/<pid>/mem` produced a 17,620,992
byte contiguous image. The image was rebuilt as a PE with
`PointerToRawData == VirtualAddress`, `FileAlignment == SectionAlignment`, and
`ImageBase == 0x79fa0000` so every recovered RVA equals its runtime VA.

| Artifact | SHA-256 |
|---|---|
| Raw memory image `base_79fa.bin` | `7088dd8bc3634142e5b54ddc2703f7f597938830ba5dd60ec14a9efa311d7f8b` |
| Rebuilt analysis PE `sc_dump.exe` | `9f583729f6e7a09a11889931e9471f3de09f770a35b635bed197dc205cd12059` |

`sc_dump.exe` was imported into a persistent Ghidra 12.1.4 project. The
project contains **56,535 functions** (including RTTI/thunks). The executable
was never modified; the snapshot is not redistributed.

## Recovered data files

CASC extraction used `tools/casc_extract.cpp` (CascLib). The bundled fixtures
are byte-identical to the retail files:

| File | Bytes | SHA-256 |
|---|---:|---|
| `arr\units.dat` | 19,876 | `da2ee6f116b77329048ec6be9e02721a527fde23d4b05b1b1336e634490abcba` |
| `arr\weapons.dat` | 5,460 | `80de165015519f9bed46ea8858195c2d2d0120b65b9e55e1c0a50b19135f4b62` |
| `arr\flingy.dat` | 3,135 | `89013fa6d81edad96b68efcfc0bb5342f1c7fa2ef7a0880a71b0169eab13d1ad` |
| `arr\orders.dat` | 4,158 | `0b79cc64f1cd8b23c77419bdf1fb39fb91cde1359c1ad9cb73884d5cceb8f7c9` |
| `arr\techdata.dat` | 836 | `98f9805ebc8efc707492b7406871298d39d6e94931458aca1f82feaf18a271af` |
| `arr\upgrades.dat` | 1,281 | `787ff7dff69c0a0bb417ce1aabdcc11fa8b0959f77372b635c4882bdc3adb59b` |
| `arr\sprites.dat` | 3,229 | `3e5ea29bf9937558d0a3a302cd85852c220cd126ec049c0da176c77739bb4a71` |
| `arr\images.dat` | 37,962 | `6b6123e2cc6771d96f914fc9bb92d34da17cc3319dc02e2db0cd10b4f3d2efdc` |
| `scripts\iscript.bin` | 40,482 | `cd35e04eaa8ea95cbbccc980bfaf4cae236f4b18b620dfb4a765822ec32c1cd0` |
| `scripts\aiscript.bin` | 62,050 | `635433e546998e05529a9d6acc806c06b528c7ce5075a206f6f2c8de44ffed6e` |
| `scripts\bwscript.bin` | 26,458 | `15a1a5b3fb4f2cab2e7b7b507ae04d4e2ef097c77081cfeb950d9a2e4ebee382` |
| `rez\stat_txt.tbl` | 40,290 | `4cbd3b355bd9e9732abbfb7502301c11d2d923090925f5af642824ff98fbfd0c` |
| `TileSet\platform.vf4` (test map) | 182,752 | `22185f7ad524acbdbf3387736377fd473cbe1e02611dfeec521ecd523b0c9e0c` |

### Table layouts

All DAT files are column-major. `src/data.rs` and `src/tables.rs` encode the
exact column offsets; `tools/oracle.py` independently reads the same bytes
with its own offsets.

* `units.dat`: 228 units; flingy u8, turret u16, shields u8/u16, HP i32,
  elevation, weapons, flags u32, acquisition/sight, armour upgrade, size,
  armour, placement `Point16`, dimensions `Box16`, portrait, costs, build
  time, requirements, supply, score, StarEdit flags.
* `weapons.dat`: 130 weapons; label, flingy, target flags, min/max range,
  damage upgrade/type/behaviour, lifetime, hit type, splash radii, damage,
  bonus, cooldown, bullet count, angles, offsets, error string, icon.
* `flingy.dat`: 209 entries; sprite, top speed (1/256 px/tick), acceleration,
  halt distance, turn rate, movement type.
* `orders.dat`: 189 entries × 22 bytes: label u16, 15 u8 flags
  (targets-enemies, background, turret-valid, interruptible, queueable,
  obstructible, weapon, tech, animation), highlight i16, dependency u16,
  target order u8.
* `techdata.dat`: 44 entries; mineral/gas/time/energy u16, unknown u32,
  icon u16, label u16, race u8, flags u16.
* `upgrades.dat`: 61 entries; mineral base/factor, gas base/factor, time
  base/factor, unknown, icon, label, race, max level, Brood War flag.
* `sprites.dat`: 517 entries (387 selectable + 130 non-selectable); image,
  health-bar size, visible, selection circle and vertical position.
* `images.dat`: 999 entries; GRP filename, directional/clickable/iscript/
  visible flags, modifier, colour shift, iscript id and six overlay GRPs.
* `stat_txt.tbl`: 1,539 strings. Tech and upgrade labels are stored one
  greater than the string index; the engine subtracts one.

### Loaded tables in the runtime image

The game expands the column-major files into per-field arrays in `.data`.
Scanning the snapshot for the exact 228-byte / 130-entry columns located them
and confirmed the parser:

| Table | Runtime VA |
|---|---|
| unit flingy / turret | `0x7ab2de50` / `0x7ab2df38` |
| unit has-shield / shield / HP | `0x7ab2e800` / `0x7ab2e8e8` / `0x7ab2eab0` |
| unit ground/air weapon | `0x7ab2f580` / `0x7ab2f750` |
| unit flags / sight | `0x7ab2fa08` / `0x7ab2fe80` |
| unit armour upgrade / size / armour | `0x7ab2ff68` / `0x7ab30050` / `0x7ab30138` |
| unit mineral / gas / build time | `0x7ab318c8` / `0x7ab31a90` / `0x7ab31c58` |
| unit supply provided/required | `0x7ab320d0` / `0x7ab321b8` |
| weapon label / flingy / target flags | `0x7ab332a0` / `0x7ab333a8` / `0x7ab33638` |
| weapon range / type / cooldown | `0x7ab33948` / `0x7ab33bd8` / `0x7ab34320` |
| order labels / first u8 field | `0x7ab2a090` / `0x7ab2a210` |
| order interruptible flag (field 5) | `0x7ab2a5d0` |
| order queueable flag (field 7) | `0x7ab2a750` |
| order highlight / dependency | `0x7ab2ad50` / `0x7ab2aed0` |
| order target-order | `0x7ab2b050` |
| acquisition range (computed) | `0x7ab2fd98` |
| tech mineral/gas/time/energy | `0x7ab2db08` / `0x7ab2db60` / `0x7ab2dbb8` / `0x7ab2dc10` |
| tech unknown/icon/label/race/flags | `0x7ab2dc68` / `0x7ab2dd18` / `0x7ab2dd70` / `0x7ab2ddc8` / `0x7ab2ddf4` |
| upgrade cost columns (9) | `0x7ab32d60`–`0x7ab33160`, stride `0x80` |
| upgrade race / max level / Brood War | `0x7ab331dc` / `0x7ab3321c` / `0x7ab3325c` |
| flingy sprite/speed/accel/turn | `0x7ab1ffa0` / `0x7ab20148` / `0x7ab20490` / `0x7ab20980` |

## Damage multiplier table

Exact 5×5 table at virtual address `0xf0757c` (file offset `0xb05d7c`) in the
32-bit on-disk image, also present in the runtime snapshot:

```
row 0  Independent  = 0,   0,   0,   0,   0
row 1  Explosive    = 1,   0, 128, 192, 256
row 2  Concussive   = 2,   0, 256, 128,  64
row 3  Normal       = 3,   0, 256, 256, 256
row 4  Ignore armor = 4,   0, 256, 256, 256
```

The helper at runtime `0x7a242e60` indexes `table[1 + type*5 + size]`, i.e.
base `0x7aaa7580` (`table + 4`), multiplies the damage and shifts right by 8.

## Recovered damage routine

Routine `0x7a13ce30` (target `CUnit` in ECX) was decompiled from the snapshot.
With the helper at `0x7a13cf90` it implements:

1. early-out when HP == 0 or invincible;
2. hallucination doubles damage (`0x7a155d00`, status bit 30);
3. `damage = weapon_damage/divisor + acid_spore_count*256`
   (`0x7a153ba0`);
4. floor at `0x80` (half a point);
5. Defensive Matrix absorbs first (`0x7a153be0`/`0x7a172300`, `CUnit+0x112`);
6. shields absorb when `shield_points >= 0x100`; Plasma Shields upgrade
   (`upgrade 15`) is subtracted unless damage type is `IgnoreArmor` (4);
7. armour (`units.dat` armour + armour upgrade) is subtracted;
8. size multiplier from the table above;
9. HP damage is floored at `0x80` again only when no shield damage was dealt;
10. HP damage is applied through `0x7a13d010`; shields are subtracted
    separately.

This confirms the ordering **Matrix → shields → armour → size → floor → HP**
and the engine's `resolve_damage_status`.

## Order queue

The queue lives in `CUnit`:

* current order byte: `+0x4d`
* queue head/tail: `+0x74`/`+0x78`
* queued-order count: `+0x84`

`FUN_7a156cd0` (append) and `FUN_7a157534` (prepend) increment the count only
for orders whose `orders.dat` highlight field is not `-1`; both check the
count against 8. `FUN_7a1576f0` drains/replaces orders, tests the
interruptible flag at `0x7ab2a5d0` (orders.dat field 5) and rejects a ninth
entry with `"Unit order queue exceeded"`.  The loader pads each order flag
field to `0xc0` bytes, so field 7 (`can_be_queued`) lands at `0x7ab2a750`;
the earlier ledger row labelled `0x7ab2a5d0` as queueable, but the expanded
bytes match field 5 exactly.

Rust: `MAX_ORDER_QUEUE = 8`, `Order` and `SimUnit.queue`.

## Movement

`FUN_7a109190`/`FUN_7a109b10` initialise a flingy from `flingy.dat`:
sprite, top speed, acceleration, turn rate, movement type and position. The
velocity helpers (`FUN_7a109a70`, `0x7a109580`, `0x7a10c7e0`, `0x7a10c7b0`)
use the direction cosine/sine table at `0x7aaa31b8`, speed in 1/256 px/tick,
and clamp acceleration to top speed.

Walking ground units carry the sentinel `top_speed = 1` in `flingy.dat`. Their
real speed comes from the iscript `Walking` animation's `move <speed>` opcode
(opcode 41; `setflspeed` is opcode 55). `src/iscript.rs` decodes the script
table and caches the mean move speed:

| Unit | iscript | speed (1/256 px/tick) |
|---|---:|---:|
| Marine | 78 | 1024 (4 px/tick) |
| SCV | 84 | 1280 (5 px/tick) |
| Zergling | 31 | 1280 (5 px/tick; animation alternates 2–9) |
| Zealot | 163 | 1024 |
| Dragoon | 153 | 1280 (move opcodes average 5 px/tick) |
| Command Center | 102 | 0 (no walking animation) |

Rust: `GameData::unit_speed`, applied in `World::speed_of`.

## Target acquisition

`FUN_7a150d80` computes the per-unit acquisition byte:

```
acquisition_px = max(target_acquisition_range * 32,
                     ground_weapon.max_range,
                     air_weapon.max_range)
```

Rust: `GameData::target_acquisition_px`.

## Build tree

`units.dat` has no live prerequisites (the field is `0xffff` for all stock
units). The executable's `unit_can_build` switch contains them; the engine's
table in `src/tech.rs` was built from that switch and cross-checked against
OpenBW's independent implementation of the same switch. Representative
entries:

| Unit | Producer | Requirements |
|---|---|---|
| Marine | Barracks | — |
| Ghost | Barracks | Covert Ops, Academy |
| Goliath | Factory | Armory |
| Battlecruiser | Starport | Physics Lab |
| Zergling | Larva | Spawning Pool |
| Lurker | Hydralisk | Lurker Aspect tech |
| Guardian | Mutalisk | Greater Spire |
| Zealot | Gateway | — |
| Dragoon | Gateway | Cybernetics Core |
| Carrier | Stargate | Fleet Beacon |
| Barracks | SCV | Command Center |
| Factory | SCV | Barracks |
| Spire | Drone | Lair |
| Gateway | Probe | Nexus |
| Robotics Facility | Probe | Cybernetics Core |

Research and upgrade building gates are in `tech::tech_buildings` and
`tech::upgrade_buildings`; second/third-level gates (Science Facility, Lair →
Hive, Templar Archives, Fleet Beacon) are in
`tech::upgrade_level_requirement`.

## Time units

* `units.dat` build time is 1/15 s. `build_ticks = ceil(time * 24 / 15)`.
* `techdata.dat` research time is 1/15 s.
* `upgrades.dat` time is 1/60 s. `upgrade_ticks = ceil(time * 24 / 60)`.
* Logical ticks run at 24 Hz.

## Maps and terrain

`tools/mpq_extract.cpp` unwraps `staredit\scenario.chk` from `.scm`/`.scx`.
`src/map.rs` parses DIM, ERA, MTXM, UNIT, MRGN, STR, OWNR, SIDE and TRIG.

Important correction verified against the CHK bytes: **UNIT x/y are pixel
coordinates** (not tiles), and the unit record is
`u32 class, u16 x, u16 y, u16 type, u16 relation, u16 special, u32 valid,
u8 owner, u8 hp%, u8 shield%, u8 energy%, u32 resources, u16 hangar,
u16 state, u32 unused` (36 bytes). MRGN coordinates are tiles.

Walkability comes from `TileSet\*.vf4`: 32 bytes per megatile, 16 `u16`
flags (4×4 minitiles). Bit 0 = walkable, bit 1 = middle height, bit 2 = high
height. The megatile index is `tile_id >> 4`. The engine builds a 4×4
minitile grid per 32×32 tile; `src/path.rs` runs octile A* over those
8-pixel cells with unit footprints and building obstacles.

## Triggers

A `TRIG` record is 2,400 bytes: 16 conditions × 20 bytes, 64 actions × 32
bytes, 4 execution-flag bytes and 28 enabled bytes. `src/trigger.rs` parses
this layout and implements the classic condition ids (command, bring, switch,
elapsed time, opponents, always) and action ids (victory, defeat, preserve,
wait, kill/remove, set resources, create unit, order, set unit resources).
StarCraft Remastered adds a few extended ids; unknown ids are ignored rather
than guessed.

The `triggers\*.trg` files and campaign CHKs are the reference inputs. Melee
`triggers\melee.trg` uses condition type 2 (command) + action type 2 (defeat)
and condition type 14 (opponents) + action type 1 (victory), which the
implementation handles.

## Reproduction

```bash
cargo test
python3 tools/oracle.py fixtures
python3 tools/extract_damage_table.py "/path/to/StarCraft/x86/StarCraft.exe" fixtures
SC_DATA_DIR=/path/to/extracted/arr \
SC_EXE="/path/to/StarCraft/x86/StarCraft.exe" \
SC_MAP_DIR=/tmp/sc_maps SC_TILE_DIR=/tmp/tiles/TileSet cargo test
# -> 212 tests, 0 failed; the oracle test covers 74,880 damage cases
SC_INSTALL="/path/to/StarCraft" cargo test --test retail
# -> 7 retail CASC/MPQ/campaign tests, 323/323 maps + 68 campaign CHKs
# -> unit/weapon/flingy/order oracle covers every scalar column
# -> fixture provenance compares the DATs, scripts, stat_txt.tbl and melee.trg

# Read-only check of the game's own expanded arrays against fixtures.
python3 tools/verify_runtime_tables.py /path/to/sc_dump.exe
# -> 73/73 arrays match

# Recovered order dispatch table (189 rows, lanes and handler addresses).
cargo run --quiet --bin sc_orders
# -> 189 rows, 165 direct handlers, 40 every-tick, 128 carry-8, ...
# -> 10 target-order remaps
# The test suite additionally diffs every row's handler VA and lane against
# docs/evidence/order_dispatch.tsv and docs/evidence/order_dispatch_vm.txt,
# and checks all 137 unique handler bodies have pseudocode and one
# transcribed Rust routine each.

# The 137 transcribed handler routines and their evidence manifest.
python3 tools/gen_handlers.py
cargo run --quiet --bin sc_handlers
# -> 137 routines, 165 direct-cased rows, 19 families, 137 bodies
cargo test --test handlers
# -> 9 passed; byte-for-byte body, callee, write and lane diffs enforced

# Recovered tile-level region graph for an extracted map.
cargo run --release --bin sc_regions -- Bottleneck.chk platform.vf4

# Trigger condition/action ids actually used by the shipped maps.
python3 tools/trigger_ids.py /tmp/sc_maps
# -> conditions 1..23, actions 1..57
```

Real-map match:

```bash
./mpq_tool "Maps/(2)Bottleneck.scm" extract bottleneck.chk 'staredit\scenario.chk'
./casc_tool "/path/to/StarCraft" extract tiles 'TileSet\platform.vf4'
cargo run --release --bin sc_match -- --chk bottleneck.chk \
  --vf4 tiles/TileSet/platform.vf4 --races terran,protoss --ticks 20000
```

## Second reconstruction pass (projectiles, vision, AI scripts)

The first pass stopped at the rules listed in the original gap list.  The
second pass closed the following items.

### Weapon ids are not the label ids

The `weapons.dat` label column is not a one-to-one weapon id map.  Several
hero rows share a stock label and the labels for the later rows are shifted
relative to the row order.  The ids used by the engine therefore come from the
mechanics columns and the unit weapon columns.  The identifying rows are:

| Row | Mechanics signature | Actual weapon |
|---:|---|---|
| 6 | 125 damage, radial splash, unit 13 carries it | Spider Mines |
| 29 | 20 damage, BulletType 1 | Longbolt Missile |
| 30 | 260 damage, hit type 15 (Yamato) | Yamato Gun |
| 31 | 600 damage, hit type 5 (nuclear) | Nuclear Strike |
| 32 | hit type 4 (lockdown) | Lockdown |
| 33 | hit type 8 (EMP) | EMP Shockwave |
| 34 | hit type 9 (irradiate) | Irradiate |
| 56 | hit type 6 | Parasite |
| 59 | hit type 13 | Dark Swarm |
| 60 | hit type 11 | Plague |
| 82 | 100 damage, unit 85 (Scarab) carries it | Scarab |
| 84 | 14 damage, BulletType 3, 60 lifetime | Psionic Storm |
| 109 | 25 damage, unit 103 (Lurker) carries it | Subterranean Spines |

`src/ids.rs` now uses these row indices.  The old values (`YAMATO_GUN = 34`,
`NUCLEAR_STRIKE = 29`) were label ids and made Yamato cast the Irradiate
weapon.

### Travelling bullets

`FUN_7a1074c2` is the `CBullet` dispatcher reached from the string
`"CBullet: Damage"` at runtime `0x7a8d4918` (xref at `0x7a107a04`).  It
switches on `bullet_type` at `CBullet+0x60`; the table at `0x7ab33d70` has the
ten behaviours.  `src/projectile.rs` and `World::spawn_projectile` implement:

| `bullet_type` | Behaviour |
|---:|---|
| 0 | fly straight to the last known position |
| 1 | follow the target unit each tick |
| 2 | appear at target unit (instant) |
| 3 | persist at target position; hit every 7 ticks |
| 4 | appear at target position (instant) |
| 5 | appear at source unit (instant) |
| 6 | self-destruct the source (Scourge/Infested Terran) |
| 7 | bounce up to three times; divisor 1, 3, 9 |
| 8 | fly to a small near-target offset |
| 9 | extend to `max_range + 20` along the firing heading |

Impact is resolved by the `hit_type` column.  The engine implements normal,
radial splash, enemy-only splash, air splash, lockdown, nuclear, parasite,
broodlings, EMP, irradiate, ensnare, plague, stasis, Dark Swarm, consume,
Yamato, restoration, Disruption Web, corrosive acid/acid spores, mind control,
feedback, optical flare and Maelstrom.  Splash uses the divisors 1/2/4 for the
inner/medium/outer radii; burrowed units only take the inner ring.

The retail nuclear damage is `max(500, 2/3 * (max_hp + max_shields))`, not the
stored 600.  It is applied through the normal shield/armour pipeline by
replacing the weapon damage for the blast.

Bullet speed comes from the weapon's `flingy.dat` row (`top_speed`, 1/256
pixels/tick).  The Halo Rocket 80-active-bullet guard is used as the general
projectile cap.

Miss chance is reconstructed from the retail defensive rules: 255/256 under
Dark Swarm, 119/256 when the target stands on higher terrain, and 255/256 for
hallucinated shooters (the shot is visible but deals no damage).

### Vision, cloak and detectors

`src/vision.rs` keeps the two per-player tile masks the retail engine uses:
*visible* and *explored*.  Ground sight cannot see over a higher `vf4`
elevation; air sight ignores elevation.  `World::enable_fog` turns rules-level
fog on and targeting consults `World::can_see_unit`.

Cloak detection follows the retail `unit_calculate_detected_flags` rules:

* a cloaked or burrowed unit is invisible unless a detector owned by the
  observer is in range;
* detector buildings use the hardened 7-tile radius, mobile detectors use
  their `units.dat` sight range;
* Scanner Sweep, Parasite and any status effect set `revealed_ticks`;
* Ghost cloaking drains 10/256 energy per tick and Wraith cloaking 13/256;
  cloaking suppresses the +8/256 regeneration (net 18/256 and 21/256), and
  the cloak drops when the fixed-point pool cannot pay;
* hallucinated detectors do not detect.

### iscript walking cycle

This pass implemented the Walking animation; the third pass adds the full
opcode VM.  The Walking animation is interpreted rather than averaged.
`Iscript::walk_cycle` executes the `wait`/`move`/`goto`/`setflspeed`/`end`
opcodes for the Walking animation and returns the per-tick speed sequence.
`SimUnit.anim_tick` indexes it.  This recovers the Zergling's alternating 2–9
px/tick cycle.  Scripts whose Walking animation contains no positive `move`
(for example the SCV) fall back to the flingy top speed.

### Campaign AI scripts

`src/aiscript.rs` parses both table layouts:

* `aiscript.bin` — 16-byte entries: 4-byte id, 32-bit code offset, 32-bit
  `stat_txt.tbl` label, 32-bit flags; 293 shipped entries, 156 of which point
  into `bwscript.bin` and therefore have no local code;
* `bwscript.bin` — 8-byte entries: 4-byte id and 32-bit offset; 156 entries.

The opcode operand widths were corrected against the decompiled interpreter
at `0x7a1e0b20` (xref to the AI globals `0x7ab68700`/`0x7ab68704`): unit ids
are **16-bit**, not the 8-bit ids in the older PyMS documentation.  For
example `build` is `byte count, u16 unit, byte priority` (the decompile reads
`*(ushort *)(...)` and rejects ids above 227).  `CampaignAi` executes the
town-management subset: start town, build, train, upgrade, tech, attack-party
add/do, wait, goto, time jump, resource jump and stop.  Commands outside that
subset are skipped, never guessed.

### Hyperspace/transport notes

The existing 8-slot queue now reads the queueable metadata from `orders.dat`
(`highlight != -1`).  Non-queueable orders interrupt and clear the queue; a
ninth queueable order is rejected, matching the recovered `0x7a156cd0` /
`0x7a157534` checks.

### Triggers

Classic trigger evaluation now uses seconds for condition 12 (not ticks),
accumulated resources for condition 4, per-unit death counters for condition
8 and the switch state for conditions 9/11.  Action 26 supports set/add/
subtract and the minerals/gas/both mask; action 40 sets a switch; action 38
moves a location; action 13 sets the countdown timer.  The shipped
`triggers\*.trg` files exercise these ids.

## Third reconstruction pass (iscript VM, AI interpreter, triggers, regions)
### iscript animation VM

The dispatcher was recovered at runtime `0x7a1134f0`.  It is the
`CImage::update`/animation executor: the start routine `0x7a1148f0` selects
`header + 8 + animation*2`, resets the image state and calls the dispatcher
immediately, which is why a Walking script's first `move` applies on the tick
the order is issued.  The dispatcher switches on `0x00..0x44` and was
cross-checked against the shipped instruction table.

`src/iscript.rs` now implements:

* `AnimState` (pc, wait, frame, direction, flip, call stack, move speed,
  flingy speed, block/ignore flags, repeat-attack flag, signal, step count);
* every opcode's operand layout, including the variable-length
  `playsndrand` (0x19) and `attackmelee` (0x1c);
* gameplay events: `playfram`, `wait`/`waitrand`, `goto`, `call`/`return`,
  `move` (0x29), `setflspeed` (0x37), `setfldirect`, turn opcodes,
  `setflipstate`, `attack`/`attackwith`/`attackmelee`/`domissiledmg`/
  `dogrddamage`, `castspell`, `sigorder`, `orderdone`, `end`, `ignorerest`,
  conditional jumps (`randcondjmp`, `pwrupcondjmp`, `trgtrangecondjmp`,
  `trgtarccondjmp`, `curdirectcondjmp`, `liftoffcondjmp`);
* visual events for the overlay/sprite/sound opcodes (recorded, not rendered).

`GameData::unit_iscript_id` walks `units.dat → flingy.dat → sprites.dat →
images.dat`.  `World` starts `Init` on spawn, runs the VM every logical tick,
uses the `Walking` VM's `move` value for speed, starts `GndAttkInit`/
`AirAttkInit` and `GndAttkRpt`/`AirAttkRpt`, fires the weapon on the
animation's release opcode (the Marine fires on tick 4), starts `CastSpell`
and releases the spell on `castspell`, and returns to
`WalkingToIdle`/`GndAttkToIdle`.  Parent units whose visible attack animation
lives on a turret subunit (for example the Missile Turret, iscript 119, whose
parent animation is only `wait 125; goto`) are detected by scanning the
animation for a release opcode; those units keep the order-tick fallback.

The `walk_cycle` table and the VM are checked against each other for every
shipped iscript id, and every shipped animation is decoded to the known
0x00..0x44 instruction set.

### Campaign AI interpreter

The retail interpreter at `0x7a1e0b20` was decompiled with a 900 s provider
budget.  The first-pass operand table had two wrong rows:

| Opcode | Row | Correct operands (decompile) |
|---|---|---|
| 7 `upgrade` | was `byte, byte, byte` | `byte level, u16 upgrade, byte priority` |
| 8 `tech` | was `byte, byte` | `u16 tech, byte priority` |

Every other row was checked against the switch: unit operands are 16-bit
(`u16`), `move_dt`, `set_gencmd`, `make_patrol`, `give_money`, `enter_bunker`,
`exit_transport`, `nuke_location`, `harass_location`, `implode`, `guard_all`,
`kill_thread`, `killable`, `wait_finishattack`, `quick_attack`,
`junkyard_dog`, `fake_nuke`, `disruption_web`, `recall_location`,
`create_nuke`, `help_iftrouble` and `if_towns` take no operands;
`place_guard` is `u16 unit, byte location`; `scout_with` is `u16`;
`define_max`/`train`/`do_morph`/`wait_force` are `byte, u16`.

`AiScript` code slices are now bounded by the script table offset as well as
the next code offset.  The previous `bwscript.bin` slicer ran past the end of
the code region into the table and produced phantom opcodes 114/132/138/140/
148/165/207.  After the fix every instruction in all 293 `aiscript.bin` and
156 `bwscript.bin` entries decodes to a known 0..=113 opcode.

`CampaignAi` implements every shipped opcode family: town start/area town,
`build` (buildings are constructed, non-buildings are trained/morphed),
`train`/`wait_train`/`wait_force`/`do_morph`, upgrade/tech, attack
clear/add/prepare/do, defense build/use/clear (gg/ag/ga/aa), expansion,
capture-expand, bunkers, turrets, farms, guard placement, patrol, transports,
nukes, harassment, scouting, `multirun`/`expand` child threads, AI jumps
(race/groundmap/enemy/resources/random/time/rush/region/owned), shared vision,
alliance and enemy marking, and the Remastered-era no-op commands.  A
`start_town` engine-side manager keeps workers near the requested maximum and
runs the supply policy; the script thread remains responsible for its own
build/train requests.  A real `TMCu` run builds supply, trains 26+ units and
executes without skipped opcodes.

### Triggers corrected

The classic CHK ids are now exactly the shipped list:

* Conditions 0 no condition, 1 countdown, 2 command, 3 bring, 4 accumulate,
  5 kill, 6 most units, 7 most at location, 8 most kills, 9 highest score,
  10 most resources, 11 switch, 12 elapsed time, 13 mission briefing,
  14 opponents, 15 deaths, 16 least units, 17 least at location,
  18 least kills, 19 lowest score, 20 least resources, 21 score, 22 always,
  23 never.
* Actions 1 victory, 2 defeat, 3 preserve, 4 wait, 5/6 pause, 7-12
  presentation, 13 set switch, 14 countdown, 15/16 run AI script,
  17-21 leaderboards, 22-25 kill/remove, 26 resources, 27 score, 28-37
  presentation/leaderboard, 38 move location, 39 move unit, 40 leaderboard
  greed, 41 next scenario, 42 doodad, 43 invincibility, 44 create unit,
  45 set deaths, 46 order, 47 comment, 48 give units, 49-53 modify unit
  HP/energy/shields/resources/hangar, 54/55 timer, 56 draw, 57 alliance,
  58/59 debug.

The earlier table had switch/countdown/deaths on the wrong ids; the new
implementation reads the switch number from the condition modifier byte and
uses comparison 2 = set, 3 = cleared.  Trigger wait times are milliseconds.
Condition and action ids 0..=23 and 1..=59 are exercised by tests, including
AI-script requests, unit-property actions and alliance changes.

### Region map and path tie-breaking

`path::RegionMap` decomposes the 8-pixel walk grid into 8-connected regions
using the same 4×4-minitile base as the retail builder `0x7a302c10`.  A
destination in a different region is rejected before A*, and the AI's
`region_size` command uses the recovered region count.  Diagonal corner
cutting is forbidden, and the path is simplified with the existing
line-of-sight pass.  (The first pass used a deterministic `(f, g, x, y)`
tie-break; the fourteenth pass replaces it with the heap policy recovered from
`0x7a305590`.)

### Order families

`orders.dat` has 189 rows, including internal AI/hero/doodad orders.  The
engine classifies each row into a behaviour family (`OrderClass`) and exposes
`World::issue_order_id(unit, id, target, pos)` which translates a row into the
typed order state.  Orders that need a parameter the caller did not supply
return `Unsupported` instead of guessing.  The high-level ids were corrected
to the shipped rows (`Move=6`, `AttackUnit=8`, `AttackMove=13`, `Hold=106`,
`Patrol=151`, `Gather=78`, `Train=37`, `Research=74`, `Upgrade=75`,
`EnterTransport=90`, `Unload=110`, `Burrow=115`, `Unburrow=117`,
`Siege=97`, `Unsiege=98`).

## Fourth reconstruction pass (all shipped maps, protected CHK recovery)

All 323 `.scm`/`.scx` files in the retail `Maps/` tree were extracted
(`tools/mpq_extract.cpp`) and loaded through `src/map.rs`.  The first run
loaded 249/323; the remaining 74 were not simulation gaps but two map-format
details used by the distributed ladder packs.

### Protected ladder maps

Many ladder maps have no listfile and are wrapped by anti-edit tools:
section order is shuffled, some optional section names are corrupted
(`UPGx`, `PTEx`, `PUNl`, `FORc`, `TlLE`) and the file may begin with a junk
header whose declared size is impossible.  The sections StarCraft needs
(`DIM`, `ERA`, `OWNR`, `SIDE`, `MTXM`, `MASK`, `UNIT`, `MRGN`, `STR`, `TRIG`)
are still present with valid payload lengths.

`Scenario::parse` now walks the section table normally and, when a header is
damaged or a required section is missing/invalid, rescans the file for the
four-byte name and accepts only a candidate whose payload length passes the
section's own predicate (`DIM` = 4 bytes with a legal size, `MTXM` >=
`width*height*2`, `UNIT` multiple of 36, `MRGN` multiple of 20, `TRIG`
multiple of 2400, ...).  This is section resynchronisation, not inference: no
gameplay data is guessed and ordinary maps keep the sequential layout.

### `ERA` high bits

Remastered-era maps store flags in the high bits of the two-byte `ERA`
section.  The tileset is the low three bits: `CircuitBreakers1.0` carries
`0x00f1` and is a Space Platform map (`0xf1 & 7 = 1`); `Destination` carries
`112` and is Badlands (`& 7 = 0`); `Fighting Spirit 1.3` carries `20` and is
Jungle (`& 7 = 4`).  `Scenario::era` masks with `0x07`.

### Campaign maps and free bases

`World::from_scenario` used to create a stock town hall and four workers for
every player that had none.  That is correct for melee maps, which contain
only start locations and resources, but it injected free bases into campaign
and use-map scenarios.  Stock population now runs only when the scenario
places no non-resource unit for any active player.

### Refinery/geyser lifecycle and Zerg melee supply

Two economy gaps surfaced while running all three races through full matches.

* A melee Zerg start is a Hatchery, four Drones **and one Overlord**.  The
  Hatchery only provides one supply, so without the Overlord the four
  starting Drones already exceed the cap and no Zerg economy can grow.
  `World::from_scenario` now spawns it.
* Gas buildings are not free-standing.  `World::find_build_spot` returns the
  geyser tile for `Refinery`/`Assimilator`/`Extractor`, and on completion the
  gas pool moves from the geyser to the building and the geyser is hidden.
  `order_gather` resolves a geyser order to the completed gas building and
  treats it as a gas resource; workers return 8 gas per trip.  Before this
  change the refinery had `resource_amount = 0`, so gas could never be
  harvested even though the DAT tables and the damage/economy paths were
  present.

The deterministic AI now builds one gas building, assigns up to three
workers to it and keeps the rest on minerals.  A real `Bottleneck` match
reaches a completed Refinery and a positive gas stock; the controlled
`refinery_takes_over_a_geyser_and_workers_gather_gas` test covers the
completion transfer and the return trip.

### Order dispatcher table

`FUN_7a178340` is the per-unit order executor.  It is reached with the
`CUnit` in ECX and switches on the current order byte at `CUnit+0x4d`; a
second switch in the same routine runs the eight-tick "order carry" pass
(`CUnit+0x85` counts down from 8).  The decompiled switch has direct cases
for **168 of the 189** shipped order ids and dispatches them to the handler
functions listed in `docs/evidence/order_dispatch.tsv` (order id, name from
`stat_txt.tbl`, runtime handler VA).  Examples:

| id | order | handler |
|---:|---|---|
| 7 | Attack | `0x7a131930` |
| 8/9 | Attack Unit | `0x7a1537a0` (id 9; id 8 shares the family) |
| 13 | Attack Move | second-pass virtual call (`guard_check_icall`) |
| 15 | Build | `0x7a17cd00` |
| 78 | Harvest | `0x7a17d7c0` (nested pass) |
| 90 | Enter Transport | `0x7a14c1e0` |
| 106 | Hold Position | `0x7a1642e0` |
| 143 | Yamato Gun | `0x7a16bea0` (generic cast/tech handler) |
| 151 | Patrol | `0x7a160fb0` |

The remaining ids are handled by the nested pass, fall through to the
next case, or are asserted/ignored by the engine.  The mapping is used to
audit `sim::order_class`; it does not by itself prove the per-id semantics,
which is why the gap below remains.

`sim::order_class` now follows the shipped row names for the rows the first
pass had folded into broad ranges: Vulture Mine (18/19) is `Special`, Drone
Attack Unit (26), Carrier Attack (20) and Turret Attack (21) are `Attack`,
gas Harvest rows (79/80/88/89/91) are `Harvest`, gas Return rows
(81/82/83) are `ReturnCargo`, and Pick Up (92) is `Transport`.
`issue_order_id` returns `Unsupported` for the families that need a unit
type or tech id (build/train/research/upgrade/cast/morph) instead of
silently turning them into `Stop`; the typed `train`/`research`/`upgrade`/
`cast`/`build` APIs are the supported entry points.

### Coverage

`src/bin/sc_campaign.rs` parses, builds and steps every extracted map for
1,200 logical ticks with the deterministic AI, trigger VM and any
`Run AI Script` actions:

```text
$ ./target/release/sc_campaign --maps /tmp/sc_maps \
    --tiles /tmp/tiles/TileSet --ticks 1200
campaign coverage: 323/323 maps simulated, 0 failed, 46685 placed units,
9835 triggers on 322 maps, 14 AI script starts, 1200 ticks each
```

The run covers 8 tilesets, `128x128`/`256x96`/`96x128`/`192x192` maps,
`2`-`8` player melee layouts, the five Enslavers campaign maps (21-30
triggers each) and 322 maps with triggers.

## Fifth reconstruction pass (order dispatcher and carry cadence)

`FUN_7a178340` was decompiled in full and saved as
`docs/evidence/order_dispatch_vm.txt`; the 88 handlers that Ghidra recognises
as procedure entries were decompiled in one batch, then a second pass created
functions at the remaining switch-target addresses and decompiled 44 more.  A
final pass created and decompiled the five targets that were still missing
(`0x7a17d780`, `0x7a17d7c0`, `0x7a17da30`, `0x7a17db50`, `0x7a1dbef0`; see
`docs/evidence/order_handlers_missing5.txt`).  **All 137 unique handler
addresses referenced by `src/orders.rs` now have pseudocode**; the two
`guard_check_icall` rows (Hover and Attack Move) are virtual sites with no
fixed address.  Entry, callees and `CUnit` bytes written are in
`docs/evidence/order_handlers_summary.tsv` (137 rows), and the full text is
`docs/evidence/order_handlers_decomp.txt.gz`.

### Three dispatch layers

The routine is not one flat switch.  It has three layers:

| Layer | Cases | Behaviour |
|---|---:|---|
| fast switch | 4 (`0`, `0x30`, `0x81`, `0xae`) | handler runs and returns every tick |
| worker/attack switch | 36 | handler runs every tick; falls through to the counter |
| carry switch | 128 | runs only when `CUnit+0x85` reaches zero |

4 + 36 + 128 = **168 direct cases**, matching the earlier audit.  The
remaining 21 rows are resolved before or outside the dispatcher (`8` aliases
`9`, `78` resolves to the mineral/gas worker rows, production timers never
enter it, and the campaign/doodad rows have no case).  `src/orders.rs`
contains an explicit entry for all **189** rows with the stat_txt name, the
handler VA where a direct case exists, the recovered lane and the behaviour
class.  There is no range folding left in `order_class`; every row is named.

### Carry counter

The recovered counter code is:

```c
c = *(char *)(ecx + 0x85);
*(char *)(ecx + 0x85) = c - 1;
if (c == 0) { *(char *)(ecx + 0x85) = 8; /* dispatch the 128-case switch */ }
```

So the counter walks `8,7,...,1,0`, reloads `8`, and the carry-gated handlers
run when the pre-decrement value is zero.  `SimUnit.order_carry` reproduces
that state, and `sim::order_runs_this_tick(row, carry)` exposes the lane.
The engine still advances flingy motion on every tick: movement orders keep
calling `move_towards`, while target re-validation, build and cast handlers
observe the carry pass.  This preserves both halves of the retail split.

The row-6 label in `orders.dat` is `Carrier Ignore`, but the disassembly of
its handler (`0x7a154430`) is the generic "move to the stored unit/point and
finish" routine (`FUN_7a178f30` / `FUN_7a178f00` followed by
`FUN_7a158040`).  That row is therefore the move order used by the engine.

## Sixth reconstruction pass (retail region graph)

The earlier pass treated regions as connected components of the 8-pixel walk
grid.  The decompiled builder shows a coarser layout:

* `FUN_7a303840` indexes the region grid with a **256-cell row stride**
  (`grid[y*0x100 + x]`) and compares against the map width/height stored at
  offsets `0xe4`/`0xe6` of the map global.  The region grid is therefore one
  cell per **32-pixel tile**, not per 8-pixel walk cell.
* The 16 minitile `vf4` flags are used only to decide whether two tiles
  touch.  `FUN_7a3046b0` builds the adjacency lists with edge/corner masks and
  `FUN_7a304c10` reads the eight neighbouring region ids.
* `FUN_7a303840` stores a bounding box per region; `FUN_7a304510` derives the
  bbox centre (min/max scanned at `<<5`, i.e. pixels).  Regions also carry
  their tile count, which the AI `region_size` command reads.
* `FUN_7a303fd0` merges regions smaller than a threshold into the smallest
  adjacent region (bbox union, size addition, source marked `0x1fff`).  The
  ordinary case is a single pass at **size < 6**; the threshold only grows
  while the live region count is still at or above `0x9c4` (2500).
* `FUN_7a304210` renumbers the surviving regions and rewrites the tile grid;
  `FUN_7a302c10` aborts when the final count exceeds 5000.

`path::RegionMap` now follows that layout: tile cells, minitile edge
connectivity (orthogonal edge pairs plus the shared corner for diagonals),
bbox centres, adjacency and the size-6 merge pass.  `find_path_with_regions`
runs the two-level search: BFS on the region graph, then octile A* through the
intermediate bbox centres (short routes stay on the direct grid search).

**Correction (2026-11-06, session log 133).**  The flood-fill creation step
above was wrong: the 32-bit dump's `FUN_00763840` (the `0x7a303840`
counterpart; `FUN_00762c10` calls it before adjacency, the `0x9c4` merge at
`FUN_00763fd0` and renumbering) seeds regions from rectangles of one tile
group found by `FUN_00764d60` (at most 8 x 7 tiles, aspect <= 3, the best of
up to 25 candidates, accepted at the previous region's area, 56 at first),
grows each by one tile (`FUN_00764360`, two passes), and grows them all once
more when rectangles drop below 7 tiles.  `RegionMap::from_terrain` now does
that; Boot Camp has 82 regions instead of 1.

`src/bin/sc_regions.rs` prints the recovered graph:

```text
$ ./target/release/sc_regions "Maps/(2)Bottleneck.scm.chk" platform.vf4
(2)Bottleneck.chk: 128x128 tiles, 16236 walkable, 1 regions, largest 16236, smallest 16236, 0 directed edges

$ ./target/release/sc_regions "scenario/(4)Zoo Keeper.chk" jungle.vf4
(4)Zoo Keeper.chk: 128x128 tiles, 14535 walkable, 5 regions, largest 14418, smallest 8, 4 directed edges
```

The Zoo Keeper graph is intentionally coarse: a tile joins a region when **any**
of its sixteen `vf4` minitiles is walkable, and the four orthogonal neighbour
cases connect unconditionally (the recovered `FUN_7a3046b0` behaviour).  The
8-pixel walk grid and `astar_cells` still enforce the real corridors, so the
region graph only guides long routes.  The 50-region figure quoted in an
earlier pass predates the exact orthogonal rule; once `FUN_7a3046b0`'s
unconditional orthogonal append was implemented, the same open map collapsed
to one dominant region plus its isolated pockets.  The one-region Bottleneck
result is unchanged.

## Seventh reconstruction pass (bunker, nydus, shield battery)

Three shipped order families had no simulation behind them:

* **Bunker Guard (row 4, handler `0x7a140520`) / Enter Transport (row 90,
  handler `0x7a14c1e0`)**.  A Bunker is a four-slot infantry container, not a
  `space_provided` transport.  `World::order_enter_transport` now accepts
  Marine/Ghost/Firebat/Medic only, caps the cargo at four, and the bunker's
  own Unload order ejects the occupants.  Loaded units die with the bunker
  (the old code ejected them alive from destroyed transports as well; that is
  now corrected for every cargo holder).  Occupants keep their own weapon,
  cooldown and upgrades and fire from the bunker's position
  (`World::bunker_fire`) with the documented one-tile range bonus; loaded
  units are hidden (`alive == false`) but no longer skipped by the firing
  pass.  Hidden cargo still counts against supply (`supply_used` includes
  `in_transport` units), so loading transports/bunkers no longer frees supply.
* **Enter Nydus Canal (row 45)**.  The order resolves to the paired completed
  canal of the same owner and places the unit at the exit position; without a
  pair the order fails harmlessly.
* **Shield Battery (rows 66/67, handlers `0x7a162210` / `0x7a162140`)**.
  `World::shield_battery` implements the classic conversion: the battery
  spends one energy point for every two plasma-shield points, stops at the
  target's maximum, and returns the restored whole points.

`cargo` death, bunker capacity/type rules, the canal pair and the 2:1
shield/energy conversion are covered by `tests/reconstruction.rs`.

## Eighth reconstruction pass (unit collision)

The earlier simulator let any number of ground units share a position.  Retail
ground units have a collision box from `units.dat` dimensions and push each
other apart; this is visible in every match and is a prerequisite for
formations, chokepoints and mining.  `World::resolve_collisions` now runs every
logical tick with a 32-pixel spatial hash and two relaxation passes.  It uses
the same `units.dat` box as the pathfinder, separates both bodies along the
smaller overlap axis, keeps the result on walkable terrain, and ignores
buildings, flyers, burrowed units and cargo.  `tests/reconstruction.rs`
checks that two Marines spawned on one point separate and that a Dropship is
not moved by ground bodies.

This is an observed retail rule rather than a single recovered routine; the
exact shove priority used by `CUnit::update` was not decompiled.

## Ninth reconstruction pass (creep and Pylon power)

Two placement/economy rules that every campaign and melee match depends on
were absent:

* **Creep.** `World` keeps a per-tile maturity grid (`0..=3`) rebuilt from
  completed Hatcheries/Lairs/Hives and Creep/Spore/Sunken Colonies.  A new
  source seeds its footprint at maturity 3; creep spreads one ring every
  `CREEP_GROW_INTERVAL` (8) ticks to the three-tile radius, and unsupported
  tiles recede one step per interval after the last source dies.
  `find_build_spot` and `order_build` reject Zerg structures (except the
  Hatchery, which creates creep) unless the centre tile has maturity > 0.
  The Extractor is exempt, as in retail: it lives off the geyser itself.
* **Pylon power.** A completed Pylon powers a 6-tile radius.
  `World::building_powered` exempts the Nexus and Pylon; `train`, `research`
  and `upgrade` return `SimError::Unpowered` outside a field, and
  `update_production` pauses instead of completing.  Protoss placement
  (`find_build_spot`) only considers powered spots, so the AI keeps its
  production inside the field.

The creep radius and the exact Pylon radius are the classic documented
values.  Creep now has a maturity grid: a completed source seeds its
footprint at maturity 3 immediately, the edge spreads one ring every
`CREEP_GROW_INTERVAL` (8) ticks until the three-tile radius, and unsupported
tiles recede one maturity step per interval after the source dies.  Build
checks use tiles with maturity > 0.  Both rules are covered by `tests/reconstruction.rs`
(`zerg_structures_require_creep_and_hatcheries_make_it`,
`protoss_production_requires_a_pylon_power_field`).

## Tenth reconstruction pass (morphs, add-ons, requirement equivalence)

Three construction rules were missing and are now implemented:

* **Zerg building morphs.**  `World::morph_building` keeps the source
  structure and turns it into a construction site for Lair, Hive, Greater
  Spire, Sunken Colony or Spore Colony (source mapping from the build tree).
  The health fraction is preserved across the morph.  Worker build orders and
  the two AIs route morph targets through this API instead of creating a
  second structure.
* **Terran add-ons.**  `World::build_addon` attaches Comsat Station/Nuclear
  Silo (Command Center), Machine Shop (Factory), Control Tower (Starport) and
  Covert Ops/Physics Lab (Science Facility) to the right edge of a parent that
  has no add-on.  The parent records the add-on id; destroying either side
  cleans up the link, and destroying the parent destroys the add-on.  Add-ons
  are never worker-built.
* **Drone consumption.**  A Drone that starts a Zerg structure is removed
  from the game, as in retail; SCVs and Probes survive construction.

`requirement_equivalent` makes upgraded Zerg structures satisfy base
requirements: Lair/Hive count as Hatchery, Hive as Lair, Greater Spire as
Spire, and Sunken/Spore as Creep Colony.  Without this, morphing the starting
Hatchery silently broke every later build and tech gate.

`World::morph_unit` implements the Zerg unit morphs (Hydralisk → Lurker,
Mutalisk → Guardian/Devourer): the unit becomes an egg, the target type and
morph timer come from `units.dat`, and `Spell::LurkerAspect` now performs the
morph instead of only revealing the unit.  Lurker Aspect is treated as a
morph rather than an energy ability (the techdata energy column is unused for
it).

`supply_cap` is clamped to the retail 200 supply (400 in the table's
half-point units) so a base with more than 25 Supply Depots cannot exceed the
cap.

### Attack-move resume

Retail attack-move keeps the destination: the unit temporarily enters Attack
Unit, and after the target dies it continues to the objective.  The typed
order state previously dropped to Idle after the first kill, which made every
attack-move army stop at its first contact.  `SimUnit.resume_order` records
the interrupted Attack Move; `order_done` restores it before the order queue
is drained.  The test `attack_move_resumes_toward_the_destination_after_a_kill`
checks the kill, the resumed order and the continued movement (a second enemy
is placed far away so the victory check does not freeze the world).

Covered by `terran_addons_attach_to_their_parent_and_die_with_it` and
`zerg_building_morphs_keep_the_structure_and_consume_drones` in
`tests/reconstruction.rs`.

## Eleventh reconstruction pass (Terran lift/land)

`orders.dat` rows 69-73 are the Terran lift/land family.  They had no
simulation state, so Command Centers could not relocate.  `World::lift_building`
and `World::land_building` implement the mechanic:

* only the liftable Terran structures (Command Center, Barracks, Factory,
  Starport, Science Facility, Engineering Bay, Armory, Academy) can lift;
* a lifted building is an air unit (`World::is_air_unit`) for weapon
  selection, Dark Swarm/terrain miss rules, splash air-only checks, spider
  mine triggering and pathing, and is excluded from ground obstacles;
* lifted buildings move with the flingy speed (falling back to 5 px/tick),
  cannot produce/research/upgrade, and leave their add-on grounded and
  inactive (landing near it re-attaches it);
* landing validates the buildable footprint and rejects overlap with other
  completed ground structures/resources.

The typed `Order::Lift` / `Order::Land` states and `issue_order_id` rows
69-73 are wired to the API.  Covered by
`terran_buildings_lift_move_and_land`.

Queen Infestation (`Spell::Infestation`, tech 12) converts a Command Center
below half HP into an Infested Command Center owned by the Queen's player,
detaching any add-on (it stays on the ground and can be re-attached).
Covered by
`queen_infestation_converts_a_damaged_command_center`.

SCV repair now uses the documented rate `0.9 * maxHP / build_time_seconds`
(per SCV) and a cost of about one third of the unit's mineral cost over its
full hit-point pool, accumulated in 1/256 minerals.  The old implementation
repaired 4 HP per tick (about eight times retail) for a flat cost.  Covered by
`scv_repair_uses_the_retail_rate_and_cost`.

Terran structures below one third hit points now burn at the documented
rate of 20/256 HP per frame (1.875 HP/s at 24 Hz).  The burn applies to the
whole 106..=125 building block (including add-ons and lifted buildings) and
never to Zerg or Protoss structures; a burning structure that reaches zero is
destroyed.  Covered by
`terran_buildings_burn_below_one_third_hit_points`.

The player-facing internal rows now have concrete behaviour too: 154/155/178
(critter/Junk Yard Dog) wander via a patrol loop, 157 (Harass Move) is an
attack-move, 156/159/160/161/163/166/61 hold, 162 (Computer Return) heads for
the owner's town hall, 164/174 self-destruct, and the Reaver/Scarab rows
56/58/59/60/64/65 attack.  Covered by
`internal_ai_and_doodad_rows_have_concrete_behaviour`.

`issue_order_id` also resolves the shipped ability rows instead of returning
`Unsupported`: cloak rows 57/107/108/109, Shield Battery 66/67, Yamato Gun
112/143, Parasite 118/119, Spawn Broodlings 120, EMP 121, Lockdown 122,
Defensive Matrix 140, Hallucination 142, Recall 148/149, Medic Heal 175/186,
Disruption Web 180, Mind Control 181, Feedback 183, Optical Flare 184 and
Maelstrom 185 dispatch through the typed cast/cloak/shield APIs.  Covered by
`ability_order_rows_dispatch_to_the_recovered_casts`.

The Vulture mine rows 18/19/131 place one Spider Mine per order from the
carried stock of three (the Vulture reuses `hangar` as the mine counter),
Carrier rows 49/62 build an interceptor, Scanner Sweep 138 casts at the
point, Archon rows 104/105/147/182 use the warp/meld spells, and rows
51/52/55/161 stop while 156/159 hold.  Plasma Torpedo launch rows
124/126/127 call `launch_nuke`.  Covered by
`vulture_mine_order_places_the_three_carried_mines`.

## Eleventh reconstruction pass (miss chances, corpus audit)

### Miss chance is a strict 1/256 roll

The defensive miss table was reconstructed earlier as 255/256 under Dark Swarm
and 119/256 when the target stands on higher ground.  The implementation
compared with `roll <= chance`, which makes those rolls 256/256 and 120/256;
with a 0..255 roll the correct comparison is `roll < chance`.  `dodge_roll`
now centralises the rule and `dodge_chance` returns the raw 1/256 value, so
the two cases can be asserted directly.  Melee attacks still bypass the roll
(`fire_melee`), and air units ignore both terrain and Dark Swarm.

### Runtime table verification expanded to the order block

`tools/verify_runtime_tables.py` now compares the whole expanded
`orders.dat` block as well as the unit/weapon/flingy columns.  The loader
stores each order field array padded to `0xc0` bytes; the labels are
contiguous (`0x7ab2a090`), the fifteen flag fields start at `0x7ab2a210` with
stride `0xc0`, and highlight/dependency/target-order follow at
`0x7ab2ad50`/`0x7ab2aed0`/`0x7ab2b050`.  The expanded `techdata.dat`
(nine columns at `0x7ab2db08`..`0x7ab2ddf4`, 88-byte u16 stride) and
`upgrades.dat` (nine u16 columns with 0x80-byte stride from `0x7ab32d60`,
plus the race/max/Brood War byte columns at `0x7ab331dc`, `0x7ab3321c` and
`0x7ab3325c`) are checked as well.  All 40 added arrays match, for **73/73**
checked arrays in total.  This also corrected the ledger: the bytes at
`0x7ab2a5d0` are orders field 5 (`can_be_interrupted`), not the queueable
field, which is field 7 at `0x7ab2a750`.

### Shipped-map trigger corpus

`src/bin/sc_audit.rs` parses every extracted `.chk` with the same
`Scenario`/`TRIG` reader as the simulator and prints histograms of condition
ids, action ids, placed unit types and tileset eras.  Against the retail
1.23.10 corpus:

```text
$ ./target/release/sc_audit --maps /tmp/sc_maps
audit: 323/323 maps parsed, 0 failed, 322 with TRIG
condition ids: 1=61 2=908 3=875 4=160 5=39 7=1 8=1 9=15 10=1 11=1132
               12=263 14=10 15=1 17=3 21=373 22=2103 23=6
action ids:    1=336 2=340 3=937 4=595 5=9 7=19 8=826 9=1042 10=54
               11=330 12=29 13=1070 14=30 15=615 16=60 17=1 18=4 19=2
               20=1 21=8 22=75 23=71 24=66 25=390 26=419 27=91 28=221
               29=3 30=18 31=16 32=12 38=56 39=155 41=5 42=14 43=5
               44=2026 46=12 47=1 57=37
outside classic set: 0 condition uses, 0 action uses
```

Every condition used by the shipped corpus is in `0..=23` and every action is
in `1..=59`, so the implemented classic VM covers all trigger content the
retail 1.23.10 map set actually exercises.  The test
`every_shipped_map_uses_only_classic_trigger_ids` enforces this when
`SC_MAP_DIR` is set; a future Remastered-only id cannot slip in silently.

## Twelfth reconstruction pass (exact region edge masks)

The last region approximation is gone.  `DAT_7ad46470` is a heap table
allocated by `FUN_7a300c20` with `0x100000` bytes: one byte per 8-pixel
minitile, laid out as a fixed 256-tile (1024-minitile) row.  The writer reads
each megatile's sixteen `vf4` flags and packs them four at a time into the
four minitile rows of the tile.  The byte written is `0x80` when the
corresponding walkable bit is clear and `0x00` when the minitile is walkable;
the `^ 0xff010101` then `<< 7` in the decompile is exactly that inversion.
The table is therefore the `vf4` bit-0 walkability bitmap, but its purpose is
the recovered adjacency predicate, not a second source of terrain data.

`FUN_7a3046b0` classifies the eight `local_18` neighbours returned by
`FUN_7a304c10` (left, up, right, down, up-left, up-right, down-left,
down-right).  The four orthogonal cases branch straight to the append path:
any two orthogonal region tiles connect.  The four diagonal cases consult
the mask table with the exact offsets in the decompile:

| Direction | Current corner 2x2 | Diagonal corner 2x2 | Side routes |
|---|---|---|---|
| up-left | `0,1,0x400,0x401` | `-0x802,-0x801,-0x402,-0x401` | left `-2,-1,0x3fe,0x3ff`; up `-0x800,-0x7ff,-0x400,-0x3ff` |
| up-right | `2,3,0x402,0x403` | `-0x7fc,-0x7fb,-0x3fc,-0x3fb` | right `4,5,0x404,0x405`; up `-0x7fe,-0x7fd,-0x3fe,-0x3fd` |
| down-left | `0x800,0x801,0xc00,0xc01` | `0xffe,0xfff,0x13fe,0x13ff` | left `0x7fe,0x7ff,0xbfe,0xbff`; down `0x1000,0x1001,0x1400,0x1401` |
| down-right | `0x802,0x803,0xc02,0xc03` | `0x1004,0x1005,0x1404,0x1405` | right `0x804,0x805,0xc04,0xc05`; down `0x1002,0x1003,0x1402,0x1403` |

A diagonal is accepted when both corner 2x2 blocks are walkable **and** at
least one of the two side-tile 2x2 blocks is walkable.  `src/path.rs` now
implements that predicate (`retail_tile_adjacent`) and uses it for region
flooding.  The decompiled writer, classifier and neighbour fetch are saved in
`docs/evidence/region_edge_masks.txt`; the two unit tests
`recovered_edge_masks_accept_orthogonal_neighbours_unconditionally` and
`recovered_edge_masks_require_corner_and_side_blocks_for_diagonals` pin the
recovered truth table.  On the extracted 1.23.10 maps the tile-level region
counts and the `sc_campaign` all-map run remain unchanged.

## Thirteenth reconstruction pass (fixed-point energy, cloak drain, medic)

The per-tick `CUnit` update at `0x7a1791e0` was disassembled from the runtime
snapshot (saved in `docs/evidence/unit_update_energy.txt`).  Three gameplay
values were being conflated with whole points.

* **Shields** at `CUnit+0x60` regenerate `+7` in 1/256 points per tick,
  clamped to `units.dat` shields (`0x7ab2e8e8[type] << 8`).  This matched the
  engine already.
* **Energy** at `CUnit+0xa2` is also fixed point.  The update adds `+8` every
  logical tick and clamps to the unit maximum, i.e. one whole point every 32
  ticks (0.744/s at fastest).  The old `+1` every 8 ticks was four times too
  fast.  `SimUnit` now carries `energy_frac` and exposes
  `energy_fixed`/`set_energy_fixed`/`spend_energy_fixed`; ability costs from
  `techdata.dat` are multiplied by 256.
* **Personal cloak** follows the branch at `0x7a179307`: while the cloak
  state at `CUnit+0xdc` bits 8..9 is set, Ghost pays `10/256` and Wraith
  `13/256` per tick and the `+8` regeneration is skipped (the documented net
  18/256 and 21/256).  The old whole-point `10`/`13` drain emptied the pool
  about 25 times too fast.
* **Medic healing** restores `200/256` HP for `100/256` energy per tick
  (`MEDIC_HEAL_PER_TICK` / `MEDIC_HEAL_COST_PER_TICK`); the previous version
  restored two whole HP for one whole energy.
* **Feedback** now drains the target's whole fixed-point pool and deals that
  exact value as HP damage; Defensive Matrix absorbs it first, while shields
  and armour are bypassed.  The previous half-rate conversion was wrong.

The detector radius had a related fixed-point bug: `can_see_unit` compared a
1/256-pixel distance against a pixel radius, so a Missile Turret's seven-tile
radius behaved like less than one pixel.  With cloak no longer expiring after
20 ticks the existing test exposed it.  The radius is now scaled by 256;
`detector_radius_is_seven_tiles_in_fixed_point_space` pins 6/7/8 tiles and the
three new energy tests pin the recovered 8/7/10/13/200/100 rates.

## Fourteenth reconstruction pass (retail A* open list and order-table diff)

### Path tie-breaking recovered

The region pathfinder at `0x7a305590` is an A* over the recovered region graph.
Its open list (`DAT_7ad46480`, count `DAT_7ad46678`) is a binary heap of
entries whose `f` lives at `+0x14`:

* `FUN_7a305e20` (sift-up) stops only when `f(parent) < f(new)`, so an
  equal-`f` entry keeps bubbling above its parent;
* `FUN_7a305da0` (sift-down) chooses the right child only when
  `f(right) < f(left)` and stops when `f(parent) <= f(chosen)`.

There is no secondary `g`, coordinate or direction key.  Equal-cost routes are
therefore ordered by discovery order, and discovery order is the recovered
`local_18` order from `FUN_7a304c10`: left, up, right, down, up-left, up-right,
down-left, down-right.  The heuristic is the exact integer expression in
`FUN_7a1a4060`; the region A* also stops after 125 open entries.  The
decompiles are stored in `docs/evidence/pathfinding_astar.txt`.

`src/path.rs` now ports that policy:

* `retail_distance` is the exact `FUN_7a1a4060` expression (pinned by
  `retail_distance_matches_the_recovered_integer_formula`);
* `RetailOpen` is the exact sift-up/sift-down pair (pinned by
  `retail_open_list_orders_ties_by_the_recovered_heap_rules`, which asserts
  the recovered pop order `5,1,2,4,3,0`);
* `RegionMap::region_path` is A* over the region graph rather than BFS, with
  the same tie policy, re-open/decrease-key handling and 125-entry guard;
* `astar_cells` uses the same heap policy and the recovered neighbour order.

Neighbour lists are no longer sorted: the append order is observable because
it feeds the tie-break.

### Order table diff against the recovered switch

`tests/reconstruction.rs::order_specs_match_the_recovered_dispatch_table`
reads the recovered `docs/evidence/order_dispatch.tsv` and compares every one
of the 189 rows with `orders::ORDER_SPECS`: id, shipped label and handler VA
(or the `guard_check_icall`/no-direct-case marker).  `order_lanes_match_the_recovered_three_layer_switch`
parses the case labels back out of `docs/evidence/order_dispatch_vm.txt` and
requires every row to sit in the same group as the decompile: 40 every-tick
rows (4 fast + 36 worker/attack), 128 carry-8 rows, and the 21 rows resolved
by an alias, production timer or engine path.  This turns the order table from
an audited inventory into a differential test against the original
executable's switch.  `every_recovered_order_handler_has_decompiled_pseudocode`
requires all 137 unique fixed handler addresses to appear in
`docs/evidence/order_handlers_summary.tsv`.

## Fifteenth reconstruction pass (complete classic trigger action semantics)

The trigger VM claimed the classic action ids but only executed a subset.  The
shipped corpus exercised the undocumented gaps: action 11 (Create Unit with
Properties) appeared **1,041** times and did nothing; action 23 (Kill Unit at
Location) **71** times; action 27 (Set Score) **91** times; action 43 (Set
Invincibility) **5** times; actions 49-53 used the wrong operand as a delta
instead of a percentage.  The CHK comparison byte was also misread: the
shipped maps store "Exactly" as **10**, while the VM accepted **2**, so every
"Exactly" condition (126 in the extracted corpus) was permanently false.

The action VM now follows the CHK trigger record and the reference semantics:

* action 3 (Preserve Trigger) is persistent per player/trigger, and action 4
  (Wait) resumes at the following action after the wait, rather than
  restarting the trigger; wait times are milliseconds;
* action 11/44 create `num` units (0 = 1) at the location centre and apply
  the selected `UPRP` slot.  `Scenario` now parses the 64 × 20-byte `UPRP`
  section (state mask, field mask, owner/HP/shields/energy/resources/hangar
  and the invincible/cloaked/burrowed/hallucinated states);
* action 23/25 kill/remove up to `num` matching units in the location;
  action 22/24 kill/remove all matching units;
* action 39 moves up to `num` units to the destination; action 46 honours
  move/patrol/attack; action 48 gives up to `num` units from a location;
* actions 49/50/51 set HP/energy/shields to a percentage, 53 adds hangar
  units, and 52 sets 16-bit resource amounts on resource units;
* action 27 writes the eight score categories with the retail aggregate
  sharing (Total = 1/4 to units/buildings/kills/razings, the pairs = 1/2),
  and conditions 5/8/9/10/15/18/19/20/21 now read per-type kill counts,
  score categories, resource kinds and unit classes (229 Any, 230 Men,
  231 Buildings, 232 Factories);
* action 43 sets/clears/toggles invincibility, and `damage_hp` ignores
  invincible units; action 42 records the first matching doodad's state;
* action 57 follows the CHK layout (player in group, status in `extra`) and
  converts existing allies on ally/allied-victory;
* actions 7-12, 17-21, 28-37, 40 and 41 record their presentation,
  leaderboard, objective and next-scenario effects in typed vectors instead
  of dropping them; actions 54/55 pause/unpause the countdown timer;
* every classic action id 1..=59 now has a branch, including the Remastered
  debug-mode ids 58/59.

New tests: `exactly_comparison_is_the_chk_value_ten`,
`create_unit_with_properties_applies_uprp_slot_to_every_unit`,
`create_unit_count_and_kill_unit_at_location_count_are_honoured`,
`modify_hit_points_uses_a_percentage_not_a_delta`,
`set_invincibility_blocks_damage_until_disabled`,
`wait_action_resumes_the_remaining_actions`,
`set_doodad_state_and_order_action_apply_the_chosen_mode` and
`every_classic_action_id_dispatches_without_silently_dropping_gameplay_ids`.
All 323 shipped maps still simulate (`0 failed`), and `cargo test` now runs
**200** tests (`110` library + `3` audio + `3` economy + `9` handlers + `7` parity + `58` reconstruction + `6` graphics + `4` retail CASC/MPQ).

## Sixteenth reconstruction pass (UNIT record property offsets)

The map `UNIT` reader was two bytes early for every property after the unit
type.  The CHK layout is: special-states set mask at offset 12, fields-set
mask at 14, owner at 16, HP/shields/energy percentages at 17/18/19, resources
at 20, hangar at 24, states at 26 and the linked serial at 32.  The parser
was reading owner at 18 (the shield percentage), HP at 19 (energy), shields
at 20 (the resource bytes) and so on.  In the shipped corpus the shield
percentage is almost always 0, so **every placed unit was owned by player 0**
and map-authored HP/shield/energy/resources/hangar/states were lost.

`Scenario` now reads the exact offsets and exposes `fields_set`, `states_set`,
`states`, `hangar` and `link_serial`.  `World::from_scenario` applies each
field only when its `fields_set` bit is present, including 0% HP (1/256 HP),
16-bit resources, capped hangar counts, and cloaked/burrowed/lifted/
hallucinated/invincible states.  `UPRP` is also recovered on protected maps by
its exact 1,280-byte size.

The new test `map_unit_records_use_the_chk_property_offsets` pins all field
offsets, and `shipped_map_units_keep_their_real_owners` walks the extracted
corpus (323 maps, 47,050 valid records) and requires non-zero owners.
`MapUnit` now also keeps the record serial and linked serial: add-ons attach
and Nydus canals pair only when the two records name each other, as the
retail loader does, with the old proximity rule kept only as a fallback for
protected maps that lost the links (`map_serial_links_attach_addons_and_pair_nydus_canals`).
The all-map harness still reports `323/323 maps simulated, 0 failed`.

## Seventeenth reconstruction pass (playable graphical client)

The rules engine was headless, so the reconstruction could be tested but not
played.  A software client now sits on top of `World` and draws the retail
art pipeline:

* `src/render.rs` decodes the shipped tileset chain: CV5 records (52 bytes,
  terrain/flags/edges/16 tile ids), VX4EX (64-byte u32 minitile refs, bit 0
  horizontal flip), VR4 (8x8 palette indices) and WPE (RGBX palette).  The
  correct MTXM decode is `group = (value >> 4) & 0x7ff`, `index = value & 0xf`,
  `megatile = CV5[group].tiles[index]`, then sixteen 8x8 VR4 minitiles.
* The GRP decoder handles the shipped Remastered GRP layout: 6-byte header,
  then `x,y,w,h,offset` per frame, with the per-row u16 offset table and the
  0x80/0x40/literal RLE runs.  Palette entries 8..15 are remapped to the
  player colour; the rest come from the shipped `Units.pal`.
* `sc_view` presents a 1280x800 framebuffer with minifb: drag/shift
  selection, right-click move/attack/gather, attack-move, patrol, stop,
  hold, build placement, train commands, control groups, camera scroll,
  fog of war, terrain dimming, health/shield bars, minimap with unit dots
  and camera rectangle, and a command card/HUD with text from `font8x8`.
* `sc_shot` renders the same scene headlessly to BMP so the graphics path is
  covered without a display.

Bundled assets: the eight tilesets (`assets/TileSet`, 33 MB), `Units.pal`,
80 retail GRPs (`assets/unit`, 18 MB) and the 2-player Bottleneck map.
`tests/render.rs` checks the Marine GRP (229 frames, frame 0 20x26),
a real CV5/VX4EX/VR4 tile render, the 128x128 minimap and the playable-race
sprite bank.  A screenshot of the client is `docs/playable.png`.

## Eighteenth reconstruction pass (menu, map pack, pause and results)

`sc_view` is now a small self-contained game shell instead of a single-map
viewer:

* a start menu lists the bundled map pack and shows player count and
  tileset; Up/Down selects, Enter loads, Esc quits.  `--chk` still bypasses
  the menu for scripted runs;
* the map pack under `assets/maps` covers all player counts and six of the
  eight tilesets (platform, jungle, twilight plus the menu paths): Bottleneck,
  Boxer, Crusader, Green Valleys, Lost Civilization, River Crossing,
  StarCraft Fortress, Twilight Struggle and the five Enslavers campaign
  missions;
* players 1..N run the deterministic `SimpleAi`, so the human has an active
  opponent that mines, builds, trains and attacks.  Trigger actions 15/16
  (`Run AI Script` / at location) start `CampaignAi` threads in the client,
  so the shipped campaign missions run their own scripts too;
* Esc opens a pause overlay (resume, restart, menu, quit); victory and defeat
  are detected from the trigger winner / defeated flags and drawn as a
  full-screen result overlay; R restarts and M returns to the menu;
* `load_map` rebuilds the scenario, tileset, world, AI list, camera, groups
  and selection for a new map, so restart and map switching do not leak
  state.

The menu, map selection, a running Enslavers mission and the pause overlay were exercised
in a real Xvfb window with `xdotool` (select SCV, move, train SCV, pause).
Screenshots: `docs/menu.png`, `docs/playable.png`, `docs/pause.png`.

`bundled_map_pack_loads_and_runs_with_the_playable_assets` loads every map in
`assets/maps`, resolves its tileset, builds a `World` and steps 120 ticks.
That test found an i32 overflow in the map-unit HP percentage application for
large Hit Point values; it is now computed in i64 before clamping.

## Nineteenth reconstruction pass (137 literal handler routines)

The last code-organisation gap is closed.  `src/handlers.rs` contains exactly
one Rust routine per fixed order-switch target, named `h_<va>`, together with
a VA-ordered `HANDLERS` registry.  The module is generated reproducibly by
`tools/gen_handlers.py` from the recovery evidence; it is not hand-maintained.

### What is in the module

* **137 routines.**  `FUN_7a178340` has 168 direct cases; removing repeated
  handler addresses leaves 137 unique targets.  Every target has its own
  `pub fn h_<va>(&mut HandlerContext) -> HandlerResult`.  The two
  `guard_check_icall` sites (rows 12 and 13) are virtual and deliberately
  have no fixed routine.
* **The decompiled body is fingerprinted, not embedded.**  Each routine's
  doc comment carries the body size, FNV-1a-64 and SHA-256 (the registry
  stores the same); the pseudocode itself is Blizzard code and stays in the
  local, gitignored evidence (since 2026-11-03).  `docs/evidence/order_handlers_decomp.txt`
  is the decompressed companion of the original `.gz` batch (derived, never
  edited); `docs/evidence/order_handlers_manifest.tsv` is the machine-readable
  one-row-per-handler manifest (VA, size, rows, lane, family, routine,
  fingerprints, callee order, `CUnit` byte writes).
* **Rows, lane and family are derived from the shipped table.**  The
  generator refuses to run unless every routine's row list agrees with
  `ORDER_SPECS` and every row has a single recovered lane.  Nineteen
  behavioural families are recorded; handlers shared by several rows (for
  example the 15-row generic cast/tech handler `0x7a16bea0`, or the
  hero/AI/doodad rows) use the shared typed family instead of duplicating
  bodies.
* **Each routine literally executes its recovered call sequence.**  The
  generator parses every call site out of the decompiled body, including its
  raw argument text, and emits the calls in body order inside the routine's
  `if c.is_probe()` block.  `HandlerContext`'s probe mode records that
  sequence, and a test compares it with a fresh parse of the evidence.  The
  game path skips the recording branch; the typed state machine remains the
  gameplay authority.
* **The routines are on the live path.**  `World::issue_order_id` now
  delegates to `handlers::dispatch_row`, which enters the recovered routine
  for every direct-cased row.  The per-tick dispatcher enters the current
  row's routine at the recovered carry cadence and records the hit in
  `World::handler_hits`.  Rows resolved before the switch (7 aliases, 1
  production timer and 13 engine-only rows) keep their exact typed behaviour.

The generated module is 895,081 bytes, the plaintext evidence is 661,355
bytes and the manifest has 137 rows.  Reproduction:

```text
$ python3 tools/gen_handlers.py
wrote src/handlers.rs: 137 routines, 895081 bytes
wrote docs/evidence/order_handlers_manifest.tsv: 137 rows
wrote docs/evidence/order_handlers_decomp.txt: 661355 bytes

$ cargo run --quiet --bin sc_handlers
handlers: 137 routines, 165 direct-cased rows (165 assigned), 40 every-tick,
97 carry-8, 19 families, 137 decompiled bodies
```

### Differential enforcement

`tests/handlers.rs` is the oracle for the transcription and runs on every
`cargo test`:

* `every_recovered_handler_has_one_routine_with_a_unique_function_pointer`
  requires 137 distinct VAs and 137 distinct function pointers, and checks
  that every direct-cased `ORDER_SPECS` row names a registered VA;
* `handler_routines_match_the_decompiled_body_fingerprints` recomputes the
  FNV-1a-64 of every body from `order_handlers_decomp.txt`, compares it with
  the manifest and registry, compares the callee list in body order and the
  `CUnit` byte writes from `order_handlers_summary.tsv`, and requires that
  no decompiled body is reproduced in `src/handlers.rs`;
* `handler_rows_lanes_and_families_match_the_recovered_order_table` derives
  each routine's rows, lane and family from `ORDER_SPECS`, requires the
  recovered `order_dispatch.tsv` to name the same handler, and checks the
  binary-search registry order;
* `rows_outside_the_switch_are_explicitly_classified` pins the 7 aliases and
  the 1 production / 13 engine rows, including the rule that row 109
  (Decloak) keeps its own inverted typed behaviour rather than inheriting
  row 57's cloak handler;
* `every_order_row_dispatches_through_the_world_and_enters_its_handler` issues
  all 189 rows (with several parameter combinations) and requires all 137
  routines to be entered through the public `World::issue_order_id` path;
* `tick_dispatch_enters_the_current_rows_routine` checks the per-tick
  `CUnit+0x85` dispatch boundary;
* `routines_execute_their_recovered_call_sequences_in_probe_mode` probes all
  137 routines and requires the executed call list (including `func_0x`
  virtual calls and excluding function pointers passed as arguments) to equal
  every call site in the decompiled body, in order;
* `manifest_callees_and_writes_match_the_recovered_summary` independently
  parses `order_handlers_summary.tsv` and requires the generated manifest's
  callee sets and `CUnit` byte writes to agree;
* `handler_specs_are_a_complete_inventory_of_the_recovered_switch` requires
  the registry and the evidence summary to cover exactly the same 137 VAs.

Test result:

```text
$ cargo test --test handlers
running 9 tests
test result: ok. 9 passed; 0 failed
```

The all-map campaign harness remains unchanged after the handler wiring:

```text
$ ./target/release/sc_campaign --maps /tmp/sc_maps \
    --tiles /tmp/tiles/TileSet --ticks 1200
campaign coverage: 323/323 maps simulated, 0 failed, 47050 placed units,
9835 triggers on 322 maps, 19 AI script starts, 1200 ticks each
```

## Twentieth reconstruction pass (campaign slots, briefing, creep, audio)

The first playable campaign run exposed a loading bug that the melee-focused
tests could not see.  The client derived the player count from the number of
start locations, so campaign maps whose live slots are not contiguous lost
their enemy bases.

### Player slots

`OWNR` is the per-slot controller field (12 bytes) and `SIDE` the authored
race (0 Zerg, 1 Terran, 2 Protoss; higher values random).  In the shipped
Enslavers missions the live slots are 0, 3, 4 and 6 while the map has four
start locations whose owner bytes are 3, 4, 0, 6.  Counting start locations
yields four slots (0..=3) and silently skipped owner 4 and owner 6, which are
a Zerg base and a Terran force placed by the mission.  Across the extracted
corpus, 17 maps reference a unit owner that the old start-location count
dropped.

`Scenario::player_slots` now follows the same split the two shipped scenarios
use:

* a scenario that places a real force (campaign/use-map) takes its live slots
  from `OWNR` (`0` inactive, `4` neutral, everything else controlled) plus
  every start-location owner; slot 0 is the human seat used by all shipped
  missions;
* a melee scenario takes its seats from the start locations, so a spare
  `OWNR` byte cannot create a phantom player;
* `SIDE` selects each slot's race, and slots with no units are marked
  defeated so they never block victory.

The all-map harness still loads **323/323** maps but now reports **47,050**
placed units (previously 46,964) because the dropped campaign owners load.
The client uses the same helper, so Enslavers 01 now contains owners 4 and 6
(visible as the nearby Zerg base and the Terran relief force).

### Briefing, results and mission progression

`STR` string 0 is the mission name, string 1 the briefing and the first later
paragraph whose lines start with `- ` the objectives.  `Scenario::briefing`
returns that copy and strips StarEdit's `(USE MAP SETTINGS)`/`Veteran Level
Campaign` place-holder header.  `sc_view` shows it on a briefing screen before
the first tick and draws a results panel with elapsed time, kills, losses,
razings and gathered resources on victory/defeat.

Trigger action 41 (`Next Scenario`) stores the `STR` index **plus one**.  The
shipped uses prove the convention:

| Map | action `text_id` | `text_id - 1` string |
|---|---:|---|
| Enslavers 01 | 21 | 20 = `(1)Enslavers02a.scm` |
| Enslavers 02a | 16 / 17 | 15 = `02b`, 16 = `03a` |
| Enslavers 02b | 25 | 24 = `(1)Enslavers03b.scm` |

The old VM stored the raw id and would have resolved to `Bunker1` or an
objective paragraph.  It now stores `text_id - 1`, and the client normalises
the filename (`scenario_file_key`: strip directory/extension/player marker and
non-alphanumerics) to find the next bundled map.  `Enter` on a victory screen
loads it.

### Creep rendering and animation

There is no creep texture in CASC: `casc_tool <Data> list "*creep*"` returns
no file, and `images.dat` has no creep GRP entry.  The retail client therefore
builds the mat from the gameplay maturity grid, which is also why the earlier
pass could model growth/decay without an asset.  `render::draw_creep` now
composites an animated mat between the terrain and the fog:

* one maturity byte per tile from `World::creep_grid()`;
* 4-pixel grain from a stable hash, with two scales of variation (grain and
  16-pixel mottling);
* a four-phase pattern that changes every 6 ticks and an 8-tick brightness
  pulse, matching the slow retail animation cadence;
* ragged borders driven by the same noise instead of 32-pixel tile squares;
* the fog mask is applied after creep, so unexplored creep stays hidden.

`creep_overlay_is_animated_from_the_gameplay_maturity_grid` renders a real
campaign tile both at tick 0 and tick 48 and requires the terrain and the two
animation frames to differ.  `docs/creep.png` is a real render at tick 300.

### Audio

Nine effects were extracted byte-for-byte from CASC into `assets/sound`:
`ui_click`, `ui_move`, `ui_swish`, `transmission`, `terran_yes`, `zerg_yes`,
`protoss_yes`, `victory` and `defeat` (sources: `sound\glue\mousedown2.wav`,
`mouseover.wav`, `swishin.wav`, `sound\misc\transmission.wav`,
`sound\terran\marine\tmayes00.wav`, `sound\zerg\drone\zdryes01.wav`,
`sound\protoss\probe\ppryes03.wav`, `sound\misc\youwin.wav`,
`sound\glue\scorefillend.wav`).  They are all 44.1 kHz 16-bit PCM.

| Bundle | SHA-256 |
|---|---|
| `ui_click.wav` | `d183413d3cc768e239ddcf15b17271fe0d6f28442777890722ea06b24741c155` |
| `ui_move.wav` | `f3d369e4af62f3230de8075cf5a326fdc16d76a659730a4fc0fe44569f167874` |
| `ui_swish.wav` | `ac1462c2a9b2f2dabad04d61cbbb8dd90f62e1e5f285d77897f82d433c239bee` |
| `transmission.wav` | `4e744b26a46ba494bb8091656a1a29033d545fbb7341afc8583251c3ebf7ce2a` |
| `terran_yes.wav` | `6f172c7e495d05a9e46d42795c0fd7ecae6509df3d24cd997676f097be6bd835` |
| `zerg_yes.wav` | `51304558d2006f1cd40ccc4dbe459363387bde649d06ec7a5819459c66b197e9` |
| `protoss_yes.wav` | `9d7bd2fba0ff30497323c16d76866080a8efda0fd3dcc1ac84c42dab1ecd2c73` |
| `victory.wav` | `ffd34a45b4ade55f43ccda79158a553e8b0a9a229daf4819664277f29cae1b9e` |
| `defeat.wav` | `7e594ce6198983b1894449ede4f39507d1d5afd840425404e1dbce2920742376` |

`src/audio.rs` parses RIFF PCM (8/16/24/32-bit), downmixes to mono and
linearly resamples to 44.1 kHz.  The mixer keeps up to eight overlapping
voices and writes 1,024-frame blocks through ALSA loaded dynamically from
`libasound.so.2`; if the library or device is absent the player drains its
queue and the game runs silently.  `tests/audio.rs` decodes every bundled
effect, checks its duration and resampling length and exercises the no-op
fallback.  A live client opens a `s16le 1ch 44100Hz` PipeWire sink input
through ALSA, which is the observable proof that the path is not a stub.

### Client interaction fixes

Three UI paths were still allowing information or orders through the fog:

* the minimap tested `can_see_unit(0, 0)` once for every enemy instead of the
  enemy's own index, so one visible unit leaked the whole enemy army onto the
  map.  `World::minimap_units` now filters per unit and
  `minimap_dots_apply_fog_per_unit` pins the behaviour;
* a plain left-click could select an enemy unit and subsequent right-click
  orders were issued to it.  Selection now accepts only owner 0;
* clicking an enemy hidden by fog is rejected before targeting;
* a `--chk` file outside the bundled pack now gets a real map entry, so the
  HUD, result panel and restart use its name rather than the first pack map.

## Twenty-first reconstruction pass (retail campaign, OWNR human seat, briefing art)

The playable client shipped only the five Enslavers scenarios and thirteen
melee maps; the single-player campaign was absent even though the retail
installation contains it.  The pass also found that the client had been
assuming the human always owns player 0.

### The campaign is not in `Maps/`

StarCraft 1.23 stores every story mission as a loose CASC CHK:

```text
sd/campaign/terran/terran01/staredit/scenario.chk
sd/campaign/zerg/zerg04c/staredit/scenario.chk
sd/campaign/expprotoss/protoss08/staredit/scenario.chk
```

`src/campaign.rs` enumerates them, sorts each episode with the branch
directories kept in order (`tutorial`, `terran05`, `terran05a`, `terran05b`,
`terran06`, ..., `bonus` last) and reads the map-authored name.  The twenty
shipped directories resolve to six playable episodes:

| Episode | Missions |
|---|---:|
| Terran Campaign | 13 (tutorial + 12) |
| Zerg Campaign | 11 (tutorial + 10) |
| Protoss Campaign | 11 (tutorial + 10) |
| Brood War: Terran | 9 (`expterran/tutorial` is a zero-byte CASC entry) |
| Brood War: Zerg | 16 (`04a`..`04f`, `bonus`) |
| Brood War: Protoss | 8 |
| **total** | **68 non-empty CHKs** |

Several maps still carry StarEdit's `Untitled Scenario` as `STR[0]`
(`terran04`, `terran05`, `terran06`, `terran08`, `terran09`, ...).  Their
retail names are the quoted establishing-shot line in the briefing script,
so the reader falls back to `"THE JACOBS INSTALLATION"` ->
`The Jacobs Installation` with Roman numerals preserved (`Norad II`).

### The human seat is `OWNR == 6`, not slot 0

`Scenario::player_slots` had marked the first active slot as the human.  The
shipped campaign uses arbitrary slots:

| Map | `OWNR` | Human slot |
|---|---|---:|
| `terran/terran01` | `0,6,3,0,5,0,0,0` | 1 |
| `zerg/zerg01` | `0,0,0,6,0,5,0,0` | 3 |
| `protoss/protoss05` | `0,0,3,5,5,0,5,6` | 7 |
| `expprotoss/protoss04` | `5,6,7,3,0,5,7,0` | 1 |
| `Enslavers 01` | `5,0,0,5,5,0,6,0` | 6 |

The rule is now `active && OWNR in {2, 6}` with the old first-active fallback
for maps without the marker.  `sc_view` keeps a `local_player` index and uses
it for fog, the minimap, selection, resources, supply, victory/defeat and the
phone-ack voice instead of hardcoding 0.  The five Enslavers missions and all
campaign missions therefore start on the correct base (previously the client
controlled a computer seat and lost on the first check).

Campaign progression also no longer depends on a `Next Scenario` trigger:
retail campaign CHKs do not contain one.  Completing a mission resolves to
the next mission of the same episode; the Enslavers trigger string still
takes precedence when present.

### Retail briefing screens and real art

Each episode ships an establishing-shot/briefing script in
`locales/enus/assets/rez/est*.txt`.  `src/campaign.rs::parse_script` decodes
the tag language:

* `</PAGE>` splits pages; `</BACKGROUND path>` / `</FONTCOLOR path>` persist;
* `</TEXTBOX>`, `</SCREENLOWERLEFT>`, `</VIDEO path>`,
  `</VIDEOAUTOPROCEED path>` are recorded per page;
* `</COMMENT ...>`, `</DISPLAYTIME>`, `</FADESPEED>` and bare timing numbers
  are not rendered as copy.

The scripts name classic `.pcx` backdrops (`glue\palta\TerranA.pcx`).
`resolve_art` maps them to the Remastered `.dds` surfaces that CASC actually
ships (`glue/palta/terrana.dds`).  `src/dds.rs` is a new DXT1/DXT3/DXT5 and
uncompressed-ARGB decoder (bilinear resize + aspect-cover).  `sc_view` now
draws the shipped backdrop full-screen, the title line at 3x and the body at
2x with a hard shadow, the `SCREENLOWERLEFT` position and page counter.

The materialised cache gained a campaign manifest and the glue art:

```text
cache/campaign/manifest.tsv   68 rows
cache/campaign/<ep>/<key>.chk 68 CHKs
cache/briefing/<key>.txt      62 establishing-shot scripts
cache/glue/**                 17 backdrops (DDS) + 17 font tables (PCX)
```

The cache marker is versioned (`retail cache v2`), so an old cache without
the campaign is rebuilt once.

Screenshots (real retail art, rendered by `sc_view --shot`):
`docs/campaign_menu.png`, `docs/retail_briefing.png` (`Wasteland`),
`docs/retail_intermission.png` (multi-page `Revolution`) and
`docs/campaign_mission.png` (the loaded mission).

### Verification

```text
$ SC_INSTALL=".../StarCraft" cargo test
test result: ok  (19 binaries, 212 tests; the 7 retail tests ran, not skipped)

$ cargo run --release --bin sc_campaign -- --install "..." --ticks 1200
...
campaign coverage: 391/391 maps simulated, 0 failed, 68013 placed units,
11742 triggers on 390 maps, 90 AI script starts, 1200 ticks each
```

391 = 323 MPQ maps + 68 campaign CHKs.  New retail tests:

* `retail_campaign_enumerates_every_shipped_mission` (counts, names, every
  CHK parses, every mission except `bonus` has a briefing);
* `retail_briefings_parse_and_their_backdrops_decode` (62 scripts, 17
  distinct DDS backdrops, every one decodes to a non-black surface);
* `campaign_human_seat_follows_ownr_and_is_not_always_slot_zero` (five
  campaign maps plus the bundled Enslavers mission);
* `materialised_assets_load_without_the_bundled_asset_directory` now also
  requires the campaign manifest and the glue art.

`sc_view --shot PATH --shot-screen
intro|menu|briefing|mission|selection|pause|leaderboard|stats|score
--shot-map N` renders any of those screens headlessly to a BMP through the
same draw code, which is how the screenshots above were produced without a
window manager.

## Twenty-second reconstruction pass (retail console, menu, score, music/voice, intros)

The client now draws the Remastered UI surfaces that the previous passes left
as a text overlay.  Every rectangle below is read from the shipped
`locales/enus/assets/rez/*.ui.json` files; the art is decoded from the player's
own CASC storage and materialised into the cache, never bundled.

### Exact console layout

The shipped `stat*.ui.json` dialogs are the unit-data/command-card layout for
the 640x480 console:

| Dialog | Rect (640x480) | Contents |
|---|---|---|
| `statdata` | `[138,388,270,92]` | unit wireframe, name, HP/shield, status |
| `statres` | `[220,0,420,20]` | resource strip |
| `statport` | `[408,408,88,72]` | portrait button `[413,410,60,56]` |
| `stat_f10` | `[408,388,88,20]` | F10 button |
| `statbtndefault` | `[496,354,144,126]` | nine `36x34` command slots at `+9/+4`, `46`/`40` pitch |
| `statlb` | `[0,0,200,112]` | eight-row in-game leaderboard |
| `msnstats` | `[0,0,320,240]` | eight-player mission statistics |
| `gamemenu` | `[184,32,264,288]` | pause menu |

`src/ui.rs` parses those files (including the UTF-8 BOM and nested children)
into absolute control rectangles.  `tests::stat_dialogs_keep_the_shipped_exact_coordinates`
pins the whole table, including all nine command slots, both portrait
rectangles and the eight score rows in `gluscore`.

### Console art

`RetailInstall::materialize` copies the real console triptych for all three
races:

```
game/consoles/<race>/console_left.dds   1970x700 (Terran)
game/consoles/<race>/console_center.dds 1156x512
game/consoles/<race>/console_right.dds  2286x700
game/consoles/<race>/conover.dds        256x256 minimap overlay
game/consoles/<race>/console_time.dds   352x92
game/consoles/<race>/console_apm.dds    304x92
game/consoles/<race>/cmdbtns.grp        37 button frames
game/consoles/<race>/cmdbtns.dds.grp    37 HD button frames
unit/cmdicons/cmdicons.grp              390 command/unit icons
unit/wirefram/wirefram.grp              228 64x64 wireframes
```

`src/ui_art.rs` composes the triptych at the bottom of the screen, draws the
minimap inside `console_left`'s frame and the command card inside
`console_right`'s nine slots.  The `statdata` wireframe is drawn with
`wirefram.grp` frame = unit id; train/build buttons use `cmdicons.grp` frame =
unit id over the HD `cmdbtns.dds.grp` button surface.  The resource strip,
portrait and command slots use the exact JSON rectangles above.

### Main menu and score art

The menu now uses the retail title art (`glue/title/title.dds`, 3412x1920),
the shipped menu frame (`glue/mainmenu/titleframe_overlay.dds` at
`[374,14,266,264]`), its inner background (`[413,66,205,205]`), the eTail,
pIntro and pCredit plates, and the exact `glumain.ui.json` control anchors.
The score screen uses `glue/score{t,p,z}{d,v}/pmain.dds` as a translucent
frame over a starfield, the eight `gluscore` row rectangles, and the shipped
`msnstats.ui.json` panel for the F10 statistics view.  The pause panel uses
`gamemenu.ui.json` (`[184,32,264,288]`); the in-game leaderboard uses
`statlb.ui.json`.

### Music and complete voice sets

The Remastered music is shipped as Ogg Vorbis (`music/*.ogg`, 23 tracks) and
is decoded by `lewton` + `ogg` in `src/audio.rs::decode_ogg`.  The materialiser
copies the title theme, the three ready rooms, the twelve battle tracks and the
six victory/defeat cues into `cache/music/`; `MusicBank` decodes each track
lazily and the mixer loops it under the effects.

The complete voice sets come from the shipped `arr\sfxdata.tbl`: 1,127 names
(`u16` count, `u16` offsets, NUL-terminated DOS paths) and the parallel
`arr\sfxdata.dat` 4-byte records.  `src/sfx.rs` maps each id to its CASC path
(`Terran\MARINE\TMaRdy00.WAV` -> `sound/terran/marine/tmardy00.wav`) and
`src/voice.rs` maps every playable unit to its shipped folder.  Cues are
classified from the file stems (`Rdy`, `Wht`, `Pss`, `Yes`, `Dth`, `Atk`,
`Hit`, specials), so selection plays the ready line and every accepted order
plays a different acknowledgement take.  The materialisation copies **1,122
of 1,127** paths; the five absent entries are not present in this 1.23 CASC
build (`bullet/phoatt00.wav`, `bullet/pzefir00.wav`, `bullet/tbafir00.wav`,
`protoss/archon/parmin00.wav`, `protoss/darchon/pdrdth00.wav`).

### Intros and screens

A five-second title intro fades from black over `glue/title/title.dds`, shows
the Blizzard/title caption, plays `music/title.ogg`, and can be skipped with
Enter/Space/Escape or replayed from the menu (`I`).  The retail campaign
briefings, mission-start screens, pause menu, score panel, F10 statistics and
L leaderboard are all reachable from the same client.  `sc_view --shot
--shot-screen intro|menu|briefing|mission|pause|leaderboard|stats|score`
renders each screen through the same draw code.

### Creep texture

CASC still ships no creep GRP (a `--contains creep` scan of 52,498 paths finds
only `CREEP_COLONY` strings).  The client therefore keeps the procedural mat
but now renders it as an opaque organic surface: dark purple-brown grain,
irregular 8-pixel raised blisters with shadow rings, 16-pixel mottling, a
4-phase animation and ragged noise edges.  The texture is still generated from
the gameplay maturity grid; the latest render is `docs/creep.png`.

### Verification

```text
$ ./target/debug/sc_assets --install "..." verify
...
PASS terran console center                296064 bytes  game/consoles/terran/console_center.dds
PASS terran command button frames         627708 bytes  game/consoles/terran/cmdbtns.dds.grp
PASS command icons                        230008 bytes  unit/cmdicons/cmdicons.grp
PASS unit wireframes                      306897 bytes  unit/wirefram/wirefram.grp
PASS main menu title frame               1672832 bytes  glue/mainmenu/titleframe_overlay.dds
PASS title art                          26204288 bytes  glue/title/title.dds
PASS terran score panel                  5875328 bytes  glue/scoretd/pmain.dds
PASS ui layout statdata / gluscore / msnstats
PASS title theme                        6265961 samples  music/title.ogg
PASS voice set                          1122/1127 paths  arr/sfxdata.tbl
verify:         21/21 checks passed
```

`tests/retail.rs::materialised_assets_load_without_the_bundled_asset_directory`
loads the materialised cache with no bundled `assets/` directory and asserts:
17 UI JSON layouts, 23 Ogg tracks, >1,000 voice files, the 390-frame command
icon sheet, the 228-frame wireframe sheet, the 37-frame HD button sheet, a
non-empty console render, and a decoded Ogg title theme plus Marine "yes"
clip.  `cargo test` (with `SC_INSTALL` set) is green, and the screenshots are
`docs/retail_menu.png` (title intro), `docs/menu.png` (main menu),
`docs/playable.png` (in-game console), `docs/selection.png` (unit wireframe
and StatData), `docs/pause.png`, `docs/score.png`, `docs/leaderboard.png`,
`docs/stats.png`, `docs/briefing.png`, `docs/creep.png` and
`docs/campaign_mission.png`.

## Known gaps (outside the playable rules core)

Remaining non-gameplay limits are only the cosmetic
20th-anniversary/Carbot console skins and pixel-exact creep (CASC ships no
creep texture), plus decoded WebM cinematics not being used beyond the
retail title/briefing screens.

The following are **not** claimed as recovered.  None of them is a missing
rule in the playable simulation.

* (superseded 2026-11-03, session log entry 76: creep is now the retail cv5
  group-1 tiles plus the `tileset/<era>.grp` edge frames, picked by the
  client's CRT `rand` rule; the statement below was wrong) the exact retail
  creep texture.  CASC ships no creep GRP and the engine
  builds the mat procedurally; the client now renders an opaque organic mat
  from the recovered maturity grid (purple-brown grain, 8-pixel blisters with
  shadow rings, 16-pixel mottling, 6-tick phase, ragged noise edge).  The
  generation is deterministic and matches the retail *behaviour*, but it is
  still not a pixel copy of the retail screen (no such asset exists);
* the Remastered 20th-anniversary/Carbot alternative console skins.  The stock
  Terran/Zerg/Protoss consoles, menus, score panels and all voice/music sets
  are used; the cosmetic alternate skins are materialised only on request.
* (superseded 2026-11-03, session log entry 76: the HUD portraits play
  their WebM clips full motion and the campaign act movies stream through
  `src/cinematic.rs`) decoded WebM cinematics are used on the retail
  title/briefing screens only.
  The `smk/starintr.webm` / `starxintr.webm` intros and the
  `interstitials/*.webm` briefing movies are parsed (`src/video.rs`) and
  their poster frames are composited on the intro and briefing screens;
  full-motion scrubbing outside those screens is out of scope.

Note: Remastered-only EUD/bitmask trigger paths are parsed as data and
intentionally not executed; `tools/trigger_ids.py` and `src/bin/sc_audit.rs`
confirm no shipped map (323 archives, 322 with `TRIG`) uses condition ids
outside 1..=23 or action ids outside 1..=57, so this is not a gameplay gap.

## Twenty-third reconstruction pass (intros, briefing cinematics, full menu)

The title intro was a five-second fade over `glue/title/title.dds`; the
briefing screen showed only an `INTERMISSION <name>` caption.  The retail
`View Intro` button plays `smk/starintr.webm` (122,081,725 bytes, VP9
1920x1080, 138 s) and the expansion intro `smk/starxintr.webm` (75,968,887
bytes, 227 s); the HD `est*_hd.txt` scripts name `interstitials/*.webm`
movies (62 files) and the campaign disks (`glue/campaign/*.webm`, 18 files).

* `src/video.rs` is a dependency-free EBML/WebM header parser (EBML,
  Segment, Info, Tracks, Cluster count; codec `V_VP8`/`V_VP9`, pixel size,
  duration through the timecode scale, audio presence).  `parse_webm` is
  exercised against a synthetic minimal WebM and, when `SC_INSTALL` is set,
  against the real `starintr` / `starxintr` / `tr1010` headers.  Poster
  decoding shells to the system `ffmpeg` (`-ss 10% -vframes 1 -f rawvideo
  -pix_fmt rgb24`): the seek lands past the opening fade (starintr is pure
  black at 0 s, real content at ~15 s).  Without `ffmpeg` the client falls
  back to the title art, so headless/CI runs still work.
* `src/menu.rs` is the retail menu state machine behind the 44
  `docs/original_screenshots/*.png` captures: Main (8 entries), Registry,
  GameType (Original/Expansion), Episode (I-VI), Mission, SavedGames,
  Replays, Lobby (map + Free For All/Melee/Use Map Settings + slot races),
  Options (Speed/Sound/Video/Game/Network), Hotkeys (Terran/Protoss/Zerg),
  Collections (stock skins only), Credits, Intro (both variants), plus the
  in-game Game Menu (7 entries), Mission Objectives and StarCraft Tips.
* `RetailInstall::materialize` now copies the 94 videos
  (`smk/star*.webm`, `interstitials/*.webm`, `glue/campaign/*.webm`,
  `glue/expcampaign/*.webm`) into `cache/videos/` with a `manifest.tsv`
  (`rel/size/codec/WxH/duration_ms/clusters/has_audio`) parsed by the same
  header reader.  The cache marker is versioned (`retail cache v4`).
* `sc_view` loads a `VideoStore` from the cache, decodes the intro poster
  (`X` toggles original/expansion) and the current briefing page's
  intermission poster, and composites them cover-fit/letterboxed with
  `CINEMATIC <stem> [decoded WebM poster]` captions.  The HD `_hd`
  establishing shots are preferred because only they carry `</VIDEO>` tags
  (`campaign::briefing_candidates` tries `_hd` first).  All 13 menu screens
  plus gamemenu/objectives/tips render headlessly
  (`--shot-screen main|registry|gametype|episode|mission|lobby|options|
  hotkeys|collections|credits|gamemenu|objectives|tips|intro|briefing`).
  `sc_assets verify` checks the three header+poster decodes (24/24) and
  `tests/retail.rs` requires >=80 videos plus non-black intro/intermission
  posters from the cache.

Remaining non-gameplay limits are only the cosmetic
20th-anniversary/Carbot console skins and pixel-exact creep (CASC ships no
creep texture), plus decoded WebM cinematics not being used beyond the
retail title/briefing screens.

The main-menu art, score-panel art, pause panel, in-game leaderboard, music,
complete unit voice sets and the title intro are now implemented from the
player's own installation (see the twenty-second pass above).  Nine shipped
WAV effects remain bundled for no-install runs; with `--install` the client
uses the full retail sound set.

The playable rules core, data tables, build tree, damage, projectiles,
movement, vision/cloak/detector rules, orders, economy, spells, transports,
the full iscript gameplay VM, both AI families (deterministic skirmish AI and
the complete shipped campaign opcode interpreter) and the classic trigger VM
are covered by `cargo test`.

## REA command log (selected)

```text
rea inspect StarCraft.exe
  -> ev_56d0ae052c47659d36622ca91a8d3a50ba42abb83f4607bbd100b642ba817f32

rea inspect_with_objdump /tmp/sc_mem/sc_dump.exe section_headers
  -> rebuilt runtime PE, SHA-256 9f583729f6e7a09a11889931e9471f3de09f770a35b635bed197dc205cd12059
  -> ev_6654fb8874d1adf0a7679ff785228f8b50865420b1d08d0d56f7da56951d893b

rea search StarCraft.exe 'units.dat' --kind strings
  -> 0xd36dac
  -> ev_a636c389800d707c56c31d7c0b6f31acf25a7f384a35222c87580c17b2ba7ca8

rea read-bytes StarCraft.exe 0xf0757c --length 100
  -> 100/100 bytes, complete
  -> ev_b5f4f6052325402d5e38652e366840910ffa3b9d937e6b78e2d2d589af2e4cba

rea decompile win_7a100000.bin 0x7a13ce30 \
    --target-format raw-binary --architecture x86 --entry-point 0x7a100000
  -> damage routine; matrix/shield/armour/size branches
  -> ev_2d6a410f20525463b8e816797887f36f0e5891ec6164ad904ec7b2f2b8114681

rea decompile win_7a100000.bin 0x7a1074c0 \
    --target-format raw-binary --architecture x86 --entry-point 0x7a100000
  -> CBullet dispatch; xref to "CBullet: Damage"
  -> ev_d7504978235e68495761e68ddb9941947d41b3110fbc1bd065df2583edd0b750

rea decompile /tmp/sc_mem/sc_dump.exe 0x7a1074c2   # CBullet dispatcher
rea decompile /tmp/sc_mem/sc_dump.exe 0x7a1e0b20   # AI script interpreter
rea decompile /tmp/sc_mem/sc_dump.exe 0x7a1134f0   # iscript opcode dispatcher
rea decompile /tmp/sc_mem/sc_dump.exe 0x7a1148f0   # animation start / wait
rea decompile /tmp/sc_mem/sc_dump.exe 0x7a114890   # playfram + direction offset
rea decompile /tmp/sc_mem/sc_dump.exe 0x7a140750   # useweapon
rea decompile /tmp/sc_mem/sc_dump.exe 0x7a1e0620   # aiscript/bwscript loader
rea decompile /tmp/sc_mem/sc_dump.exe 0x7a112b00   # iscript loader
```

The second-pass decompiles were run through the same Ghidra 12.1.4 project
(`/tmp/sc_ghidra/scproj`, target SHA-256
`9f583729f6e7a09a11889931e9471f3de09f770a35b635bed197dc205cd12059`) using
`analyzeHeadless … -process sc_dump.exe -noanalysis -postScript DumpDecomp.java`.
The MCP server in this session had no `GHIDRA_INSTALL_DIR`, so the identical
Ghidra provider was invoked directly; the REA CLI was used for the earlier
evidence records above.  The AI operand-width correction (u16 unit ids) is
visible in the decompiled `case 6` and `case 1` branches and is independently
confirmed by the shipped `Ter3` byte stream.

The Ghidra 12.1.4 project used for the expanded decompiles was created from
the rebuilt runtime PE (`sc_dump.exe`) and queried with
`analyzeHeadless … -process sc_dump.exe -noanalysis -postScript DumpDecomp.java`.
Selected recovered functions:

| VA | Observation |
|---|---|
| `0x7a1576f0` | order queue replace/drain; 8-entry limit; interruptible/queueable table checks |
| `0x7a156cd0`, `0x7a157534` | append/prepend order; increment count only when highlight != -1 |
| `0x7a13ce30`, `0x7a13cf90` | damage pipeline and second floor / shield subtraction |
| `0x7a242e60` | damage-type×size multiplier lookup |
| `0x7a150d80` | acquisition range = max(target_acq, weapon ranges) |
| `0x7a109190`, `0x7a109b10` | flingy initialisation from flingy.dat |
| `0x7a109a70`, `0x7a109580`, `0x7a10c7e0` | direction-table velocity and acceleration |
| `0x7a1ef120` | HP+shield strength helper |
| `0x7a302c10` | region/pathfinding builder; 5,000-area guard |
| `0x7a2c7d90` | invalid action/order assertion |

All addresses are runtime VAs in the snapshot whose SHA-256 is above.
Decompiled pseudocode is a reconstruction, not original source; it is used
only to confirm the layouts and formulas listed here. The on-disk `.text`
remains encrypted, so a static xref result against the shipped file is a
false negative and was not used.

## Retail asset access without redistributing the game

The reconstruction does not need a copy of the retail art, audio, maps or
tables in the repository.  `src/retail.rs` opens the player's installation
read-only and resolves the same files the game loads:

1. `.build.info` names the build key; `Data/config/<xx>/<yy>/<build key>`
   carries the ENCODING and ROOT content/encoded keys.
2. Local `Data/data/%02x%08x.idx` files map the first nine bytes of an
   encoded key to `(data.NNN, offset, encoded size)`.
3. The ENCODING manifest (`EN` v1, 4 KiB CKey pages) maps each content key to
   its encoded key(s); StarCraft's ROOT manifest is the text `path|cKey` list
   used by CascLib's `TRootHandler_SC1`.
4. File bodies are BLTE streams.  Local archives may prepend the 0x1e-byte
   encoded header; frames use `N` (stored) or `Z` (zlib).  The README root and
   map CHKs decode with this path.
5. `Maps/**.scm|.scx` are MPQ v1 archives.  The shipped corpus includes
   protected ladder maps with encrypted hash/block tables
   (`0xC3AF3770`/`0xEC83B3A3`), hash entries that store hash types 1/2, a
   scrambled declared hash-table size (the real size is
   `block_table_offset - hash_table_offset`), `KEY_V2` file-key adjustment
   (`(key + file_pos) ^ file_size`), and both `FILE_COMPRESS` (mask byte +
   PKWARE/zlib) and `FILE_IMPLODE` (bare PKWARE sector streams) layouts.
   MPQ file keys hash the plain name (`scenario.chk`), not the archive path.

The vendored `third_party/pklib` is StormLib's MIT-licensed PKWARE DCL
implementation; it is only used for the `0x08` sector method.  No retail bytes
are compiled into the crate.

### Verification against the live install

```bash
cargo run --release --bin sc_assets -- \
  --install "/path/to/StarCraft" verify
```

Observed against 1.23.10.13515:

```
CASC:           52498 paths, 14382 index entries, 43091 encoding entries
PASS arr/units.dat ... scripts/iscript.bin   (7 documented SHA-256 hashes)
# tests/retail.rs also checks all 9 retail effects byte-for-byte
MPQ maps:       323/323 CHKs parsed, 322 with TRIG, 0 failed
PASS Maps/(2)Bottleneck.scm scenario.chk 65833b8611ea0b1c...
world:          66 units after 240 ticks on MPQ Bottleneck
verify:         9/9 checks passed
```

`cargo run --release --bin sc_campaign -- --install "/path/to/StarCraft"
--ticks 1200` then simulates the same corpus directly from MPQ + CASC:

```
campaign coverage: 323/323 maps simulated, 0 failed, 47050 placed units,
9835 triggers on 322 maps, 19 AI script starts, 1200 ticks each
```

Raw command output is stored as `docs/evidence/retail_verify.txt` and
`docs/evidence/retail_campaign.txt`.

`tests/retail.rs` enforces the same facts (`SC_INSTALL=... cargo test --test
retail`): 323 archives, 322 with triggers, byte-identical Bottleneck CHK, the
documented DAT/GRP/WAV hashes, and a frame rendered from a materialised cache
with no bundled `assets/` directory.  The client's `--install` flag
materialises the needed subset into `$XDG_CACHE_HOME/galaxycraft/retail-<build>`;
the cache is derived from the user's own installation.

## Remastered intro table and front-end assets (2026-10-29)

REA (`read_bytes` + `search_strings`, Ghidra 12.1.4) on
`x86_64/StarCraft.exe` (1.23.10.13515, sha256
`ce3ab05dc9a6aa35418947e5d95e19651ad6fbcd5e2c1856cd729c5b7c31281c`)
resolved the intro movie table at **`0x140ae7ae0`**.  The 28 NUL-terminated
paths in table order are:

```
smk\blizzard.webm
smk\starintr.webm
smk\PreTerran_1.webm .. smk\PreTerran_4.webm
smk\Terran1.webm      .. smk\Terran4.webm
smk\PreZerg_2.webm    .. smk\PreZerg_4.webm
smk\Zerg1.webm         .. smk\Zerg4.webm
smk\PreProtoss_1.webm .. smk\PreProtoss_3.webm
smk\Protoss1.webm      .. smk\Protoss4.webm
smk\starXintr.webm
smk\xProtoss.webm / smk\xTerran.webm / smk\xZerg.webm
```

`video::CAMPAIGN_CINEMATICS` and `intro_sequence` encode this table (when
each campaign film plays comes from the `MenuEntry` table:
`campaign_progress::cinematic_movies` / `after_victory`, session log 87); `every_intro_table_movie_is_readable_and_decodable`
opens and parses all 28 files from the player's installation
(`SC_INSTALL=... cargo test --lib video::tests::`, 10/10).

The front-end layout data is the shipped `glumain.ui.json` (640x480): the
hotboxes are Single `[0,30,320,150]`, Multiplayer `[78,190,240,160]`, footer
`[0,345..445,160,20]`, Exit `[425,320,180,136]`; the label anchors and ink
widths in `menu::MAIN_LABELS` were measured on
`docs/original_screenshots/Screenshot_20261027_130637.png` at 3x.  The FLC
rectangles come from `rez/flc.json` (`Single -20,0,425,175`; `SingleOn
-40,-10,425,200`; `Multi -10,-5,282,270`; `MultiOn -52,-2,330,220`; `Exit
-2,-4,204,182`; `ExitOn 5,0,176,160`).  The title frame is drawn at native/4
(overlay 1584x1054), which lands its red border on the 820x820 panel.

Verification shots: `docs/menu.png`, `docs/intro.png` and
`docs/ui-coverage.png` (headless 2560x1440, compared with the 32 references
in `docs/original_screenshots/`).

## Campaign 1 against retail recordings (2026-11-05)

Sources: the shipped tables (`arr/mapdata.tbl`, `gluhist`, `MenuEntry`,
`glurdyt`/`wmission`/`lmission`/`gluscore` layouts, `rez/flc.json`,
`stat_txt.tbl`, `network.json`, `rc3.json`), the 11 Episode I CHKs and the
user's retail recordings (video only): Wasteland briefing, rescue and
victory (`Screencast_20261105_0737`-`0745`), Boot Camp in full (`_111714`)
with a ready-room screenshot, Backwater Station in full (`_113518`).
Every number below is in `research/docs/session-log.md` 2026-11-05
(87-95), with the FACT/INFERENCE split.

| Behaviour | Retail measurement | Test |
|---|---|---|
| Ready room script time 0 / Duke / text cleared | 9.10 / 36.70 / 64.90 s | `wasteland_ready_room_follows_the_retail_recording` |
| Ready-room text: hold, then linear scroll | 4.0 s; 18.23 / 13.34 px/s | `ready_text_scroll_follows_the_retail_recording` |
| Trigger text lifetime | 9 samples, rule within 0.2 s | `text_lifetimes_follow_the_retail_measurement` |
| Boot Camp opening order and gaps | within 0.6 s | `boot_camp_opening_follows_the_retail_recording` |
| Wasteland rescue -> objective text | 6.0 s | `wasteland_rescue_conversation_follows_the_retail_timing` |
| Wasteland victory after the tenth Marine | 0.57 s | `wasteland_victory_fires_within_one_trigger_cycle` |
| Backwater Station opening line | first game frame | `backwater_station_opens_with_raynor_on_the_first_frame` |
| Backwater Station ending and victory | within 0.7 s | `backwater_station_ending_follows_the_retail_recording` |
| Score panel slide and count-up | edge within 2 px | `score_count_up_follows_the_retail_curve` |

Engine bugs these comparisons exposed: `MRGN` scaled x32, wait-blocking
missing within a cycle, Victory/Defeat applied to player 0, force triggers
never run, rescue polled after the first trigger pass.  Not verified:
audio timing (no sound in any recording) and missions 4-11.

