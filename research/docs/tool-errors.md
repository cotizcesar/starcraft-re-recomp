# Errores de herramientas (registro)

## 1. REA `binary_overview` — `provider_timeout` en dump 17.6 MB (2026-10-28)

- Herramienta: `tools.rea.binary_overview` (servidor `rea-agents@6.8.0`,
  proveedor Ghidra 12.1.4 + JDK Temurin 21.0.13+9).
- Objetivo: `/home/cotizcesar/ReverseEngineering/work/sc_mem_menu_pe.exe`
  (PE32 x86, 17 620 992 B, `.text` descifrado verificado: entropía 6.8950).
- Error: `provider_timeout` / "Ghidra startup deadline elapsed". La respuesta
  de error supera el presupuesto MCP (10 MiB) → se exportó a
  `/tmp/opencode/dump-error-bundle.json` (33 012 076 B, 1 registro).
- Causa: deadline de arranque de Ghidra en el servidor MCP, no del análisis
  en sí (el log muestra `Headless startup complete (901 ms)` y el bridge
  sigue escribiendo stdout cuando expira el plazo). Determinista: 2 intentos
  idénticos; no se reintenta más por esta vía.
- Controles: el mismo flujo sobre PE de 4.8 MB (Launcher) da PASS el mismo
  día → el proveedor funciona; el factor es el tamaño/complejidad del dump.
- Alternativa en curso: `analyzeHeadless` directo con JDK 21 en fondo,
  `logs/ghidra-dump-analysis.log`. Sin `sudo`, sin tocar el original.
- Doc oficial: https://github.com/morluto/rea (guías y timeouts del servidor).

## 2. `analyzeHeadless` sin `JAVA_HOME` usa Java 27 de Homebrew

- Observado al probar sintaxis: Ghidra 12.1.4 requiere JDK 21. Fijado con
  `JAVA_HOME=/home/cotizcesar/.local/share/ghidra/jdk-21.0.13+9` en el
  lanzamiento en fondo. Sin impacto (solo aviso para futuras ejecuciones).

## 3. REA proveedor Ghidra — `provider_timeout` en consultas del dump de menú (2026-10-30)

- Objetivo: `research/work/sc_mem_menu_pe.exe` (PE32, 17 620 992 B,
  SHA-256 `7c0bd733…fcc5`), abierto con `tools.rea.open_binary` OK.
- Fallo: `search_strings`, `list_procedures` y el resto de consultas que
  arrancan el análisis del proveedor agotan el plazo (`provider_timeout`,
  respuesta >10 MiB) aunque `REA_GHIDRA_STARTUP_TIMEOUT_MS=900000`. El
  proveedor devuelve `analysis_provider_binding` Ghidra 12.1.4 y las
  capacidades como disponibles, pero no sirve resultados en el plazo del
  cliente.
- Control: el proyecto ya analizado `research/build/ghidra-proj32/DumpProj`
  abre y descompila en segundos con `analyzeHeadless -noanalysis`.
- Vía usada (autorizada por `AGENTS.md` §5): `GhidraDecompileBatch.java` /
  `GhidraXrefsToStrings.java` sobre ese proyecto. Evidencia:
  `analysis/investigations/main-menu-re.md`.

### 3.1 Resolución (2026-10-30)

Con `REA_GHIDRA_STARTUP_TIMEOUT_MS=3600000` el CLI de REA completa el
análisis del dump de 17.6 MB y descompila correctamente
(`rea decompile … 0x89cdf0`, Evidence `ev_b3f87de7…296911`). 900 s no
bastan; el proyecto Ghidra ya analizado sigue siendo la vía rápida para
consultas repetidas.
