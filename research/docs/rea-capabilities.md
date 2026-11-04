# REA capabilities observed in this session (without Hermes)

Source: `binary_session` without a target + `binary_session` with a PE x86 open +
CLI `capabilities` with env. Observed facts only.

## Ghidra 12.1.4 provider — available and selected

With a PE x86 target: `availability.status available`, selection
`auto-single-candidate`, `analyzer_preset=ghidra-default`. Available groups:
`address_name`, `inspect_native_data_type`, `inspect_native_instruction`,
`procedure_callees/_callers/_info`, `resolve_native_call_targets`,
`address_to_file_offset`, `analyze_function` (FunctionManager + Listing +
ReferenceManager + BasicBlock + decompiler; Ghidra pseudocode, not original
source), `inspect_native_load_image`, `list_documents/procedures/segments/
strings/names`, `procedure_assembly`, `procedure_pseudo_code`,
`procedure_references`, `read_bytes`, `read_function_instructions`,
`resolve_containing_procedure`, `search_bytes/procedures/strings`, `xrefs`,
annotations (`annotate_native_function`, `set_address_name(s)` — they mutate only
the ephemeral database, never the executable). Ephemeral Project and Program, deleted
on close. Hopper: `unavailable` (`/opt/hopper/bin/Hopper` absent);
not a blocker (Ghidra covers native).

## rea-dotnet-static v1 provider — available

`inspect_managed_artifact`, `inspect_managed_members`,
`inspect_managed_native_boundaries`. Only inventory/decoding of CLI
metadata; never loads assemblies or decompiles C#. Relevance for this
game: none (no .NET markers in the executables); kept in reserve.

## rea-artifact-graph v1 provider — partially available

Available: `extract_artifact`, `inspect_artifact`, `decode_interface_builder`,
`inspect_keyed_archive`, `trace_dylib_resolution` (depending on target).
Not available here: `inspect_asset_catalog` (requires macOS `assetutil`),
variants tied to `.app`/Macho. Intended use: artifact inventory,
not main disassembly.

## native-macos provider — not available (expected)

All `unsupported_host` (requires macOS/Xcode). No impact on Linux.

## Verified usage rules

- Discover the real name → read description/parameters → bounded query
  before escalating. That is how it was done (`open_binary` → `binary_overview` →
  `binary_session` → `close_binary`).
- Without an open target, Ghidra does not appear as a candidate (only after
  `open_binary` with a suitable format/architecture).
- `raw-binary` requires `architecture` + `entry_point`; PE is autodetected.

## Tool × capability matrix A–F (completed 2026-11-04)

Columns A–F: A = capability, B = REA + Ghidra (MCP), C = rea-dotnet,
D = artifact-graph, E = CLI (`file`/`objdump`/headless `analyzeHeadless`),
F = repository readers and scripts.  (The "A–F" label predates the
repository's first commit and is not defined anywhere in it; reading it as
these six columns is an INFERENCE.)  Rows 1–8 are the 2026-10-28
observations; rows 9–15 were added and every B/E/F cell of them observed on
2026-11-04 (session log entry 78).  C and D were not re-run: there is still
no .NET target, and CASC is fully covered by F.

| # | A: Capability | B: REA+Ghidra | C: rea-dotnet | D: artifact-graph | E: CLI / headless | F: Repo |
|---|---|---|---|---|---|---|
| 1 | PE x86/x64 disassembly | yes (PASS) | no | no | `objdump` linear sweep (headers/strings; desyncs on junk bytes, row 10) | no |
| 2 | Pseudocode decompilation | yes; images >10 MB: `provider_timeout` / `resource_constraint` | no (never C#) | no | `analyzeHeadless` + `GhidraDecompileBatch.java` (timeout/payload/instruction limits via env) | `handlers.rs` transcriptions |
| 3 | Xrefs/callers/callees | yes; on the 17 MB dump `resolve_containing_procedure` → `resource_constraint` | no | no | `RefsToAddr.java`, `FunctionProfile.java` | no |
| 4 | Managed .NET | n/a (no target) | yes (metadata) | no | no | n/a |
| 5 | CASC/BLTE/MPQ/DCL | no | no | partial (`extract_artifact`) | no | yes (`retail.rs`) |
| 6 | DAT/TBL/CHK/iscript/AI | no | no | no | no | yes |
| 7 | GRP/tileset/WAV/Ogg/DDS | no | no | no | no | yes (`render.rs`, `audio.rs`, `dds.rs`) |
| 8 | Dynamic/traces | no (REA is static here) | no | no | no | Proton + observation; `heap_capture.py` |
| 9 | Provider capability report | `binary_session`: Ghidra 12.1.4, all analysis groups `available` on the 32-bit dump | — | — | — | — |
| 10 | Protector-obfuscated code (junk, overlapping branches) | no (`0x8aee30` never returns) | no | no | decompiler fails with an empty error; flow listing + `objdump` show the junk | Python structural scans (branch-target / junk counts) |
| 11 | Imports of the protected dump (zeroed import dir, IAT → trampolines) | no names | no | no | `ImportRefs.java`: 0 named imports (the negative) | `.text` scan for `call [IAT]` signatures → pump located |
| 12 | CEF JavaScript bundle (`webui/dist`) | `analyze_javascript_application` completes but its result exceeds the 10 MiB MCP cap, even for one 297 KB panel | no | no | `grep` over the extracted bundle | `sc_assets list` / `path` extract it read-only |
| 13 | Extracted file → CASC/MPQ origin | no | no | no | no | `sc_assets manifest` (CKey = MD5), `research/manifests/extracted-origin.tsv` |
| 14 | Sound id → file (`SND_*` / `GLUSND_*`) | no | no | no | no | `sfx::SfxData` + `glusnd_path` (tests) |
| 15 | Runtime RW memory (map heap) | no | no | no | no | `tools/heap_capture.py` (`/proc/<pid>/mem`, read-only), `heap-ingame-findings.md` |

External requirements: only Ghidra 12.1.4 + JDK 21 (present). Nothing else to
install for the critical path.
