# Player profiles (Registry IDs): retail format and the `GCPF` store

Date: 2026-11-03 (session log entry 77).  Implementation:
`engine-rust/src/profile.rs`; client wiring: `engine-rust/src/bin/sc_view.rs`
(`load_profile_store`, `sync_profiles`, `record_game_result`).

## Sources

- Reference files, copied read-only to `/tmp` from the Wine prefix
  (`drive_c/users/steamuser/Documents/StarCraft/`): `Characters/REA.spc`
  (66 B, SHA-256 `a7190ed3…7643`), `Characters/LinKa.spc` (70 B,
  `ae9e6361…7bd7`), `Characters/LinKa.mpc` (68 B, `08cf7919…325b`),
  `CSettings.json` (1 545 B, `13c0843d…90e7`).  Not committed (user data).
- 32-bit menu dump `research/work/sc_mem_menu_pe.exe` (SHA-256
  `7c0bd733…fcc5`, image base `0x400000`, file offset = VA − `0x400000`).
  REA `open_binary` succeeded, but `resolve_containing_procedure` returned
  `resource_constraint` (response over the 10 MiB MCP budget, the >10 MB
  image limit of `tool-errors.md`).  Fallback per AGENTS.md §5:
  `analyzeHeadless -noanalysis -readOnly` on
  `research/build/ghidra-proj32/DumpProj` with `GhidraDecompileBatch.java`,
  `RefsToAddr.java`, `CreateFunctionAt.java`.
- Byte search for the magic `33 22 11 00` in the dump: three code sites,
  `0x6826b6` and `0x682b98` (`cmp … ,0x112233`) and `0x683066`
  (`mov dword [edi],0x112233`).

## Findings

| # | Finding | Evidence | Confidence |
|---|---|---|---|
| 1 | A Registry ID is the file `Characters\<name>.spc`, or `.mpc` when the byte `0x0119f7bc` (multiplayer flag) is set. | `0x00683080` builds `Characters\` + name + (`.mpc` if `[0x0119f7bc]` else `.spc`); strings `0x00d3ea60/68/70`. | FACT |
| 2 | The Registry lists IDs by enumerating that directory, skipping directories. | `0x00682580`: callback testing attribute bit `0x10` and copying the entry name (≤ `0x5f`); it contains the magic compare at `0x6826b6`. | FACT (callback) / INFERENCE (it feeds the list box) |
| 3 | The record is `0xd0` bytes, read as a `0x70` block then a `0x60` block; a legacy `0x34` head is accepted and the rest zeroed. | Loader `0x00682a80`: `FUN_00688290(rec,0x70,f)`, then `(rec+0x70,0x60)`; fallback `(rec,0x34)` + `memset(rec+0x34,0,0x9c)`. | FACT |
| 4 | The file name wins over the stored display name. | `0x00682a80`: `_stricmp(name, rec+0x70)`; on mismatch `0x00682540` copies the file name in. | FACT |
| 5 | New record: `memset(0,0xd0)`, name at `+0x0c` (budget `0x19` = 24 chars), name at `+0x70` (≤ `0x5f`), `time(NULL)` at `+8`, magic `0x00112233` at `+0`, `setne` of the multiplayer flag at `+4`. | `0x00683000`, disassembled by hand from the bytes at file `0x283000` (Ghidra cut the body after `memset`). | FACT |
| 6 | Block layout: `u32 check, u32 chunks, chunks × {u32 size, data}`; 8 KiB chunks; raw when `size == min(rest, 0x2000)`, zlib otherwise. | `0x00688290` (reader); zlib streams `78 9c` in all files. | FACT |
| 7 | `check = !crc32(raw block)`. | Holds for all 6 blocks of the 3 files (e.g. `REA.spc` head: `crc32 = 0x3f558242`, stored `0xc0aa7dbd`). Unit test `retail_block_check_is_inverted_crc32`. | FACT |
| 8 | Campaign progress: six `u32` at `+0x28` = original Zerg, Terran, Protoss, then Brood War Zerg, Terran, Protoss; value = highest mapdata mission reached, 0 = never started. | `0x007a3940`: `progress[race] = max(progress[race], entry.mission)` with Brood War at `+12`, then saves (`0x00682e20`); `0x006eaac0` lock checks (`terran < 12`, `zerg < 22`) give the race order; `LinKa.spc +0x2c = 10` = the live `9. New Gettysburg` list. | FACT (original three, `+0x2c` Terran) / INFERENCE (Brood War order: same race index at `+12`; falsifier: win a Brood War mission and re-read `+0x34..+0x40`) |
| 9 | Retail records progress when a mission is *started* (`OnPlayCampaignMission` path `0x007a29f0`), this client when it is *won*. | `0x007a29f0` updates and calls `0x00682e20` before launching. | INFERENCE (falsifier: start, then abort a new mission and re-read the `.spc`) |
| 10 | No statistics in the record: every byte after `+0x40` of the three files is 0; settings are global (`CSettings.json`). | Hex dumps. | FACT for these files / INFERENCE in general (falsifier: play a LAN game and diff `LinKa.mpc`) |

The writer `0x00682e20` was decompiled only up to the path build (Ghidra
treats the copy helper as non-returning); the write layout is taken from
the reader and the files.  This client's writer emits zlib chunks, and a
raw chunk when compression does not shrink it, both of which the retail
reader accepts.

## This client's store: `profiles.bin` (`GCPF` v1)

Next to `hotkeys.bin` (`$GALAXYCRAFT_SAVES` or the user cache root).

```
"GCPF" u16 version=1 u16 count u16 active (0xffff = none)
count × { u8 len, name (1..=24 B), u64 created, 6 × u16 progress,
          u8 n, n × u64 stats }
u16 settings × { u8 len, key (≤ 64 B), f32 value }
SHA-256 of everything above
```

- Caps: 256 KiB file, 256 IDs, 256 settings; anything malformed, a bad
  checksum, a duplicate (case-insensitive) or invalid name, or an
  out-of-range active index loads as `None` (tests
  `store_rejects_corruption_and_oversize`).
- Registry ID rules follow findings 1, 4 and 5: 1..24 bytes, unique
  ignoring ASCII case, no path or Windows-reserved characters (it is a file
  name in retail).
- Stats (`n` = 16 today; older files load with the missing tail at 0):
  games, wins, losses, games by race ×3, wins by race ×3, kills, deaths,
  razings, minerals gathered, gas gathered, best total score, ticks played.
  This client's own data, not a retail format (finding 10).
- Settings are the `GameOptions` fields by name (bools 0/1, modes their
  index); unknown keys are skipped, values clamped to what the UI produces.
- Migration: without `profiles.bin`, the reference Registry (`REA`, `LinKa`)
  is created and the progress of a previous `campaign.bin` (`GCPR`) is
  carried over.
- Retail import: `sc_view --import-characters DIR` reads every `*.spc`
  (read-only).  New IDs take the retail progress; an existing ID keeps the
  further of the two per campaign.

## When the client writes

`sync_profiles` (every `update`) writes when the serialised store differs
from the last write: Registry New ID / Delete / Ok, the end of every game
(statistics; progress on a campaign win), and settings once the Options
dialog is closed.  `flush_profiles` also runs when the window closes and at
the end of a menu script.  Headless shots and `--start-screen` never read or
write the player's file; menu scripts write only into an explicit
`GALAXYCRAFT_SAVES`.  Replays never count as games played.
