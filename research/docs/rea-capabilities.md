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

## Tool × capability matrix (observed)

| Capability | REA+Ghidra | rea-dotnet | artifact-graph | CLI (`file`/`objdump`) | Repo readers |
|---|---|---|---|---|---|
| PE x86/x64 disassembly | yes (PASS) | no | no | partial (headers/strings) | no |
| Pseudocode decompilation | yes (Ghidra) | no (never C#) | no | no | `handlers.rs` transcriptions |
| Xrefs/callers/callees | yes | no | no | no | no |
| Managed .NET | n/a (no target) | yes (metadata) | no | no | n/a |
| CASC/BLTE/MPQ/DCL | no | no | partial (`extract_artifact`) | no | yes (`retail.rs`) |
| DAT/TBL/CHK/iscript/AI | no | no | no | no | yes |
| GRP/tileset/WAV/Ogg/DDS | no | no | no | no | yes (`render.rs`, `audio.rs`, `dds.rs`) |
| Dynamic/traces | no (REA is static) | no | no | no | Proton + observation |

External requirements: only Ghidra 12.1.4 + JDK 21 (present). Nothing else to
install for the critical path.
