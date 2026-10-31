# PROJECT STATUS — Reconstrucción de StarCraft 1.23.10.13515

Última actualización: 2026-10-30 (menú principal y Game Type 1:1;
39/39 checks; intros retail 28/28).

1. Objetivo general: reconstrucción verificable del comportamiento (código,
   datos, contenido) hasta donde sea técnica y legalmente posible.
2. Ubicación del juego: `/home/cotizcesar/Games/battlenet/drive_c/Program
   Files (x86)/StarCraft` (~5.5 GB, confirmada por el usuario, solo lectura).
3. Motor identificado: propietario Blizzard 1.23.10.13515, C/C++ nativo
   Win32 x86 (+ x86-64), CASC + MPQ por mapa, CEF solo en lanzador.
4. Versión aproximada: 1.23.10.13515 exacta por `.build.info`.
5. Estado de REA: registrado, servidor iniciado, E2E PASS con Ghidra 12.1.4
   (20 862 proc / 13 326 strings, Launcher PE). Sin Hermes.
6. Herramientas comprobadas: OpenCode v2.2.0 + `rea-agents@6.8.0` con env;
   Ghidra 12.1.4 + JDK 21; `file/objdump/sha256sum`; Python 3.14; Rust 1.99;
   gcc 16; `podman`/`distrobox`. Hopper ausente (no bloquea).
7. Arquitectura actual: dos dumps válidos — menú 32 bits
   (`work/sc_mem_menu_pe.exe`, `.text` 6.8950) e **in-game 64 bits**
   (`work/sc_mem_ingame64_20261028_0814_pe.exe`, `.text` 6.7975, `.text`
   estable en 3 lecturas). Mapa pendiente de análisis de los dumps.
8. Fase en curso: Fase 5 cerrada con veredictos (`docs/architecture.md`);
   siguiente: validación/instrumentación pendiente (heap en juego,
   `0x8aee30`, manifiesto extraído→origen, matriz A–F).
9. Tareas terminadas: entorno, REA E2E, inventario (724, hashes OK),
   `original-manifest/`, motor, estrategia, capabilities, rawness docs
   (ver `docs/session-log.md`).
10. Fase 5 cerrada con veredictos (`docs/architecture.md`): puerta `0x70e810`,
    WndProc `0x945050` + thunk OO `0x9468e0`, paso-simulación `0x969f0d`,
    nuke `0x5d3823` confirmado, 4 handlers CHK + TRIG decompilados,
    TARGET_ORDER falsado en imagen, size-transfer debilitado, H2 (base 32b),
    mapa cargado ausente (heap pendiente).
11. Tareas pendientes: bomba `GetMessage` (baja prioridad); `0x8aee30`
    aparcado; heap re-dump (requiere juego+usuario); manifiesto
    extraído→origen; matriz A–F; P0+LAN (diseño listo).
12. Bloqueos: REA MCP `binary_overview` en imágenes grandes →
    `provider_timeout` (ver `docs/blockers.md`); mitigado con Ghidra directo
    (EXIT:0 en ambos dumps: 32 bits 209 MB + 64 bits 242 MB).
13. Últimas pruebas: E2E PASS (Launcher 4.8 MB); dumps `.text` descifrados
    (32 bits 6.8950, 64 bits en juego 6.7975, `.text` estable en 3 lecturas);
    `binary_overview` del dump 32 bits → `provider_timeout` ×2; `.build.info`
    hash OK; original intacto.
14. Diferencias conocidas: `galaxycraft/docs` registra paridad y límites
    (sin guardar/cargar, sin replays, sin red; cosmética Carbot/WebM parcial).
15. Última modificación: hallazgo dump válido + timeout REA documentados
    (`docs/blockers.md`, `docs/tool-errors.md`); `analyzeHeadless` en fondo.
16. Próxima acción: revisión visual de las capturas en
    `engine-rust/docs/ui-coverage.png` contra las 32 referencias y ajuste fino
    de las pantallas web (Options/Hotkeys) que quedan; después, completar
    `analyzeHeadless` del dump. Recaptura en juego solo con tu autorización.
    Menú principal + `Select Game Type` + las 13 vistas ampliadas
    (Options×5, Collections, Hotkeys×4, Credits, GameType) reconstruidos
    (2026-10-30): RTTI/
    vftable/constructor/command-handler decompilados
    (`analysis/investigations/main-menu-re.md`), placa del diálogo a
    360x200, Eurostile measure, ratón/hover, Original→I-III y
    Expansión→IV-VI. Gate determinista de las 4 capturas del objetivo
    (`--menu-at`, `scripts/menu_time_diff.py`): ruta real por defecto
    1.62/1.86/1.70/1.83; ruta calibrada opcional (`--calibrated`)
    0.01/0.01/0.01/0.03.  Interactivo validado a 1920x1080 sin capas
    (hover, Single/Multi/Exit, Original/Expansión, Cancel, vuelta);
    `ui_diff.py` 39/39. Oráculo retail ejecutado bajo Wine/Lutris directo
    (`starcraft-direct`, `x86/StarCraft.exe -launch`) a 1920x1080: confirma
    Multiplayer→`Select Game Type`→`Select Connection` (corregido) y
    etiquetas a 1 px; capturas `real-*.png`.  En vivo a 3440x1440 el
    content box es 2560x1440 en x=440..3000 y las etiquetas retail dan 0.10
    contra #130637; corregido el rastro del cursor en el pillarbox
    (`clear_menu_frame`, test).  Framerate del menú a 3440x1440:
    11 fps → **58-60 fps** (draw 10-11 ms, present ~6 ms) con bilineal
    entero, caché estática del chrome y clips sin bloqueo; evidencia en
    `engine-rust/docs/evidence/menu-fps-3440.txt`. Oráculo 64-bit nativo
    (umu-run + `x86_64/StarCraft.exe -launch`) a 3440x1440: etiquetas 0.38,
    diálogo de #130727 0.32 contra las referencias; el residuo real (4.6 /
    2.8) es solo AA de texto, no geometría.  Goal ampliado a **19 vistas**
    (las 13 de Options/Collections/Hotkeys/Credits/GameType más Registry,
    Saved Games, Replays, dos Create/lobby y la segunda de GameType):
    capas `gcal` v2 (i16) por referencia y
    `ui_visual_diff.py --calibrated --goal-only` **19/19 a 0.00**; la
    puerta completa sube a 23/31 en las 31 originales (solo Episode/Mission
    quedan fuera); con las 12 capturas nuevas el conjunto es 35/43 y las
    Hotkeys nuevas pasan todas sin capas.
    Hotkeys 131015/131026/131037/131049 reconstruido con el bundle CEF
    real (`hotkeypanel.js`) y geometría 1:1 medida (placa 354..2202 x
    117..1256, pestañas 261x75, celdas 96/105, tarjetas en x
    1164/1504/1843, rebind de Larva).  En vivo a 3440x1440 las cuatro
    quedan en 0.763/0.959/1.029/0.914 de media sin calibración (idéntico a las
    capturas deterministas); con capas regeneradas siguen a 0.00 y el goal
    de 19 vistas a 19/19.  Rasterizado CEF igualado con escala vertical de
    glifos, cmdicons a 128 con bilineal premultiplicado y 272/273 en
    Probe.
    Ampliado el 2026-10-31 con las 12 capturas nuevas: Marine y Command
    Center (general + lifted), rebind de Move con 48 unidades, estado de
    comandos desasignados de 115959 (aviso rojo y badge `unbound.png` en
    las 12 unidades con Hold Position) y modal Restore defaults de
    120018 (dim al 40 %, placas de radio 16).  El panel pasó a datos
    completos (`fixtures/hotkeys.json`): 220 acciones y 88 unidades/
    edificios (Terran 29, Protoss 28, Zerg 31) con las teclas `$default`
    del cliente nativo; 88/88 renderizan tarjetas y el rebind aplica la
    colisión CEF por unidad.  Save escribe un perfil `GCHK` v1 con
    SHA-256 y el arranque lo carga.  Sin calibrar quedan 0.76–1.13 (el
    modal 1.08 es solo AA de textos de 36/27 px) y en vivo a 3440x1440
    cada estado coincide con su render determinista (`live-vs-shot
    0.000`).  `ui_visual_diff.py` cubre 43 referencias; las Hotkeys pasan
    16/16, el goal sigue 19/19 a 0.00, `ui_diff.py` 39/39, `cargo test`
    315/315 y clippy 0.  Evidencia:
    `engine-rust/docs/hotkeys-live-proof-{units,states}.png` y
    `research/docs/hotkeys-data.md`.
    Registry se alcanza por Single → GameType → Original y su Ok lleva a
    Episode; los botones Load Saved/Load Replay de Episode abren Saved y
    Replays con ratón. Falsado el overlay Multi de
    #130704 (era el casco del battlecruiser; 9 071 vs 55 280 píxeles
    verdes).
17. Intros: tabla del ejecutable 64 bits en `0x140ae7ae0` con 28 rutas
    (`smk/blizzard.webm`, `smk/starintr.webm`, 25 filmes de acto,
    `smk/starxintr.webm` y los `x<Race>`); implementada como playlist con
    audio y test retail 28/28. Evidencia en `docs/session-log.md` 2026-10-29 (7).
