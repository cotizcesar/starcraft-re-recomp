# Inventario de herramientas y capacidades reales

Verificado el 2026-10-28. "Verificado" significa ejecutado con resultado
observable, no documentado de oídas. Sin Hermes en la cadena de trabajo.

## REA (Reverse Engineer Anything) vía OpenCode

| Aspecto | Estado | Evidencia |
|---|---|---|
| Servidor MCP en OpenCode | **conectado** | `opencode mcp list` → `✓ rea connected` (2026-10-28) |
| Paquete | `rea-agents@6.8.0` vía `npx`, entrada `mcp.rea` en `~/.config/opencode/opencode.json`, `enabled: true` | config |
| Entorno Ghidra en OpenCode | **configurado** (`GHIDRA_INSTALL_DIR`, `JAVA_HOME`, `REA_GHIDRA_STARTUP_TIMEOUT_MS`) | `opencode.json` |
| Backend Ghidra (CLI) | **disponible** | `capabilities` con env → `status: available`, Ghidra 12.1.4, JDK 21.0.13, `analyzeHeadless` y decompiler nativo presentes |
| Ghidra instalado | 12.1.4 y 12.2_PUBLIC en `~/.local/share/ghidra/` (+ JDK Temurin 21.0.13+9) | `ls`, `java -version` |
| CLI `rea` | no existe en PATH; no se usa directo | `command -v rea` → vacío |
| Prueba E2E OpenCode→REA→Ghidra | **pendiente**: abrir un PE de prueba con `open_binary` + `binary_overview` desde OpenCode | Fase 3 |

Nota: registros previos en `~/ReverseEngineering/` mencionan Hermes como
cadena funcional. Quedan como histórico; no se usan ni se amplían.

## OpenCode

- v2.2.0, `~/.opencode/bin/opencode`.
- MCP activos: `gsd`, `godot`, `comfyui`, `rea` (los 4 `connected`).
- Modelo por defecto en config: `opencode-go/muse-spark-1.3-contributor`.
- Config activa: `~/.config/opencode/opencode.json`
  (existe `opencode.json.rea.backup` previo al parche de entorno).

## Análisis binario disponible

| Herramienta | Estado |
|---|---|
| `file`, `sha256sum`, `xxd`, `objdump` (binutils) | presentes |
| Ghidra 12.1.4 / 12.2_PUBLIC + JDK 21 | presentes en `~/.local/share/ghidra` |
| `analyzeHeadless` en PATH | no (usar ruta absoluta `.../support/analyzeHeadless`) |
| `pefile`, `capstone`, `lief` (Python) | ausentes (instalables en venv de usuario si hacen falta) |
| `binwalk` | ausente |
| Extracción CASC/MPQ | resuelto en este repo: `tools/casc_extract.cpp`, `tools/mpq_extract.cpp` + lector nativo `src/retail.rs` |
| `wine` | ausente (juego vía Lutris/Proton; no se ejecutan binarios fuera de ahí sin autorización) |

## Reglas de uso

- Nada de `sudo`; instalaciones en `~/.local` o venv de usuario.
- No ejecutar binarios del juego fuera de Lutris/Proton sin autorización.
- No tocar la instalación original; el trabajo escribe en este repo
  (`docs/ logs/ analysis/ extracted/ reconstructed/ tests/ build/`)
  y en el caché derivado borrable.
