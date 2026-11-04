# Unresolved questions

State as of 2026-11-04.  Closed items keep one line with the answer and
where it is recorded; open items carry the experiment that settles them.

## Closed

1. In-game (post-initialization) dump — **done** 2026-10-28:
   `work/sc_mem_ingame64_20261028_0814_pe.exe` plus the RW heap capture
   `work/heap_ingame_20261028_1237/` (map text and objectives found;
   `heap-ingame-findings.md`, re-verified 2026-11-04).
2. REA MCP on the large dumps — **mitigated**: `binary_overview` /
   `resolve_containing_procedure` stay over the MCP deadline / response
   budget on >10 MB images; the analysed headless Ghidra projects
   (`research/build/ghidra-proj32|64`) are the working path
   (`tool-errors.md`, `rea-capabilities.md` matrix rows 2, 3, 12).
3. x86-64 variant — **same engine** (H2, `architecture.md` §5).
4. Message pump — **located**: `FUN_00946500` (PeekMessage), per frame from
   `0x6bbdfa` (`architecture.md` §2).
5. `0x8aee30` — **analysed**: protector-obfuscated body (`architecture.md` §1).
6. Multiplayer/Battle.net — out of scope (LAN only; `PROGRESS.md`).

## Open

1. Game PDB (`Bootstrapper.pdb` is the launcher's): whether the dumps carry
   game symbols was never checked.  Not needed for the reconstruction.
   (P3.)
2. `.idx`/`.index` classified `unknown` in `manifests/inventory.csv`:
   rename to "CASC index" at the next inventory regeneration. (P3.)
3. `opencode mcp list` reports "No MCP servers configured" from `research/`
   but 4/4 from the repository root (per-directory config lookup).  Not
   blocking. (P3.)
4. INFERENCES that need the retail client running (each also recorded where
   it is used):
   - Brood War campaign slot order in `.spc` (`profile-format.md` #8): win
     a Brood War mission, re-read `+0x34..+0x40`.
   - Retail records campaign progress on mission start (`profile-format.md`
     #9): start and abort a new mission, re-read the `.spc`.
   - Message-pump slot names (`architecture.md` §2): resolve the trampoline
     behind IAT slot `0xd3063c` in a live process.
   - Trigger text lifetime `TEXT_MS` and the "always display" flag `0x04`
     (`src/transmission.rs`, `src/trigger.rs`): time a retail Display Text
     line; play a flagged message with subtitles off.
   - Transmission duration modifiers (`trigger::transmission_ms`).
   - Building/mineral point lights for in-game parity (`PROGRESS.md`,
     in-game frame parity): read the light inputs from a running game.
5. "Matrix A–F": the original definition is not in the repository; it was
   read as the six columns of the tool × capability matrix
   (`rea-capabilities.md`).  Confirm with the project owner.
