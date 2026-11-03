# Unresolved questions

1. Dump of the decrypted image: it EXISTS and is valid
   (`work/sc_mem_menu_pe.exe`, 17 620 992 B, `.text` entropy 6.8950 vs 7.9997
   on disk; menu state, 37 561 strings). What is missing is an in-game dump
   (post-initialization): recapture via Lutris + `/proc/<pid>/mem` with
   authorization? (P0 — extends Phase 2/4, does not fully block it.)
2. REA MCP on the dump: `binary_overview` gives a deterministic `provider_timeout`
   (Ghidra startup deadline; see `docs/blockers.md`). Alternative path:
   direct `analyzeHeadless` in progress (`logs/ghidra-dump-analysis.log`). (P0.)
2. x86-64 variant: **confirmed as the same engine** (P1 → P3). 17 216
   strings shared with the 32-bit dump, identical markers
   (`analysis/discovery/dump-manifest.md`). Logic comparison remains
   (pending Ghidra analysis of both dumps).
3. Launcher `Bootstrapper.pdb`: is the game's PDB available in memory?
   Check symbols after the dump. (P2.)
4. `.idx`/`.index` classified `unknown` in the inventory: rename to
   "CASC index" in the next inventory regeneration. (P3.)
5. Multiplayer/Battle.net: out of offline scope; only document limits.
   (P3.)
6. `opencode mcp list` CLI difference depending on cwd ("No MCP servers configured"
   from `research/` vs 4/4 connected from the repo): per-project
   config? Not blocking (the MCP session works), but document the cause. (P3.)
