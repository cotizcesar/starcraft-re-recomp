# Preguntas sin resolver

1. Dump de la imagen descifrada: EXISTE y es válido
   (`work/sc_mem_menu_pe.exe`, 17 620 992 B, `.text` entropía 6.8950 vs 7.9997
   en disco; estado menú, 37 561 strings). Lo que falta es un dump en juego
   (post-inicialización): ¿recapturar vía Lutris + `/proc/<pid>/mem` con
   autorización? (P0 — amplía Fase 2/4, no la bloquea por completo.)
2. REA MCP sobre el dump: `binary_overview` da `provider_timeout` determinista
   (Ghidra startup deadline; ver `docs/blockers.md`). Vía alternativa:
   `analyzeHeadless` directo en curso (`logs/ghidra-dump-analysis.log`). (P0.)
2. Variante x86-64: **confirmada como mismo engine** (P1 → P3). 17 216
   cadenas compartidas con el dump de 32 bits, marcadores idénticos
   (`analysis/discovery/dump-manifest.md`). Queda comparar lógica
   (pendiente de análisis Ghidra de ambos dumps).
3. `Bootstrapper.pdb` del lanzador: ¿PDB del juego disponible en memoria?
   Verificar símbolos tras dump. (P2.)
4. `.idx`/`.index` clasificados `desconocido` en el inventario: renombrar a
   "índice CASC" en la próxima regeneración del inventario. (P3.)
5. Multijugador/Battle.net: fuera de alcance offline; solo documentar límites.
   (P3.)
6. Diferencia CLI `opencode mcp list` según cwd ("No MCP servers configured"
   desde `~/ReverseEngineering` vs 4/4 connected desde el repo): ¿config por
   proyecto? No bloquea (la sesión MCP funciona), pero documentar causa. (P3.)
