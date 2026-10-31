# GalaxyCraft — lo que falta para completar el juego

Inventario verificado el 2026-10-28 contra el código (`src/`), la cobertura
documentada (`docs/COVERAGE.md`, `docs/EVIDENCE.md`) y la suite
(`cargo test`: 141 lib + 15 integración, todo verde). Criterio: paridad con el
StarCraft 1.23.10.13515 comercial jugando offline de un jugador, más lo necesario
para que otros jugadores lo usen como juego completo, no como motor.

## Lo que ya está (no tocar)

- Reglas: daño, proyectiles (10 balas, 25 hit types), movimiento (A* octil,
  regiones, iscript walk), colisiones, economía, tech tree, hechizos, transportes,
  bunkers, Nydus, visión/niebla/detectores, triggers 1..59, IA de campaña
  (`aiscript`/`bwscript`), IA de escaramuza, iscript 0x00..=0x44.
- Datos: 73/73 tablas verificadas contra el juego vivo, 391 mapas (323 + 68 de
  campaña) simulados, tilesets, GRP, sonido/música/voces con `--install`.
- Cliente `sc_view` jugable: menú, briefing, misión, pausa, victoria/derrota,
  restart, oponente IA, HUD por raza, minimapa, hotkeys, opciones.

## P0 — Funcionalidad de juego base que hoy es stub

### 1. Guardar / cargar partida — HECHO
- Implementado como *command log + fast-forward* (`src/replay.rs`): el guardado
  lleva el `.chk` embebido, razas, tick y el log de comandos; cargar reconstruye
  el mundo y reaplica el log hasta el tick (`replay_to`), que es el mismo camino
  que usan repeticiones y LAN.
- `F5` guarda en `<dir>/saves/save-NNN.gcsave`, `F6` graba repetición en
  `<dir>/replays/replay-NNN.gcrep` (ver `docs/replays.md`). `--replay FILE` abre
  una repetición directamente desde la línea de comandos.
- Aceptación cubierta por `tests/replay.rs`
  (`savefile_roundtrip_with_embedded_map`,
  `boxer_with_ai_save_reload_replays_identically`): el hash de estado tras
  cargar coincide con el de la sesión original.

### 2. Repeticiones (replays) — HECHO
- Formato `.gcrep` (`GCS1` v2): semilla, hash del mapa, comandos por tick y
  SHA-256 final; el `.gcsave` (`GCSF` v1) lo envuelve con el `.chk` embebido.
  Especificación completa en `docs/replays.md`.
- Reproductor en el cliente (`Espacio` pausa, `+`/`-` velocidad, `Esc` corta),
  lista en `Multiplayer → Replays`, y `sc_view --replay FILE` para lanzarla
  desde la terminal. `sc_replaytool make|info|verify` graba una demo, resume el
  contenido y comprueba que dos pasadas dan el mismo hash.
- Aceptación cubierta: mismo log → mismo `state_hash`
  (`sc_replaytool verify`, `tests/replay.rs`).

### 3. Persistencia de perfil y progreso de campaña
- Estado: los perfiles (`Player`, `REA`, `LinKa`) viven solo en memoria
  (`sc_view.rs:600`); terminar una misión avanza en la sesión pero no se recuerda.
  Los hotkeys ya tienen archivo propio (`~/.cache/galaxycraft/hotkeys.bin`,
  GCHK v1 con SHA-256) y todas las unidades/edificios/razas se editan en el
  panel Hotkeys; faltan misiones desbloqueadas y opciones.
- Falta: archivo de perfil en disco (misiones desbloqueadas por episodio,
  opciones), creado/editable desde el menú `Registry`.
- Aceptación: cerrar el juego, abrirlo y seguir la campaña donde quedó.

## P1 — Multijugador (el bloque grande)

Hoy no hay ni una línea de red en `src/` (sin sockets): todo es 1 humano vs IA.

### 4. Multijugador LAN / directo
- Falta: protocolo (idealmente lockstep determinista sobre el sim actual:
  intercambiar solo comandos por turno, checksum periódico de estado),
  descubrimiento en LAN, lobby con slots humanos, countdown, manejo de lag/drop,
  y chat pre-partida.
- Aceptación: 2 PCs en LAN juegan melee 1v1 completo sin desync en 20+ min.

### 5. Battle.net / online
- Falta: todo (cuentas, ladder, matchmaking, NAT traversal, anti-cheat).
  Es un proyecto de infraestructura aparte; ni el cliente retail offline lo necesita.
- Decisión propuesta: declarar online fuera de alcance y apuntar a LAN + juego
  directo por IP como techo del multijugador.

### 6. Hotseat / segunda persona local
- Estado: el lobby cicla `Random/Terran/Zerg/Protoss/Computer` por slot, sin
  segunda entrada humana.
- Falta (solo si se quiere sin red): doble esquema de teclas y pase de turno.
  Recomendación: no hacerlo; el esfuerzo rinde más en LAN.

## P2 — Presentación y contenido

### 7. Cinemáticas a movimiento completo
- Estado: completo para los 28 filmes de la tabla retail (REA `0x140ae7ae0`).
  `src/cinematic.rs` reproduce en streaming con audio; `blizzard.webm` +
  `starintr.webm` al arrancar, `starxintr.webm` en Brood War y los
  `Pre<Race>_<act>`/`<Race><act>`/`x<Race>` alrededor de los actos de campaña.
  Salto con Enter/Esc y póster de respaldo sin `ffmpeg`.
- Aceptación: `SC_INSTALL=… cargo test --lib video::tests::` (28/28) y
  `--shot-screen intro-blizzard|intro-title|intro-expansion` (`docs/intro.png`).

### 8. Skins conmemorativas / Carbot
- Estado: consolas Terran/Zerg/Protoss stock + menús + voces (documentado como
  límite en `COVERAGE.md`).
- Falta: skins alternativas del 20.º aniversario y estilo Carbot.
- Nota: puramente cosmético, no afecta jugabilidad.

### 9. Creep pixel-exacto
- Estado: malla orgánica determinista propia (no existe GRP de creep en CASC).
- Falta: nada obligatorio; documentado como límite aceptado. Solo si se busca
  paridad visual cuadro a cuadro.

### 10. Audio sin instalación
- Estado: 9 WAV de respaldo incluidos; música (23 temas) y voces (1.122) solo
  con `--install`.
- Falta (opcional): nada de motor; es decisión de distribución (peso vs retail).
  Solo documentar qué oye un jugador sin instalación.

## P3 — Compatibilidad y pulido

### 11. Rutas de memoria EUD de StarCraft Remastered
- Estado: parseadas, intencionalmente no ejecutadas; ningún mapa distribuido
  las usa (`tools/trigger_ids.py`).
- Falta: nada, salvo que aparezcan mapas que las requieran. Mantener el chequeo.

### 12. Puertos y empaquetado
- Estado: binarios Linux x86_64; cliente con `minifb`/X11.
- Falta (según audiencia): builds Windows/macOS, AppImage/Flatpak, iconos,
  instalador que detecte la instalación retail, y `sc_assets --install` guiado.
- Aceptación: un jugador no técnico instala y juega sin terminal.

### 13. Accesibilidad y localización
- Estado: cliente enUS; sin reescalado de UI ni daltonismo ni subtítulos en cinemáticas.
- Falta: al menos esMX/esES si el público es hispano, y escala de interfaz.

## Explícitamente fuera de alcance (proponer como no-objetivos)

- Editor de mapas/campañas (el retail trae Campaign Editor; es otro producto).
- Battle.net, ladder y servicios online (ver §5).
- Compatibilidad con parches distintos de 1.23.10.13515 sin re-verificación.

## Orden de ataque sugerido

1. Guardar/cargar (§1) — desbloquea sesiones reales y es prerrequisito de replays.
2. Replays (§2) — reutiliza serialización y determinismo; además sirve para
   verificar desyncs del futuro multijugador.
3. Perfil + progreso (§3) — completa el bucle de campaña.
4. Cinemáticas (§7) — cierra la presentación.
5. LAN (§4) — el único bloque de red que vale la pena.
6. Empaquetado (§12) — cuando el juego esté redondo.
