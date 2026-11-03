# Replays (`gcrep` / `gcsave`): how to record, verify and play back games

A GalaxyCraft replay is **the map + the command log** of the
game. It is not a video nor a copy of the state: opening it rebuilds the
world from the embedded `.chk` and reapplies the commands tick by tick. Since the
simulation is deterministic (LCG with a fixed seed and `World::issue` as the only
entry point for orders), the result is identical to the original
game, and this is checked with a state hash.

---

## 1. Recording from the game

| Action | Key | What it writes |
|---|---|---|
| Save game | `F5` | `save-NNN.gcsave` in `<dir>/saves/` |
| Save replay | `F6` | `replay-NNN.gcrep` in `<dir>/replays/` |

`<dir>` is, in order:

1. `$GALAXYCRAFT_SAVES`, if defined,
2. `$XDG_CACHE_HOME/galaxycraft`, if defined,
3. `~/.cache/galaxycraft`.

Real example on this machine:

```bash
$ ls ~/.cache/galaxycraft/replays ~/.cache/galaxycraft/saves
```

To take a replay to another computer, just copy the `.gcrep`: it carries the
`.chk` inside, so it does not depend on the map still being installed.

```bash
cp ~/.cache/galaxycraft/replays/replay-001.gcrep /tmp/my-game.gcrep
```

The on-screen message when saving confirms the path and the tick:

```
Replay saved /home/<user>/.cache/galaxycraft/replays/replay-001.gcrep
```

## 2. Playing back

### From the menu

`Multiplayer → Replays` (or `Single Player → Load Replay`) lists the `.gcrep`
files found; `↑`/`↓` selects, `Enter` plays and `Del` deletes.

During playback:

| Key | Action |
|---|---|
| `Space` | pause / resume |
| `+` / `-` | speed up / down |
| `Esc` | stop and return to the menu |

### From the command line

```bash
cargo run --release --bin sc_view -- \
  --install "$SC_INSTALL" \
  --replay /tmp/my-game.gcrep
```

`--replay` accepts a `.gcrep` **or** a `.gcsave` (in both cases it opens as a
replay, without AI). If the file is not a valid save the client exits with
code 1 and a clear message, instead of opening the menu.

For a windowless capture (CI, screen comparison):

```bash
cargo run --release --bin sc_view -- --replay /tmp/my-game.gcrep \
  --width 3440 --height 1440 --shot /tmp/replay.bmp --shot-screen replay
```

## 3. Create, inspect and verify without playing

`sc_replaytool` records a deterministic AI-vs-AI game, prints what a
file contains and checks that two playbacks of the same log match.

```bash
# 1) record a demo (25 s of gameplay, map included)
cargo run --release --bin sc_replaytool -- \
  make --chk assets/maps/Boxer.chk --ticks 600 --out /tmp/demo.gcrep

# 2) see what it contains
cargo run --release --bin sc_replaytool -- info /tmp/demo.gcrep

# 3) verify determinism (two passes, same hash)
cargo run --release --bin sc_replaytool -- verify /tmp/demo.gcrep
```

Output checked on this machine:

```
$ ./target/release/sc_replaytool make --chk assets/maps/Boxer.chk --ticks 600 --out /tmp/opencode/demo.gcrep
wrote /tmp/opencode/demo.gcrep  map "Boxer"  tick 600  21 commands  final hash f1127320cf799060

$ ./target/release/sc_replaytool verify /tmp/opencode/demo.gcrep
map Boxer                    tick 600
commands 21
run #1   f1127320cf799060
run #2   f1127320cf799060
OK: the command log replays identically
```

`run #1` can additionally be compared with the hash printed by `make`: if it matches,
the recorded game and the played-back one are the same state.

## 4. Format (to copy or reimplement it)

All integers are little-endian. Two containers: `gcsave` (wrapper) and `gcrep`
(pure log).

### `.gcrep` — `GCS1`, version 2

```
u32  magic  0x47435331 ("GCS1")
u16  version 2
u64  seed                 world RNG seed
u8[32] map_hash           SHA-256 of the CHK (0 if created by hand)
u32  cmd_count
cmd* command:
     u64 tick             tick at which it is applied (before step)
     u8  kind             0 = Issue, 1 = Train, 2 = Morph, 3 = MorphBuilding
     ... payload depending on kind
u8[32] sha256(payload)    checksum of everything above
```

Payload of each command:

| kind | payload |
|---|---|
| 0 `Issue` | `u32 unit` + order (`u8` tag + fields) |
| 1 `Train` | `u32 building` + `u16 unit_type` |
| 2 `Morph` | `u32 larva` + `u16 unit_type` |
| 3 `MorphBuilding` | `u32 source` + `u16 unit_type` |

Orders (`Order`, tag = declaration order): `0 Idle`, `1 Stop`,
`2 Move{pos}`, `3 AttackMove{pos}`, `4 AttackUnit{target}`, `5 HoldPosition`,
`6 Patrol{a,b,forward}`, `7 Gather{resource}`, `8 ReturnCargo{depot}`,
`9 Build{type,pos}`, `10 Repair{target}`, `11 Train{type}`, `12 Research{tech}`,
`13 Upgrade{id,level}`, `14 Cast{spell,target}`, `15 EnterTransport{unit}`,
`16 Unload`, `17 Burrow`, `18 Unburrow`, `19 Siege`, `20 Unsiege`, `21 Lift`,
`22 Land{pos}`, `23 Die`.

### `.gcsave` — `GCSF`, version 1

```
u32  magic  0x47435356 ("GCSF")
u16  version 1
str  map_name             u32 len + bytes (max. 1024)
u32  chk_len
u8[] map_chk              embedded CHK (max. 8 MiB)
u32  race_count
u8[] races                0 Zerg, 1 Terran, 2 Protoss, 3 Neutral
u32  local_player
u64  tick                 tick of the save
u32  rec_len
u8[] rec                  a complete `.gcrep`
u8[32] sha256(payload)
```

Limits the reader enforces before allocating memory: `chk_len ≤ 8 MiB`,
`cmd_count ≤ 10 000 000`, `race_count ≤ 12`. A file that fails the magic,
version, checksum or size check is rejected with `None` (it never
panics): see `tests/replay.rs::corrupt_file_is_rejected`.

## 5. Restoring: how it is reapplied

`replay_to(world, rec, upto_tick)` (in `src/replay.rs`) is the only
implementation of playback, shared by save, replay and LAN:

```text
while world.tick < upto_tick:
    apply the commands stamped at world.tick (in order)
    world.step()
apply the commands stamped exactly at upto_tick (no step)
```

That is why a loaded save and a watched replay use exactly the same
path as the live game, and `state_hash(&world)` must match the one from
the original session at the same tick (`tests/replay.rs`:

- `savefile_roundtrip_with_embedded_map`
- `boxer_with_ai_save_reload_replays_identically`).

## 6. Common problems

| Symptom | Cause | Fix |
|---|---|---|
| `cannot replay X: not a GalaxyCraft save/replay` | retail `.rep` file or corrupt file | use `sc_replay` to inspect StarCraft `.rep` files; GalaxyCraft only reads `gcrep`/`gcsave` |
| playback stops early | the `.gcrep` was recorded mid-game | this is normal: `tick` is the last recorded tick |
| the final hash does not match | the `.gcrep` was edited | recompute the checksum by recording again; `sc_replaytool verify` detects it |
