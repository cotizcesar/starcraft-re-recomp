# Evaluación de rescate (cerrada 2026-10-28: `engine-c` eliminado)

> Resultado: todo lo útil se migró; el árbol C se borró. Testigos:
> `research/re-c.bundle` (historial) y `research/docs/rea-evidence-c.md`.
> Este informe se conserva como análisis histórico.

Fecha: 2026-10-28. Pregunta: ¿hay algo salvable en esas carpetas para el
objetivo actual (P0 + LAN + validación)?

## Qué es cada cosa

- `galaxycraft` (Rust, sin git): reconstrucción de **reglas con fidelidad
  total** — 228 unidades, 130 armas, 189 órdenes, 137 handlers transcritos,
  triggers 1–59, iscript/aiscript completos, 241 tests, 73/73 tablas
  verificadas, 391 mapas. Le falta P0 (guardado/replays/perfil) y red.
- `sc-cpp` (C, git, 2 commits, 3 737 líneas, 22 bins, 368+ tests):
  reconstrucción **simplificada pero completa como juego** — sim
  determinista 24 Hz/512 entidades, DB de 44 unidades, CHK 68/68, MPQ
  crypto, fog/BFS, IA, 9 matchups, torneo, cliente jugable, `save/load`,
  `replay` y `net`. `docs/REA-EVIDENCE.md` con identidad del objetivo
  (SHAs coinciden con nuestra instalación).

## Veredicto: sí, salvable — patrones y arneses, NO tablas de datos

| Pieza sc-cpp | Estado real | Uso para galaxycraft |
|---|---|---|
| `sc_save.c` (18 lín.) | volcado crudo de structs, sin magic/versión | patrón a portar a Rust **con** magic+versión (P0) |
| `sc_replay.c` (31 lín.) | hashes de estado periódicos + comparación | arnés de verificación de determinismo y base del checksum anti-desync LAN |
| `sc_net.c` (13 lín.) | `socketpair` + envío de 1 orden int | semilla del `NetTransport` TCP del diseño LAN |
| `sc_match.c`, `play.c` | bucle de partida, torneo, cliente jugable | disciplina de tick fijo + barrido de mapas (alinear con `sc_campaign`) |
| DB 44 unidades | simplificada | **NO portar**: regresión frente a las 228 de galaxycraft |
| `docs/REA-EVIDENCE.md` | imports = 2 DLLs (`MessageBoxW`), entry `0x1400b0af0`, `.text 00aabfe4` | corrobora motor/protector (stub empaquetado mínimo) |

## Oportunidad de validación cruzada (recomendada)

Dos reconstrucciones independientes (Rust fidelidad-total + C simplificada)
que barren los mismos 68 mapas: test diferencial "mismo mapa, comparar
desenlaces" como validación de equivalencia barata (Fase 12). Registrar como
tarea pendiente.

## Límites honestos

- `sc-cpp` no sustituye el análisis del binario: su DB es aproximada y su
  replay es de hashes, no de comandos (no sirve como `.gcrep` jugable).
- El `net` actual es fondo de socket, no protocolo: el diseño
  `docs/lan-protocol.md` sigue necesario.
