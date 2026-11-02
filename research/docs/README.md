# research/docs — índice

Orden de lectura para una sesión nueva. Canónicos vigentes; `archive/`
es histórico (citado por el session-log, no usar como referencia).

## Entrada (leer primero)

| Doc | Qué es |
|---|---|
| `PROJECT-STATUS.md` | Estado vivo del proyecto (objetivo, fase, pendientes) |
| `architecture.md` | Mapa verificado del binario (puerta, WndProc, sim, nuke) |
| `../../AGENTS.md` §5 | Reglas REA/Ghidra |
| `blockers.md` | Bloqueos activos y vías alternativas |

## Por tema

| Tema | Doc |
|---|---|
| Toolchain + reglas de uso REA | `toolchain.md` |
| Integración REA↔OpenCode (E2E PASS) | `rea-opencode-integration.md` |
| Capacidades observadas por proveedor | `rea-capabilities.md`, `tool-capability-matrix.md` |
| Errores de herramientas (timeouts, JDK) | `tool-errors.md` |
| Plataforma/ejecutables/CASC | `runtime-and-platform.md` |
| Inventario del juego (724 archivos) | `file-inventory.md` |
| Formatos y sus lectores | `format-coverage.md` |
| Estrategia de análisis | `analysis-strategy.md` |
| Protocolo LAN | `lan-protocol.md` |
| Paridad visual vs retail | `gap-audit.md`, `parity-report.md`, `graphics-remastered.md` |
| Datos completos de Hotkeys (220 acciones, 88 unidades) | `hotkeys-data.md` |
| Hallazgos de heap en partida | `heap-ingame-findings.md` |
| RE de episodios + modal de misión (1:1, 43/43) | `../analysis/investigations/episode-screen-re.md` |
| RE de las pantallas CEF (Registry/Collections/Saved/Replays/Lobby, 43/43) | `../analysis/investigations/cef-screens-re.md` |
| Decisiones con fecha | `decision-log.md` |
| Preguntas abiertas | `unresolved-questions.md` |
| Motor C eliminado (histórico) | `rea-evidence-c.md`, `salvage-sc-cpp.md`, `../SALVAGED.md`, `../re-c.bundle` |
| Bitácora append-only | `session-log.md` |
| Entorno Fase 0 (histórico) | `environment.md` |
| Identificación del motor (histórico) | `engine-identification.md` |
