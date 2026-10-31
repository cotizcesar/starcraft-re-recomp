# Registro de decisiones

- 2026-10-28: Hermes fuera de la cadena. Motivo: simplificar a OpenCode+REA,
  verificado E2E. Las menciones en docs previos quedan como histórico.
- 2026-10-28: Estructura del master prompt (`original-manifest/`,
  `analysis/discovery/`, …) se adopta por adición; no se borran los
  artefactos previos (`manifests/`, `analysis/file-inventory.csv`) para no
  romper referencias.
- 2026-10-28: `GAME_PATH` = instalación Battle.net StarCraft 1.23.10.13515
  (confirmada por el usuario). Solo lectura.
- 2026-10-28: proveedor Ghidra 12.1.4 (pin REA); 12.2 instalado pero no usado.
- 2026-10-28: reconstrucción en Rust (`~/Documents/galaxycraft`) se reutiliza
  como implementación compatible; este directorio conserva evidencias y planes.
- 2026-10-28: LAN multijugador DENTRO de alcance (decisión del usuario).
  Battle.net/online sigue fuera. Diseño: lockstep determinista sobre el sim
  actual (LCG; solo comandos por turno + checksum periódico), lobby LAN,
  countdown, manejo de lag/drop, chat pre-partida. Aceptación: 1v1 2 PCs
  20+ min sin desync. Comparte infraestructura con replays P0 (log de
  comandos por tick).
- 2026-10-28: orden LAN: diseño de protocolo EN PARALELO con P0
  (implementación tras P0). Transporte: AMBOS (capa abstracta; TCP primero
  para correctitud, UDP con retransmisión después). Decisión del usuario.
