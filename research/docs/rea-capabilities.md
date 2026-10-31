# Capacidades REA observadas en esta sesión (sin Hermes)

Origen: `binary_session` sin objetivo + `binary_session` con PE x86 abierto +
`capabilities` CLI con env. Solo hechos observados.

## Proveedor Ghidra 12.1.4 — disponible y seleccionado

Con objetivo PE x86: `availability.status available`, selección
`auto-single-candidate`, `analyzer_preset=ghidra-default`. Grupos disponibles:
`address_name`, `inspect_native_data_type`, `inspect_native_instruction`,
`procedure_callees/_callers/_info`, `resolve_native_call_targets`,
`address_to_file_offset`, `analyze_function` (FunctionManager + Listing +
ReferenceManager + BasicBlock + decompiler; pseudocódigo Ghidra, no fuente
original), `inspect_native_load_image`, `list_documents/procedures/segments/
strings/names`, `procedure_assembly`, `procedure_pseudo_code`,
`procedure_references`, `read_bytes`, `read_function_instructions`,
`resolve_containing_procedure`, `search_bytes/procedures/strings`, `xrefs`,
anotaciones (`annotate_native_function`, `set_address_name(s)` — mutan solo
la base efímera, nunca el ejecutable). Proyecto y Program efímeros, se
eliminan al cerrar. Hopper: `unavailable` (`/opt/hopper/bin/Hopper` ausente);
no es bloqueo (Ghidra cubre nativo).

## Proveedor rea-dotnet-static v1 — disponible

`inspect_managed_artifact`, `inspect_managed_members`,
`inspect_managed_native_boundaries`. Solo inventario/decodificación de
metadatos CLI; nunca carga ensamblados ni descompila C#. Relevancia para este
juego: nula (sin marcadores .NET en los ejecutables), queda como reserva.

## Proveedor rea-artifact-graph v1 — parcialmente disponible

Disponibles: `extract_artifact`, `inspect_artifact`, `decode_interface_builder`,
`inspect_keyed_archive`, `trace_dylib_resolution` (según objetivo).
No disponibles aquí: `inspect_asset_catalog` (requiere macOS `assetutil`),
variantes ligadas a `.app`/Macho. Uso previsto: inventario de artefactos,
no desensamblado principal.

## Proveedor native-macos — no disponible (esperado)

Todo `unsupported_host` (requiere macOS/Xcode). Sin impacto en Linux.

## Reglas de uso verificadas

- Descubrir nombre real → leer descripción/parámetros → consulta acotada
  antes de escalar. Así se hizo (`open_binary` → `binary_overview` →
  `binary_session` → `close_binary`).
- Sin objetivo abierto, Ghidra no aparece como candidato (solo tras
  `open_binary` con formato/arquitectura adecuados).
- `raw-binary` exige `architecture` + `entry_point`; PE se autodetecta.
