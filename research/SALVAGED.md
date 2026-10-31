# Procedencia: motor C eliminado (2026-10-28, estructura final)

Queda un solo motor (`../engine-rust/`). El árbol `../engine-c/` se eliminó
tras migrar todo lo útil (diseño save/load+replay+LAN, arneses de
determinismo). Verificación de aquella migración: listas idénticas
(354/354, 113/113), checksums agregados de `src/` idénticos, `Cargo.lock`
idéntico. Testigos: `re-c.bundle` en este mismo directorio (historial git) y
`docs/rea-evidence-c.md` (identidad del objetivo + cripto MPQ + CHK).

## Qué hay

- `../engine-rust/`: reconstrucción Rust íntegra SIN `target/` (regenerable
  con `cargo build`). Joyas: `src/handlers.rs` (137
  rutinas + direcciones runtime base `0x79fa0000` usadas como huella),
  `src/orders.rs` (189 filas + VA), `src/retail.rs` (lector CASC/MPQ),
  `src/trigger.rs` + `src/map.rs` (formatos CHK), `tools/` (oracle,
  `gen_handlers.py`, `verify_runtime_tables.py`, extractores C++),
  `fixtures/` (tablas de test), `docs/` (`COVERAGE.md`, `EVIDENCE.md`,
  evidencias), `features.md`, `Makefile`.
- Motor C (eliminado): su contenido útil ya vive portado en `engine-rust`
  (diseño y tests de save/replay/LAN). Testigos: `re-c.bundle` en este
  mismo directorio (historial git) y `docs/rea-evidence-c.md`.

## Qué NO se migró (y por qué)

- `target/` de Rust (1.4 GB): artefactos de compilación, regenerables.
- El árbol C: eliminado tras la migración; su historial queda en
  `re-c.bundle`.

## Nota de rutas

`~/Documents/galaxycraft` → `../engine-rust`,
`~/ReverseEngineering` → `.` (este workspace). El enlace
`~/Documents/sc-cpp` se eliminó con el motor C. Los documentos antiguos que
citan las rutas originales siguen resolviendo vía los enlaces restantes.
