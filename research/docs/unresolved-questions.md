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
7. Building/mineral point lights — **recovered** 2026-11-04 from a live
   capture and `anim/main.lit` (`hd-lighting.md`).
8. Brood War `.spc` slot order — **FACT** from the `MenuEntry` race bytes
   (`profile-format.md` #8).
9. Message-pump slot names — **FACT**, resolved in the live 64-bit client
   (`architecture.md` §2, `tools/iat_trampolines.py`).
10. Campaign progress is recorded on mission start — **FACT** (live `.spc`
    diff, `profile-format.md` #9).  Consequence not yet mirrored by the
    client: the locked-campaign prompt (`0x006eaac0`) only appears while
    the campaign was never started (progress 0); see `PROGRESS.md`.
11. Classic glue screen transitions — **FACT** from a 60 fps retail
    recording (session log 84): plates slide in ×0.75 per 1/30 s step and
    out by 2^n − 1 UI px; static RE had hit protected code (session log 83).

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
   - Trigger text lifetimes: measured once (2026-11-04, Wasteland: TIP
     9.5-11.2 s, SCV transmission 6.7-8.5 s) and applied
     (`transmission::text_life_ms`); more samples would turn them into a
     rule.  The "always display" flag `0x04` is still untested: play a
     flagged message with subtitles off.
   - Transmission duration modifiers (`trigger::transmission_ms`).
   - The per-sprite lighting mix and lighting scale uniforms
     (`hd-lighting.md` #10).
5. "Matrix A–F": the original definition is not in the repository; it was
   read as the six columns of the tool × capability matrix
   (`rea-capabilities.md`).  Confirm with the project owner.
