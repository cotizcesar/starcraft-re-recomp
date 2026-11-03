# Tool errors (log)

## 1. REA `binary_overview` — `provider_timeout` on 17.6 MB dump (2026-10-28)

- Tool: `tools.rea.binary_overview` (server `rea-agents@6.8.0`,
  provider Ghidra 12.1.4 + JDK Temurin 21.0.13+9).
- Target: `research/work/sc_mem_menu_pe.exe`
  (PE32 x86, 17 620 992 B, decrypted `.text` verified: entropy 6.8950).
- Error: `provider_timeout` / "Ghidra startup deadline elapsed". The error
  response exceeds the MCP budget (10 MiB) → exported to
  `/tmp/opencode/dump-error-bundle.json` (33 012 076 B, 1 record).
- Cause: Ghidra startup deadline in the MCP server, not the analysis
  itself (the log shows `Headless startup complete (901 ms)` and the bridge
  is still writing stdout when the deadline expires). Deterministic: 2
  identical attempts; not retried again through this path.
- Controls: the same flow on a 4.8 MB PE (Launcher) PASSes the same
  day → the provider works; the factor is the size/complexity of the dump.
- Alternative in progress: direct `analyzeHeadless` with JDK 21 in the background,
  `logs/ghidra-dump-analysis.log`. No `sudo`, original untouched.
- Official docs: https://github.com/morluto/rea (server guides and timeouts).

## 2. `analyzeHeadless` without `JAVA_HOME` uses Homebrew's Java 27

- Observed while testing syntax: Ghidra 12.1.4 requires JDK 21. Pinned with
  `JAVA_HOME=$HOME/.local/share/ghidra/jdk-21.0.13+9` in the
  background launch. No impact (just a note for future runs).

## 3. REA Ghidra provider — `provider_timeout` on menu-dump queries (2026-10-30)

- Target: `research/work/sc_mem_menu_pe.exe` (PE32, 17 620 992 B,
  SHA-256 `7c0bd733…fcc5`), opened with `tools.rea.open_binary` OK.
- Failure: `search_strings`, `list_procedures` and the other queries that
  start the provider's analysis time out (`provider_timeout`,
  response >10 MiB) even with `REA_GHIDRA_STARTUP_TIMEOUT_MS=900000`. The
  provider returns `analysis_provider_binding` Ghidra 12.1.4 and the
  capabilities as available, but does not serve results within the
  client deadline.
- Control: the already-analyzed project `research/build/ghidra-proj32/DumpProj`
  opens and decompiles in seconds with `analyzeHeadless -noanalysis`.
- Path used (authorized by `AGENTS.md` §5): `GhidraDecompileBatch.java` /
  `GhidraXrefsToStrings.java` on that project. Evidence:
  `analysis/investigations/main-menu-re.md`.

### 3.1 Resolution (2026-10-30)

With `REA_GHIDRA_STARTUP_TIMEOUT_MS=3600000` the REA CLI completes the
analysis of the 17.6 MB dump and decompiles correctly
(`rea decompile … 0x89cdf0`, Evidence `ev_b3f87de7…296911`). 900 s is not
enough; the already-analyzed Ghidra project remains the fast path for
repeated queries.
