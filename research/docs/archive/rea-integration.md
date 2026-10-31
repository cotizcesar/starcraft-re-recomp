# REA ↔ OpenCode — integración (sin Hermes)

Fecha: 2026-10-28. Hermes fuera de la cadena por decisión del proyecto.

## Estados (no equivalentes)

| Estado | Resultado |
|---|---|
| REA instalado | sí: `rea-agents@6.8.0` vía `npx` (config `mcp.rea` en `~/.config/opencode/opencode.json`, `enabled: true`, con bloque `environment`) |
| REA registrado en OpenCode | sí: herramientas `tools.rea.*` visibles en esta sesión |
| Servidor MCP iniciado | sí: responde (`binary_session`, `open_binary`, `binary_overview` ejecutados) |
| Herramientas visibles | sí: 200+ `tools.rea.*` |
| Proveedor Ghidra instalado | sí: 12.1.4 + JDK 21.0.13 en `~/.local/share/ghidra`, `analyzeHeadless` y decompiler verificados |
| Proveedor capaz con este objetivo | sí: PE x86 analizado E2E (ver abajo) |

## E2E PASS 2026-10-28

Copia de trabajo `/tmp/opencode/test_launcher.exe` (original intacto,
sha256 `ab51787f…1963a`): `open_binary` PE x86 OK → `binary_overview`
20 862 procedimientos, 13 326 cadenas, 7 segmentos, `image_base 0x400000` →
`binary_session` binding Ghidra 12.1.4 (`auto-single-candidate`,
`ghidra-default`) → `close_binary` OK. Registro: `logs/rea-readiness.log`
y `logs/integration-tests.log`.

## Documentación oficial

- Instalación: https://github.com/morluto/rea/blob/main/docs/installation.md
- Proyecto: https://github.com/morluto/rea
- Guías: https://morluto.github.io/rea/

No se modificó ninguna configuración en esta tarea (solo lectura + copia a
`/tmp` + análisis). La entrada `mcp.rea` ya traía el bloque `environment`
(`GHIDRA_INSTALL_DIR`, `JAVA_HOME`, `REA_GHIDRA_STARTUP_TIMEOUT_MS`).
