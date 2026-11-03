# Blockers

1. [ACTIVE] REA MCP `binary_overview` on the 17.6 MB dump
   (`work/sc_mem_menu_pe.exe`): deterministic `provider_timeout` (2 attempts,
   2026-10-28) — "Ghidra startup deadline elapsed" (the JVM does start: log
   `AnalyzeHeadless ... Headless startup complete (901 ms)`, but the analysis
   exceeds the MCP server deadline). Evidence exported to
   `/tmp/opencode/dump-error-bundle.json` (32 MB, 1 record
   `mcp_tool_error`). The small PE (4.8 MB) passes E2E without problems.
   Alternatives: (a) direct `analyzeHeadless` with JDK 21 — in progress,
   `logs/ghidra-dump-analysis.log`; (b) MCP server with a larger
   `REA_GHIDRA_STARTUP_TIMEOUT_MS`; (c) do not repeat `binary_overview`
   on this dump until (a)/(b) change the diagnosis.
2. [CURRENT] No in-game dump (post-initialization): the existing dump is of
   the menu state (37 561 strings vs 50 655 on disk). Recapture requires
   launching the game via Lutris + reading `/proc/<pid>/mem` (read-only) with
   user authorization.
3. [CLOSED] Dump entropy: full-section verification confirms
   decrypted `.text` (dump 6.8950 vs disk 7.9997). The 200 KB sample
   read (7.999) was misleading; the dump is valid for analysis.
