# Bloqueos

1. [ACTIVO] REA MCP `binary_overview` sobre el dump de 17.6 MB
   (`work/sc_mem_menu_pe.exe`): `provider_timeout` determinista (2 intentos,
   2026-10-28) — "Ghidra startup deadline elapsed" (JVM sí arranca: log
   `AnalyzeHeadless ... Headless startup complete (901 ms)`, pero el análisis
   supera el deadline del servidor MCP). Evidencia exportada a
   `/tmp/opencode/dump-error-bundle.json` (32 MB, 1 registro
   `mcp_tool_error`). El PE pequeño (4.8 MB) pasa E2E sin problema.
   Alternativas: (a) `analyzeHeadless` directo con JDK 21 — en curso,
   `logs/ghidra-dump-analysis.log`; (b) servidor MCP con
   `REA_GHIDRA_STARTUP_TIMEOUT_MS` mayor; (c) no repetir `binary_overview`
   sobre este dump hasta que (a)/(b) cambien el diagnóstico.
2. [VIGENTE] Sin dump en juego (post-inicialización): el dump existente es de
   estado menú (37 561 strings vs 50 655 en disco). La recaptura requiere
   lanzar el juego vía Lutris + lectura `/proc/<pid>/mem` (solo lectura) con
   autorización del usuario.
3. [CERRADO] Entropía del dump: verificación de sección completa confirma
   `.text` descifrado (dump 6.8950 vs disco 7.9997). La lectura de muestra de
   200 KB (7.999) era engañosa; el dump es válido para análisis.
