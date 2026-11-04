# Blockers

State as of 2026-11-04.  Nothing blocks the reconstruction; the two open
items in `PROGRESS.md` need resources this machine does not have (see
"Needs the user").

1. [MITIGATED] REA MCP on the large dumps (`work/sc_mem_menu_pe.exe`,
   17.6 MB): `binary_overview` gave a deterministic `provider_timeout`
   (2026-10-28, "Ghidra startup deadline elapsed"); on 2026-11-04
   `open_binary` worked but `resolve_containing_procedure` and
   `analyze_javascript_application` returned `resource_constraint` (result
   over the 10 MiB MCP response budget).  Working path: `analyzeHeadless
   -noanalysis -readOnly` on the analysed projects with the scripts in
   `research/tools/` (`tool-errors.md`).  Do not repeat the failing MCP
   calls unchanged.
2. [CLOSED 2026-10-28] No in-game dump: the 64-bit in-game dump and the RW
   heap capture exist (`heap-ingame-findings.md`).
3. [CLOSED] Dump entropy: full-section verification confirms decrypted
   `.text` (dump 6.8950 vs disk 7.9997); the 200 KB sample (7.999) was
   misleading.

## Needs the user

- A second machine on the same LAN for `scripts/lan_two_machines.sh`.
- The retail game running (Lutris) for read-only memory reads: building /
  mineral point lights (in-game parity) and the inferences listed in
  `unresolved-questions.md` (Open #4).
