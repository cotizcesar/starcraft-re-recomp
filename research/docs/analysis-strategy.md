# Analysis strategy (Phase Four)

Engine: native code (proprietary C/C++) + CASC/MPQ data. Path: **native with
REA+Ghidra for code; dedicated readers for data**. No Unity/Unreal/
Godot/.NET/Java/web that would justify other providers.

## Verification by format (representatives)

| Format | Representative | Tool | Result |
|---|---|---|---|
| PE x86 | `StarCraft Launcher.exe` (copy in `/tmp`) | REA+Ghidra 12.1.4 | PASS: 20 862 proc, 13 326 strings, base `0x400000` |
| PE x86 game | `x86/StarCraft.exe` (on disk, encrypted) | `file`/`objdump`/strings | valid PE, `.text` entropy ~8.0 → requires dump |
| CASC | `Data/data/data.000` + indices | `src/retail.rs` / casc_extract | tables/tiles/GRP/WAV read |
| MPQ maps | `Maps/*.scm` (323) | `src/retail.rs` / mpq_extract | 323/323 decompressed and parsed |
| iscript/aiscript | `scripts/*.bin` via CASC | `src/iscript.rs`, `src/aiscript.rs` | opcodes 0x00..0x44 + full AI |
| CHK triggers | `staredit\scenario.chk` | `src/trigger.rs`, `src/map.rs` | conditions 0..23, actions 1..59 |

## Scope

- Native: functions, xrefs, calls, symbols, strings, types, structures,
  pseudocode (Ghidra). Starting point: decrypted image (recapture
  pending; the 2026-10-27 dump is no longer on disk).
- Data: DAT, TBL, GRP tables, tilesets, WAV/Ogg, CHK (already decoded in
  `engine-rust/`; reuse, do not reinvent).
- Do not install: IDA, Hopper (absent and not needed), ILSpy/dotNet (no
  target), web/Java providers.

## Limits

- Without a current dump there is no new static analysis of the game (documented
  blocker, not a tool failure).
- Pseudocode ≠ original source; inferred names are marked.
