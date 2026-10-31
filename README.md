# starcraft-re-recomp — Reconstrucción jugable de StarCraft 1.23.10.13515

Reconstrucción **verificable y reproducible** del StarCraft 1.23.10.13515
comercial (build del `.build.info`), a partir del ejecutable y sus datos
CASC/MPQ. El original (`~/Games/battlenet/.../StarCraft`, 5.5 GB) **no**
está aquí y nunca se modifica: solo se lee.

## Estructura (una carpeta, un motor activo)

| Ruta | Qué es | Estado |
|---|---|---|
| `engine-rust/` | Reconstrucción en Rust: reglas de fidelidad total + cliente jugable `sc_view` | **Activo** — 246 tests verdes |
| `research/` | Ingeniería inversa: inventario, dumps descifrados, proyectos Ghidra, 116k funciones, evidencias, planes (incluye `re-c.bundle`: historial del motor C eliminado, clona limpio) | Fase 5 cerrada en mapa |
| `AGENTS.md` | Contrato del proyecto (principios, reglas, Definition of Done) | — |
| `research/docs/PROJECT-STATUS.md` | Estado vivo del proyecto (16 campos) | — |

Rutas antiguas resueltas por enlaces: `~/ReverseEngineering` → `research/`
(el enlace legacy `~/Documents/galaxycraft` se eliminó el 2026-10-29).

## Lo que hay (con prueba)

**Motor Rust (`engine-rust/`):**
- **Reglas completas**: daño (matriz, escudos, armadura, tamaños), 10 balas /
  25 hit types de proyectiles, movimiento A* octil + regiones + ciclo
  iscript, colisiones, economía (8 minerales/viaje, gas, supply 200),
  tech tree cableado, 25+ hechizos, transportes, bunkers, Nydus, visión /
  niebla / detectores, triggers clásicos (cond. 0–23, acciones 1–59), IA de
  campaña (`aiscript`/`bwscript`) e IA de escaramuza, iscript 0x00–0x44.
- **Datos verificados**: 73/73 tablas idénticas al juego vivo; 323 mapas MPQ
  + 68 de campaña simulan (0 fallos); tilesets, GRP, WAV/Ogg/voces vía
  `--install` (nada retail en el repo salvo 9 WAV + tiles de conveniencia).
- **Guardado y replays (P0 parcial)**: formato `.gcsave`/`.gcrep` con
  magic+versión+SHA-256 (command log + fast-forward determinista); **F5**
  guardar, **F6** guardar replay; pantallas Saved Games/Replays con
  lista/carga/borrado; reproductor con **SPACE** pausa, **+/-** velocidad,
  **R** reinicio, **ESC** salir. Test de ida y vuelta: guardar en tick T,
  recargar, **hash idéntico** (incluido Boxer+IA, 300 ticks).
- **Cliente jugable** `sc_view`: menú Remastered, briefing, misión, pausa,
  victoria / derrota, HUD por raza, minimapa, hotkeys, niebla, leaderboard.
  **Resolución**: 16:9 hasta 4K y ultrawide (la escala sale de la altura y
  el mundo ocupa todo el ancho). Detalle y límites en
  `research/docs/graphics-remastered.md`.
- **LAN (lockstep, TCP + UDP)**: `src/net.rs` + binario `sc_lan` + lobby
  dentro de `sc_view` (tecla **L**). Solo se intercambian comandos;
  checksum de estado cada turno (4 ticks). Verificado con dos procesos y
  mapa retail: TCP 2400 ticks (600 turnos) → hash idéntico
  `76dfe3105027fa8a`; UDP 600 ticks → hash idéntico `bdf9377e0487c36f`.
- **Descubrimiento LAN** por UDP broadcast (`:7778`) y **reconexión** por
  resync del log de comandos (el peer se reconstruye con fast-forward, sin
  transferir estado).
- **Cobertura**: `cargo test` → 0 failed;
  `cargo clippy --all-targets` → 0 avisos.

**Ingeniería inversa (`research/`):**
- Inventario: 724 archivos, 5.88 GB, SHA-256 (`original-manifest/`).
- Motor identificado: nativo Blizzard C/C++ x86+x86-64, CASC+MPQ, CEF solo
  en lanzador; `.text` cifrado en disco → análisis sobre imagen descifrada.
- Dos dumps descifrados verificados (menú 32b `.text` 6.8950, partida 64b
  6.7975), proyectos Ghidra (209+242 MB), 56 948 + 59 706 funciones.
- Mapa en `docs/architecture.md`: puerta `0x70e810`, WndProc `0x945050`,
  paso de simulación `0x969f0d`, handler nuke `0x5d3823` confirmado,
  4 handlers del parser CHK + TRIG decompilados, TARGET_ORDER falsado en
  imagen. H2 (mismo engine, doble binario) confirmada.
- E2E REA→Ghidra verificado; `binary_overview` MCP limitado en imágenes
  grandes (vía directa `analyzeHeadless` documentada).

**Motor C (eliminado 2026-10-28 tras migrar lo útil):** su save/load,
replay-hashes y red-`socketpair` ya están portados como diseño y tests en
`engine-rust`; evidencias en `research/docs/rea-evidence-c.md`, historial
en `research/re-c.bundle`. DB simplificada: nunca se usó.

## Lo que falta

- **P0**: persistencia de perfil y progreso de campaña en disco (guardado /
  replays / reproductor: hecho).
- **LAN**: núcleo, UDP, descubrimiento, lobby en `sc_view` y reconexión
  hechos y probados. Falta **la prueba física en 2 máquinas** (no
  ejecutable desde esta sesión; arnés en `scripts/lan_two_machines.sh`),
  lobby >2 jugadores en la UI y reconexión automática con backoff.
  **Battle.net/online: fuera de alcance.**
- **RE**: heap del mapa en memoria no capturado (requiere re-volcado con el
  juego en marcha); `0x8aee30` aparcado (2 timeouts); bomba `GetMessage`
  sin localizar (baja prioridad); manifiesto extraído→origen; matriz A–F.
- **Cosmético**: skins Carbot/20.º aniversario, creep píxel-exacto, WebM
  completo fuera de título/briefing.

## Uso rápido

```bash
cd engine-rust             # todo lo de cargo/make se ejecuta aquí
cargo test                # suite completa, sin instalación retail
cargo clippy --all-targets
cargo run --release --bin sc_view -- --install "/path/to/StarCraft"
SC_NO_AUDIO=1 cargo run --release --bin sc_view -- --shot /tmp/x.bmp --shot-screen pause

# LAN: PC 1 (host) y PC 2 (cliente), mismo mapa en ambas
cargo run --release --bin sc_lan -- --host --chk map.chk --vf4 platform.vf4 \
  --races terran,protoss --ticks 28800
cargo run --release --bin sc_lan -- --join 192.168.1.20:7777 \
  --chk map.chk --vf4 platform.vf4 --race protoss
# (--udp en ambos lados para usar el transporte UDP)

# Prueba de aceptación en dos máquinas (compara los LAN_RESULT):
scripts/lan_two_machines.sh host  192.168.1.20 terran  map.chk platform.vf4 tcp
scripts/lan_two_machines.sh join  192.168.1.20 protoss map.chk platform.vf4 tcp
```

En el juego: menú → tipo de partida → **L** abre el lobby LAN
(**S** escanear, **H** hospedar, **ENTER** unirse).

Teclas: F5 guardar · F6 guardar replay · SPACE pausa/replay · +/- velocidad ·
R reiniciar/reintentar · ESC menú · Enter cargar/reproducir · DEL borrar.

## Porcentajes honestos (denominador explícito)

- **Juego offline 1P**: reglas 40/40 + datos 20/20 + cliente 22/25 +
  meta-funciones ~5/15 (falta perfil) ≈ **85 %**.
- **Con LAN**: TCP y UDP, descubrimiento, lobby en cliente y reconexión
  hechos y verificados; falta la prueba física en 2 PCs → **≈ 90 % de LAN**.
- **Proceso RE (fases 0–16)**: Fases 0–5 cerradas con evidencia ≈ **30 %**.
