# Engine identification — evidence

Status: **proprietary Blizzard engine (StarCraft 1.23 “classic”)** — main
hypothesis with converging evidence; it is not ruled out that the 64-bit executable
is a recompiled variant of the same engine.

## Observed evidence

| # | Observation | Command / source | Interpretation |
|---|---|---|---|
| E1 | `x86/StarCraft.exe` is `PE32 i386`, 12 820 096 B, 7 sections | `file`, `objdump -h` | Native Win32 x86 binary, no managed runtime |
| E2 | `x86_64/StarCraft.exe` is `PE32+ x86-64`, 15 734 912 B, 8 sections (with `.pdata`) | `file`, `objdump -h` | Second native 64-bit variant |
| E3 | `.text` with **entropy 8.000** in both binaries; `.rdata` in plain text | own computation over section offsets | The code is **encrypted/compressed on disk**; the data is not |
| E4 | Cleartext strings: `scripts\iscript.bin`, `scripts\aiscript.bin`, `glue\battle.net\`, `font\BLIZZARD-REGULAR.ttf`, `us.actual.battle.net` | string extraction | Resource names and names of the classic engine itself (iscript/aiscript are StarCraft formats) |
| E5 | `Data/data/data.000…005` (~5.4 GB) + `*.idx` + `Data/indices` + `Data/config` | `find`, `du`, `.build.info` | Blizzard **CASC** storage, not MPQ on disk |
| E6 | `Data/config/44/...` contains `# Patch Configuration` with `patch-entry = download <hash> <size> …` | reading the file | Blizzard CDN/patching metadata |
| E7 | `.build.info`: `Version = 1.23.10.13515`, `Product = …`, CDN `level3.blizzard.com` | reading | Exact build identified |
| E8 | `Maps/*.scm` (163) and `Maps/*.scx` (160) | inventory | Maps in classic MPQ format with `staredit\scenario.chk` |
| E9 | `StarCraft Launcher.exe` + `x86*/cef/` + `SceneCefBrowser.exe` | inventory | The **launcher UI** is Chromium Embedded Framework; the game is not |
| E10 | No traces of `UnityEngine`, `il2cpp`, `UnrealEngine`, `GodotEngine`, `mscorlib`/`BSJB` in the game executables | marker search | Rules out Unity (Mono/IL2CPP), Unreal, Godot and .NET for the game |

## Hypotheses and testing

- **H1 (main): proprietary StarCraft engine, native C/C++ x86 + x86-64, resources via CASC.**
  Evidence: E1–E9. Explains the formats (iscript/aiscript/CHK), the CEF launcher and the
  complete absence of commercial-engine markers.
- **H2: “Remastered-adjacent” build with dual binaries (32 and 64 bits) and protected code.**
  Evidence for: two executables, both with maximum-entropy `.text`. In favor of the
  same engine: the resource strings are identical in both.
- **H3 (rejected): Unity/Unreal/Godot.** No markers; furthermore the data formats
  are those of classic StarCraft.
- **H4 (partial, not the game): Electron/CEF in the launcher.** Only `SceneCefBrowser.exe`
  and CEF `*.pak` files; it is the UI/menu layer, not the game engine.

## Critical technical consequence

The code is encrypted at rest (E3). Therefore:

1. Disassembling `on-disk` with Ghidra produces garbage (entropy 8.0 is not code).
2. The valid path is a **read-only snapshot of the in-memory image**
   of the running process (`/proc/<pid>/mem` via Proton), or locating the
   decryption routine. Earlier work (in the former `~/Documents/galaxycraft` path, now this repo) already did this: the
   32-bit process is mapped at `0x79fa0000` and a 17 620 992 B image was obtained with
   decrypted, analyzable `.text`.
3. No attempt was made to bypass any protection: only the memory of the user's own
   legitimate process is read, on their machine, for analysis.

## Next discriminating experiment

With the dumped image, `mcp__rea__binary_overview` on the dump should return
functions with unwind/PDB names and not garbage. If the dump also comes out with entropy
8.0, the copy was taken before decryption (or from a wrong region) and the
capture must be repeated after engine initialization.
