# Toolchain (sin Hermes)

Verificado 2026-10-28 en Bazzite 44.20261026.0. Cadena: OpenCode + REA MCP.

| Bloque | Herramienta | Estado |
|---|---|---|
| Orquestación | OpenCode v2.2.0 | activo; REA disponible como MCP en esta sesión |
| Análisis nativo | REA `rea-agents@6.8.0` + Ghidra 12.1.4 + JDK Temurin 21.0.13+9 | E2E PASS 2026-10-28 (20 862 proc / 13 326 strings, Launcher PE) |
| Binario | `file`, `objdump`, `sha256sum`, `xxd` | presentes |
| Datos CASC/MPQ | `src/retail.rs` (lector nativo en `engine-rust/`), `tools/casc_extract.cpp`, `tools/mpq_extract.cpp` | funcionales |
| Scripts/análisis | Python 3.14, `tools/oracle.py`, `tools/trigger_ids.py` | presentes; `pefile`/`capstone`/`lief` ausentes |
| Build/run | Rust 1.99.1, gcc 16, `podman`/`distrobox` | presentes |
| Ejecución juego | Lutris/Proton (sin `wine` en PATH) | solo entornos aislados, con autorización |

Nota: `opencode mcp list` ejecutado vía shell desde `~/ReverseEngineering`
devuelve "No MCP servers configured" (variante por directorio de trabajo);
en esta sesión de OpenCode las herramientas `tools.rea.*` están registradas,
el servidor arranca y Ghidra analiza (E2E PASS). El estado que vale es el de
la sesión, documentado en `docs/archive/rea-integration.md` y `logs/rea-readiness.log`.

Nada requiere `sudo`; todo vive en el home. Ghidra 12.2 también instalado,
pero REA fija 12.1.x: usar 12.1.4. `analyzeHeadless` solo por ruta absoluta.

## Reglas de uso

- Instalaciones en `~/.local` o venv de usuario; nada de `sudo`.
- No ejecutar binarios del juego fuera de Lutris/Proton sin autorización.
- No tocar la instalación original; el trabajo escribe en este repo
  (`docs/`, `logs/`, `analysis/`, `build/`) y en el caché derivado borrable.
- `tool-inventory.md` (E2E "pendiente", superado) archivado en
  `docs/archive/`; este archivo es el canónico.
