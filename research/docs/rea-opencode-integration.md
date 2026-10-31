# Integración REA ↔ OpenCode — verificación real (sin Hermes)

Fecha: 2026-10-28. Solo cadena OpenCode. Hermes fuera de servicio por
decisión del proyecto; las menciones en `~/ReverseEngineering/` quedan como
histórico.

## 1. Estado actual

| Prueba | Comando | Resultado 2026-10-28 |
|---|---|---|
| OpenCode | `opencode --version` | v2.2.0 |
| MCP list | `opencode mcp list` | 4/4 `connected`: `comfyui`, `godot`, `gsd`, **`rea`** |
| Config `rea` | `~/.config/opencode/opencode.json` → `mcp.rea` | `command: npx -y rea-agents@6.8.0 mcp`, `enabled: true`, **con** bloque `environment` |
| Entorno | `GHIDRA_INSTALL_DIR`, `JAVA_HOME`, `REA_GHIDRA_STARTUP_TIMEOUT_MS=900000` | presentes en la entrada `rea` |
| Proveedor Ghidra (CLI, con env) | `capabilities` | `status: available`, Ghidra 12.1.4, JDK 21.0.13, `analyzeHeadless` y decompiler OK |
| Proveedor Ghidra (CLI, sin env) | `capabilities` | `not_configured` (confirma que el env es el que lo activa) |

El gap documentado en `~/ReverseEngineering/docs/rea-opencode-integration.md`
(entrada `rea` sin `environment`) **está cerrado**: el `opencode.json` actual
ya incluye el bloque.

## 2. E2E verificado (Fase 3 — PASS 2026-10-28)

`open_binary` + `binary_overview` + `binary_session` + `close_binary` desde
OpenCode vía MCP sobre copia `/tmp/opencode/test_launcher.exe` (original
intacto, sha256 `ab51787f…1963a`): **PASS**.

- `open_binary`: PE x86 OK.
- `binary_overview`: 20 862 procedimientos, 13 326 cadenas, 7 segmentos,
  `image_base 0x400000` (coincide con el histórico).
- `binary_session`: binding `ghidra 12.1.4` (`auto-single-candidate`,
  `analyzer_preset=ghidra-default`), grupos disponibles.
- `close_binary`: OK. Registro completo en `logs/integration-tests.log`.

## 3. Notas

- Ghidra 12.2 también instalado, pero REA pide pin 12.1.x; usar 12.1.4.
- `analyzeHeadless` solo por ruta absoluta; no está en PATH.
- No ejecutar binarios del juego fuera de Lutris/Proton sin autorización.

## 4. Documentación oficial

- Instalación: https://github.com/morluto/rea/blob/main/docs/installation.md
- Proyecto: https://github.com/morluto/rea
- Guías: https://morluto.github.io/rea/
