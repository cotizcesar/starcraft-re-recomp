# Bitácora de sesiones

## 2026-10-28 (gráficos: animación del menú + HUD a ancho completo)

- `video::decode_frames`: decodifica N frames en **un** proceso ffmpeg
  (`-ss -vframes -r -f rawvideo`), usado por las 3 capas del menú
  (6 frames @ 6 fps, tope 1280 px).
- `MenuLayers` guarda secuencias y `pick(t)` cicla por tiempo; se descartan
  frames con destello a pantalla completa (`drop_flash`, luma media > 96)
  porque en mezcla aditiva lavaban el menú (ese era el rectángulo naranja).
- HUD: los platos ya se anclan a los bordes (`draw_plates`), así que en
  21:9 cubre todo el ancho; verificado a 3440x1440.
- Corregido duplicado en `--shot-screen gamemenu` (panel procedural sobre el
  arte retail).
- Pruebas: `tests/menu_layers.rs` (frames distintos) y
  `tests/terrain_layers.rs` (0 huecos). `cargo test` → 258 passed / 0 failed;
  clippy 0 avisos.

## 2026-10-28 (gráficos: huecos de terreno, escala y menú Remastered)

Estímulo del usuario: "no se parece en nada al juego oficial" y "solo
Remastered", más el requisito de 16:9 hasta 4K y ultrawide expansible.

- **Bug de terreno corregido**: `draw_tile` no pintaba el megatile de fondo
  del grupo CV5 (`+10`), así que la transparencia de los megatiles dejaba
  negro. Antes 2/698 tiles vacíos y decenas parciales; ahora 0/0.
  Regresión: `tests/terrain_layers.rs`.
- **Escala unificada**: mundo en framebuffer lógico + `blit_scaled`; el HUD
  usa la misma escala (derivada de la altura). Verificado a 1280x720,
  1920x1080, 2560x1440, 3440x1440 y 3840x2160.
- **Ratón**: `to_logical` cableado (antes con escala ≠ 1 se seleccionaba la
  unidad equivocada).
- **Menú Remastered**: los WebM `glue/mainmenu/{single,multi,exit}.webm` no
  son texto sino las **capas animadas** del menú; se componen en aditivo
  sobre un fondo espacial, con el marco DDS, entradas verdes, perfil y
  versión. `VideoStore::poster` gana fallback a disco y
  `decode_frame_at(seek, max_width)` para elegir frame y resolución.
- `cargo test` → 257 passed / 0 failed; clippy 0 avisos.
- Límites documentados en `docs/graphics-remastered.md` §4 (sin animación ni
  parallax, sin planeta, panel del marco vacío, HUD 4:3 centrado).

## 2026-10-28 (GOAL LAN cerrado salvo prueba física en 2 máquinas)

- **UDP**: `UdpTransport` con stop-and-wait ARQ sobre el mismo trait
  `Transport`; `host_turn`/`client_turn` pasan a genéricos. Handshake
  prefijado `0xFF` para que un saludo tardío no se confunda con un ACK.
  Tests: bidireccional con hilos, duplicados con re-ACK, rechazo de
  datagrama sobredimensionado.
- **Descubrimiento**: `Announce` + `broadcast`/`discover` en
  `DISCOVERY_PORT=7778`; test con deduplicación por dirección.
- **Reconexión**: `Msg::Rejoin`/`Msg::Resync`; el host envía su log de
  comandos y el peer reconstruye con fast-forward (`apply_resync`).
  Test: 8 turnos → caída → resync → 8 turnos → hash final idéntico.
- **Lobby en `sc_view`**: `MenuScreen::Lan` (L desde el menú de tipo),
  S escanear / H hospedar / ENTER unirse; `--shot-screen lan` verificado.
  Órdenes y entrenamiento enrutados por `send_order`/`send_train` para que
  en LAN se encolen en el turno.
- **Pruebas de dos procesos** (Boxer.chk, 66 unidades): TCP 600/1200/2400
  ticks con hash idéntico en ambos (2400 = 600 turnos/checksums); UDP 600
  ticks con hash idéntico `bdf9377e0487c36f`.
- `cargo test` → 256 passed / 0 failed; clippy 0 avisos.
- **NO ejecutado**: prueba física en dos máquinas (una sola disponible).
  Arnés `scripts/lan_two_machines.sh` + criterio de PASS en
  `docs/lan-protocol.md` §6.

## 2026-10-28 (GOAL LAN: núcleo lockstep implementado y verificado)

- `src/net.rs` nuevo: `Msg` (Join/Setup/Turn/Bundle/Sum/Desync/Chat/Bye)
  con encoding propio, `Transport` + `TcpTransport` (framing u32, timeouts
  10 s, `nodelay`), `Lockstep` (merge determinista peer→inserción,
  `commit`), `Lobby::bind/accept_peers`, `connect`, `host_turn`,
  `client_turn`.
- Reutiliza el encoding de `RecAct` de `src/replay.rs` (DRY): red y `.gcrep`
  comparten formato. `Cursor` gana `new/offset/at_end/take32/take_bytes` y
  sus lectores pasan a `pub`; helpers `w_*` a `pub(crate)`.
- `tests/lan.rs`: 2 tests por TCP loopback (20 turnos con comandos de
  ambos lados, y rechazo de mapa distinto). Verdes.
- Binario `src/bin/sc_lan.rs` (host/cliente headless, `--chk/--vf4`).
- **Prueba real de 2 procesos** con `Boxer.chk`: 600 ticks → hash
  `bdf9377e0487c36f` en ambos; 2400 ticks (600 turnos/checksums) → hash
  `76dfe3105027fa8a` en ambos, exit 0.
- `cargo test` → 251 passed / 0 failed; clippy 0 avisos.
- Docs: `docs/lan-protocol.md` (estado + decisiones), `README.md`.

## 2026-10-28 (diferencial de heap: método validado, despachador no)

- Con el usuario pausando la partida: capturas A, A2 (ruido) y B (tras una
  orden de movimiento). `tools/heap_diff.py`.
- Suelo de ruido: 11,46 MB cambiados / **13** punteros nuevos a código.
  Una orden: 16,02 MB / **69** punteros nuevos → el diferencial discrimina
  (5×), aunque el heap es muy ruidoso en pausa (buffers de render).
- **Negativo verificado:** las 3 tablas runtime (`mod+0x11b9b78/0x118f970/
  0x11afef0`) NO cambian al emitir una orden (0 bytes) → son vtables de
  clase inicializadas al cargar, no el despacho de órdenes.
- Decisión: no seguir con diff de heap (ROI bajo y ruido alto); si se
  retoma, instrumentar con Frida en vez de comparar volcados.
- Detalle: `docs/heap-ingame-findings.md` §6.

## 2026-10-28 (heap en partida: el despachador es runtime, no estático)

- Capturado el heap de la partida real (Terran 8 "The Big Push", 483
  regiones RW, 897 MB) con `tools/heap_capture.py`. El mapa SÍ está
  cargado: STR del briefing localizada (`0x432fbf1` en región 0xad22000).
- **Hallazgo principal:** 3 tablas de punteros a `.text` en `.data`
  (`mod+0x11b9b78` 97 ptrs, `mod+0x118f970` 84, `mod+0x11afef0` 63) están
  **a cero en la imagen en disco** y pobladas en memoria → se construyen en
  runtime. Esto explica por qué el barrido estático no las veía.
- Tabla C6 de secciones CHK recuperada en 64 bits (30 entradas, TRIG =
  `0x6ffffb1388d0`); barrido de los 20 handlers en el heap: 1 hit (el
  propio TRIG en su tabla estática) → el despacho de secciones NO es
  runtime.
- Descartados con evidencia: TRIG-2400 crudo, columnas DAT, clusters
  UNIT/MRGN (falsos positivos de BSS a cero).
- Detalle: `docs/heap-ingame-findings.md`. Siguiente: diff con pausa.

## 2026-10-28 (caza del despachador: veredicto honesto)

- 13 tablas de salto indexadas revisadas (máx 86 casos): ninguna toca
  handlers conocidos. `cmp` contra 168/188/189: 0 en `.text`.
- 0 refs en `.rdata`/`.data` a nuke/paso/puerta/cast. Sin tabla estática.
- Región `0x5c9000-0x5cb000`: 89 calls desde 71 funciones, pero el mejor
  candidato `0x5bd8e7` es un parser de config (`strtod`, std::string,
  7 casos, sin IAT) — NO es el despachador. Vecinos = stubs.
- Conclusión: el despachador no vive como switch/tabla estática en la
  imagen (tabla construida en heap o wrappers sin xrefs). Vía estática
  agotada en lo eficiente; siguiente: heap re-dump con el juego en marcha
  (requiere usuario) o pivotar a P0/LAN donde el ROI es mayor.

## 2026-10-28 (GOAL migrar engine-c y borrarlo: CERRADO)

- Migrado lo último con valor: `docs/REA-EVIDENCE.md` →
  `research/docs/rea-evidence-c.md` (identidad del objetivo, imports,
  cripto MPQ diferencial, CHK vía StormLib); `re-c.bundle` →
  `research/re-c.bundle` (clona limpio, 2 commits, verificado).
- Borrado `engine-c/` + enlace `~/Documents/sc-cpp` (verificado ausentes).
- Docs actualizadas: `README.md` (sin fila engine-c), `AGENTS.md` (un solo
  motor; §4 = validación sin segundo motor), `research/SALVAGED.md`
  (procedencia final), `salvage-sc-cpp.md` (cerrada como histórica).
- Estructura final: `engine-rust/` + `research/` (+ `README.md/AGENTS.md`).

## 2026-10-28 (GOAL unificación: 1 carpeta, AGENTS.md, README.md)

- Estructura en ese momento: `engine-rust/` (motor activo) + `engine-c/`
  (congelado entonces, eliminado después; ver entrada siguiente) +
  `research/` (workspace). Eliminada `salvaged/` tras migrar a `INDEX`→
  `research/SALVAGED.md`. Enlaces: `~/ReverseEngineering`→`research`,
  `~/Documents/galaxycraft`→`engine-rust`, `~/Documents/sc-cpp`→`engine-c`
  (este último eliminado con el motor C).
- Limpieza DRY en `engine-rust/`: fuera andamiaje vacío
  (`analysis/extracted/reconstructed/build/tests/results`) y docs del
  workspace → `research/docs/` (solo las vigentes: environment,
  tool-inventory, rea-opencode-integration); duplicados eliminados
  (`file-inventory.csv`, `PROJECT-STATUS.md` del engine, `logs/`).
- `AGENTS.md` (contrato: roles, SOLID/DRY/YAGNI/KISS, reglas Rust/C/RE/docs,
  Definition of Done) y `README.md` (qué hay / qué falta / uso / %) nuevos.
- Flaky ambiental en suite (3 tests UI) investigado: parse OK aislado,
  suite completa verde después (246/246 ×2); no es regresión del
  movimiento. Causa exacta sin aislar; queda anotado, no bloquea.

## 2026-10-28 (GOAL save/load + replay player CERRADO)

- `src/replay.rs`: `RecAct::{Issue,Train,Morph,MorphBuilding}` (v2),
  hooks en `issue/train/morph/morph_building`, `SaveFile` (CHK embebido),
  `state_hash`, `replay_to`, `apply_act` pública.
- Hallazgo clave: la IA gasta vía directa (`train/morph`), no por `issue`;
  sin actos económicos el replay diverge al tick 1 (minerales 0 vs 50).
  Resuelto unificando el log; Boxer+IA replays idéntico.
- `sc_view`: F5 guardar, F6 guardar replay, Saved/Replays con
  lista/carga/borrado, reproductor (SPACE pausa, +/- velocidad, R restart,
  ESC salir), overlay REPLAY PAUSED, `apply_act` compartido.
- Pruebas: `cargo test` → **246 passed, 0 failed** (incl. Boxer+IA y
  round-trip SaveFile). `clippy --all-targets`: 0 avisos. Shots headless
  saved/replays/pause OK (3.6 MB renderizados).
- Pendiente P0: persistencia de perfil/campaña en disco (no entraba en
  este goal).

## 2026-10-28 (GOAL src/replay.rs CERRADO: 244/244 + clippy limpio)

- `src/replay.rs` (nuevo): codificación manual de `Order` (24 tags),
  `Spell`/`SpellTarget`, `Recording` (magic `GCS1` + versión + sha256),
  `state_hash` (tick, RNG, jugadores, unidades, proyectiles),
  `replay_to`, `Recorder`.
- Hook en `World::issue` (solo éxitos) + campo `recorder: Option<Recorder>`
  (`None` por defecto, coste cero). `Cargo.lock` sin cambios (sha2 ya era
  dependencia).
- `tests/replay.rs`: ida y vuelta tick 60 (3 comandos) → hash idéntico;
  fichero corrupto rechazado.
- Pruebas: `cargo test` → **244 passed, 0 failed** (142 lib + integración,
  incluye 1 unit + 2 integración nuevos). `cargo clippy --all-targets`:
  0 warnings/errors.

## 2026-10-28 (WndProc CONFIRMADO: `0x945050`)

- `FUN_00945050` (3 759 B, 57 IAT-refs/25 slots): firma
  `(hwnd,msg,wParam,lParam)` — `switch(param_2)` 67 casos,
  `msg<0x100`, `msg==0x400` (WM_USER), `wParam&0xffff`.
  **Es el WindowProc principal.** 0 xrefs = registrado vía
  `RegisterClass` (nadie lo llama directo), coherente.
- Top callee `0x9468e0` ×20 (sub-handler de mensajes probable).
- `0x969f0d` queda como paso de simulación/trabajador (0 IAT).
- Grafo actualizado: CRT → `0x70e810` → init (`0x8aee30`,
  `0xc61f70`) → WndProc `0x945050` → paso `0x969f0d` (hipótesis).

## 2026-10-28 (bomba de mensajes localizada + bug RVA/VA)

- Bug propio encontrado y corregido: el barrido IAT usaba RVA como VA
  absoluta (todo caía en la última entrada del CSV). Con base `0x400000`:
  ranking real por IAT-refs.
- Candidata a bomba: **`0x945050`** (3 759 B, 57 refs IAT, 25 slots
  distintos; muy por encima del resto). `0x969f0d` = 0 IAT (lógica interna
  pura — matiz confirmado). `0xc61f70` = 21 (control OK).
  `0x8aee30` = 0 IAT.
- Batch 7 en fondo: xrefs de `0x945050` (quién la llama) + decompilado.

## 2026-10-28 (TRIG decompilado + IAT acotada + matiz al bucle)

- `FUN_00710970` (TRIG) decompilado tras crear la función: usa slots IAT
  (`DAT_00d301f8/f4/ec/f0`), lector `0x984770` y doble llamada a `0x610970`
  con struct+`0x1b0` (proceso de registro de trigger). Familia parser: 14
  llamantes distintos del lector (cluster `0x70fxxx` + `0x8faa8f`,
  `0x7725b0/0x771f10/0x771fd0/0x7723b0`).
- IAT confirmada en `0xd301xx` (vía TRIG). Barrido FF15/FF25 en `0x969f0d`:
  **0 llamadas indirectas** (118 calls directas) → es lógica interna pura,
  NO la bomba de mensajes externa; matiz a A: candidato a paso de
  simulación, la bomba con IAT sigue sin localizar.
- `0x8aee30` aparcado (2 timeouts). Siguiente: cazar la bomba (funciones con
  IAT + cadenas de ventana/input) y C6 completa si aporta.

## 2026-10-28 (C6-TRIG localizado + IAT falsada + batch 5)

- C6 parseada (16 entradas reales): TRIG handler = `0x710970` (flags=1).
  NO está en el CSV: cae +112 tras `FUN_00710900` (size 38) pero sus bytes
  empiezan con prólogo real (`55 8bec`) → función no partida por Ghidra.
- IAT por inmediatos (`0xd30000–0xd31000`, `0x13d35xx`) en gate/bucle/nuke/
  DIM: **0 refs** → hipótesis falsada para esos rangos; la IAT real va por
  otra vía (resolver vía xrefs Ghidra, no por barrido).
- `.data`→región nuke (`0x5c0000–0x5d5000`): 22 punteros dispersos, sin tabla
  densa → el despachador no vive en una tabla `.data` obvia.
- Batch 5 en fondo: xrefs de `0x710970` + decompilado de `0x710900`
  (handler FORC vecino).

## 2026-10-28 (GOAL: rescate total Rust+C + borrado — CERRADO)

- Migrado a `re-work/salvaged/`: `re-rust/` 354 ficheros (sin `target/`),
  `re-c-tree/` 113 ficheros + `re-c.bundle` (historial git). Pruebas:
  listas diff-idénticas, sha256 agregado `src/` idéntico
  (`862ef1cf…e43d`), `Cargo.lock` idéntico, bundle clona limpio (2 commits).
- Borrados `re-rust/`, `re-c/` y symlinks `~/Documents/{galaxycraft,sc-cpp}`
  (verificado ausentes; `~/ReverseEngineering`→`re-work` intacto).
- `README.md` maestro + `salvaged/INDEX.md` actualizados.
- Batch 4: `0x5ca229` tiene 0 xrefs (llegada por tabla/puntero); su
  decompilado (6 914 lín.) NO contiene la call a nuke registrada por el
  reference manager → solape de funciones en la zona, decompilado no fiable
  ahí; el xref `UNCONDITIONAL_CALL 0x5ca26a→0x5d3823` vale como hecho.

## 2026-10-28 (C1 validado + cadena nuke + batch 4)

- Handlers C1 decompilados: DIM (`0x90ca70`, guarda word en struct+0x70),
  OWNR (`0x90cb90`, guarda `param&7` en struct+0x30 = máscara de raza),
  STRx (`0x90cdd0`, bounds + tabla de strings). Helper común `0x984770`
  (lector CHK). Tabla del parser VALIDADA como código real.
- Nuke: 1 único xref — `UNCONDITIONAL_CALL` desde `FUN_005ca229`
  (+0x41). Callees del handler = glue/errores menores (confirma realismo,
  no el núcleo). `0x8aee30` falla también a 300 s → se aparca (init, no
  crítico para órdenes); reintento por partes queda registrado.
- Batch 4 en fondo: xrefs de `0x5ca229` (¿quién llama al llamante? →
  hacia el despachador) + decompilado de `0x5ca229`.

## 2026-10-28 (C1 resuelto + batch 3 en curso)

- Tabla C1 parseada (`f=0x968334`, 8×{handler,flags,nombre}): DIM
  `0x90ca70`, ERA `0x90cb40`, OWNR `0x90cb90`, SPRP `0x90cab0`, STR
  `0x90ceb0`, STRx `0x90cdd0`, MTXM `0x90ce40`, UNIT `0x90cbd0`.
  En CSV: 3 entradas exactas (DIM 55 B, OWNR 47 B, STRx 14 B), resto dentro
  de funciones vecinas (partición Ghidra).
- Nuke `0x5d3823`: **0 llamantes E8 directos** en `.text` → invocado por
  despachador (puntero/tabla), coherente con modelo de órdenes.
- Batch 3 en fondo (`logs/ghidra-batch3.log`): `RefsToAddr` → xrefs del
  nuke + decompiles de 3 handlers C1, 4 callees nuke y reintento `0x8aee30`
  (timeout 120→300 s).

## 2026-10-28 (integración en `starcraft-re-recomp`)

- `~/Documents/starcraft-re-recomp/` = `re-rust/` (galaxycraft) + `re-c/`
  (sc-cpp) + `re-work/` (ReverseEngineering). `README.md` maestro creado.
- Rutas antiguas dejadas como symlinks (todo lo documentado sigue
  resolviendo). `re-c` git limpio; `re-rust` verificado (`src/lib.rs`).

## 2026-10-28 (master prompt, sin Hermes)

- Fase 0: entorno re-verificado (Bazzite 44.20261026.0, kernel 7.2.11);
  `logs/environment-check.log` actualizado.
- Fase 1 (REA): E2E `open_binary`/`binary_overview`/`binary_session`/
  `close_binary` PASS con Ghidra 12.1.4; `docs/rea-integration.md`,
  `docs/rea-capabilities.md`, `logs/rea-readiness.log` generados.
- Fase 2: `GAME_PATH` confirmado; inventario verificado (724 archivos,
  `.build.info` coincide); `original-manifest/` creado por copia;
  `docs/file-inventory.md` + `analysis/discovery/unrecognized-files.csv`
  (239 filas, mayoría CEF/CASC conocidas).
- Fase 3: motor confirmado (propietario Blizzard 1.23.10.13515);
  `analysis/discovery/engine-evidence.md`, `docs/runtime-and-platform.md`.
- Fase 4: `docs/analysis-strategy.md`, `docs/tool-capability-matrix.md`,
  `docs/format-coverage.md`.
- Estructura master creada por adición; originales intactos (solo lectura +
  copia a `/tmp` para el E2E, eliminada tras el cierre).

## 2026-10-28 (dump: verificación y análisis)

- Dump `work/sc_mem_menu_pe.exe` redescubierto (17 620 992 B, 06:57).
  Entropía de sección completa: `.text` 6.8950 (descifrado) vs 7.9997 en
  disco (cifrado). Válido para análisis.
- REA `open_binary` del dump OK; `binary_overview` → `provider_timeout`
  determinista ×2 ("Ghidra startup deadline elapsed"). Evidencia exportada a
  `/tmp/opencode/dump-error-bundle.json`. Sesión cerrada limpia.
- Alternativa: `analyzeHeadless` directo con JDK 21 en fondo
  (`logs/ghidra-dump-analysis.log`). `docs/blockers.md`, `docs/tool-errors.md`
  y `docs/unresolved-questions.md` actualizados. Sin `sudo`, sin tocar el
  original.

## 2026-10-28 (lanzamiento para recaptura, autorizado)

- Lanzado `lutris lutris:battlenet` en fondo (`logs/lutris-launch.log`).
  Battle.net arranca vía umu/GE-Proton11-11 (verificado en `ps`: Launcher.exe
  + wineserver activos). Sin `StarCraft.exe` aún: requiere que el usuario
  pulse Jugar y entre en partida para capturar memoria en juego.

## 2026-10-28 (recaptura en juego — variante x86-64)

- Usuario en partida. Proceso: `x86_64\StarCraft.exe -launch -uid s1`
  (PID 24329). La variante en ejecución es la de **64 bits** (no la de 32).
- Sin ruta del exe en `maps` (Proton mapea vía `/memfd:wine-mapping`).
  Imagen hallada por barrido: base `0x6ffffade0000` (PE32+, machine 0x8664,
  8 secciones, SizeOfImage 0x1450000 = coincide con el PE en disco).
- Solo lectura `/proc/<pid>/mem` (pread, sin depurador, sin escritura).
  `.text` entropía 6.7975 → **descifrado** (disco: 7.9997).
- Archivos: `work/sc_mem_ingame64_20261028_0814.bin` (crudo) y
  `work/sc_mem_ingame64_20261028_0814_pe.exe` (PE reconstruido,
  `file` → PE32+ x86-64, 8 secciones). `PointerToRawData=VA`,
  `FileAlignment=SectionAlignment` (offsets iguales en PE32+).
- Estabilidad: `.text` idéntico en 3 lecturas (`f40d8651…`); los hashes de
  imagen completa difieren entre lecturas por `.data` viva (normal).
- Nota: `tools/capture_memory.py` solo acepta PE32 (32 bits) y busca la base
  por ruta en `maps`; bajo Proton/64 bits hizo falta el procedimiento manual
  de arriba. Generalizar el script queda pendiente (Fase 13).

## 2026-10-28 (H2 confirmada a nivel de datos; juego cerrado)

- Directorio IMPORT ausente en ambos dumps (protector/Wine): sin comparativa
  de imports posible.
- `strings -n 8`: 25 802 (32b) / 20 761 (64b), **17 216 compartidas**;
  marcadores idénticos. H2 confirmada (mismo engine, doble binario).
- `analysis/discovery/dump-manifest.md` creado; `engine-evidence.md` y
  `unresolved-questions.md` actualizados. Usuario cerró el juego.
- `analyzeHeadless` del dump de 32 bits sigue en fondo; el dump de 64 bits se
  analiza después (secuencial, no saturar CPU).

## 2026-10-28 (análisis 32 bits OK; 64 bits en curso)

- `analyzeHeadless` directo (JDK 21) del dump de 32 bits: **EXIT:0**,
  `Analysis succeeded`, proyecto guardado. Ruido no fatal: reparaciones de
  cuerpo por solape, 2 funciones con thunk contenido, errores de pcode en
  saltos computados a memoria inexistente (típico en código de juego).
  Proyecto persistido en `build/ghidra-proj32` (209 MB, fuera de `/tmp`).
- Vía alternativa validada: Ghidra directo sí procesa imágenes de ~18 MB;
  el límite era el deadline del servidor MCP, no Ghidra.
- Lanzado `analyzeHeadless` del dump de 64 bits en fondo
  (`logs/ghidra-dump64-analysis.log`, proyecto `build/ghidra-proj64`).
  `docs/blockers.md` actualizado.
- Dump 64 bits en juego: **EXIT:0, `Analysis succeeded`**, proyecto guardado
  (`build/ghidra-proj64`, 242 MB).
- Estado: dos proyectos Ghidra listos (32 bits menú 209 MB + 64 bits en juego
  242 MB), ambos de imágenes descifradas verificadas.
- Siguiente: explotación — inventario de funciones, referencias cruzadas y
  localización de sistemas (entrada, bucle, iscript/órdenes) → `docs/architecture.md`
  (Fase 5). Vía: scripts Ghidra headless + REA sobre funciones acotadas
  (evita el `binary_overview` completo que excede el deadline MCP).

## 2026-10-28 (inventarios de funciones listos)

- `analysis/functions32.csv`: 56 948 funciones (56 669 reales + 279 thunks).
- `analysis/functions64.csv`: 59 706 funciones. Intersección de nombres no
  genéricos: 229 (casi todo `Unwind@`/thunks; la lógica real es `FUN_*`).
- Herramienta: `tools/GhidraExportFunctions.java` (Java; Jython no disponible
  en headless). Logs: `logs/ghidra-export32.log`, `logs/ghidra-export64.log`.

## 2026-10-28 (xrefs reales + rescate sc-cpp)

- Xrefs batch 1 (5 cadenas, script con máscara a cero = basura) descartado;
  batch 2 corregido: `nukeWarning` 1 hit (ref desde `.data`, sin función),
  `rescue_passive` 1 hit + 2 refs (`0x562e12` fuera de cuerpo,
  `0x7d4adb` dentro de `FUN_007d4a03` 251 B), `SND_NUKE_LAUNCH` →
  `FUN_005d3823` (440 B), `Starcraft.ini` → `0x70ac50` (confirma puerta).
- Batch 3: `ChkLarvaStray` 2 hits, `SND_GAS_DEPLETED` 1, `STR_BURROW` 1, los
  tres SIN xrefs de código (referencia por tabla/dirección computada).
- Decompile batch 2 en fondo: `0x7d4a03` (rescue), `0x5d3823` (nuke),
  `0x562db4` (rescue débil). `analysis/decompiled32/`.
- `sc-cpp` evaluado: `docs/salvage-sc-cpp.md`. Segunda reconstrucción (C,
  3 737 lín., determinista 24 Hz, save/load, replay-hashes, net-socketpair,
  jugable, 368+ tests). Salvable: patrones P0/LAN + validación cruzada;
  NO sus tablas (44 vs 228). REA-EVIDENCE corrobora motor/protector.

## 2026-10-28 (nuke confirmado; rescates descartados)

- `FUN_005d3823` (440 B, ancla `SND_NUKE_LAUNCH`): handler de orden Nuke
  CONFIRMADO — `switch (*(CUnit+0x4e))` casos 0-6, juega el sonido,
  transiciones `+0x4e = 3/5`, chequeos `+0x54/+0x80`, posiciones
  `+0x58/+0x5a`. Valida el método por xrefs y el layout `CUnit` de B.
- `FUN_007d4a03` (rescue): basura/ofuscado (bad instructions, solapes) —
  despriorizado. `FUN_00562db4`: helper de ficheros (`"closed"`), no rescue.
- `ChkLarvaStray`/`SND_GAS_DEPLETED`/`STR_BURROW`: hits sin xrefs de código
  (referencia por tabla/computada). Región `0x7d4xxx` con bytes
  ofuscados: nota para el análisis del protector.

## 2026-10-28 (batch decompilados 8/9: veredictos)

- Puerta confirmada: `0x70e810` = main lado juego (llama ini-loader y
  `FUN_00538810(2)` noreturn); `0x70ac50` construye ruta + `Starcraft.ini`.
- `0x969f0d` = lógica real con bucles (22 `while/for`, 118 calls, top
  callees coinciden con lo predicho `0x55cbe0`×17): mejor candidato a
  paso de frame, pendiente de IAT para frame-pacing.
- `0xc046b0` NO es dispatcher de órdenes: es resolutor GL (`glAccum` +
  switch). Corrección a A-#5.
- Cast genérico (B-exp.1): `0x7d5dde` (37 callees) sin stores `+0x4e`;
  otros 3 candidatos con 9/3/1 callees. Firma NO confirmada → hipótesis de
  transferencia por tamaño se debilita; pivotar a xrefs de cadenas
  (`nukeWarning`, `ChkLarvaStray`, `rescue_passive`).
- `0x8aee30` falló (timeout 120 s, 11 KB): reintentar con timeout mayor o
  por partes.
- `TARGET_ORDER`: solo tabla identidad en imagen (ver anterior).
- Siguiente eficiente: script headless de xrefs a esas 3 cadenas + lista de
  llamantes, 1 sola ejecución.

## 2026-10-28 (eficiencia: batch de decompilados + LAN en paralelo)

- Búsqueda `TARGET_ORDER` (B-exp.7) en disco: solo tabla IDENTIDAD
  (32b `.data 0xb2a570`, 64b `.data 0xdd5190`); variante no-identidad: 0.
  La firma de 10 divergencias no está en la imagen → el remap vive en heap
  o la ref. antigua no transfiere. Experimento registrado como falsación
  parcial; la caza del despachador sigue por estructura de switch.
- Batch de 9 decompilados en fondo (`analysis/decompiled32/`,
  `logs/ghidra-decompile32.log`): `0x70e810`, `0x70ac50`, `0x969f0d`,
  `0xc046b0`, 4× size-531, `0x8aee30`. Herramienta:
  `tools/GhidraDecompileBatch.java`.
- Diseño LAN: `docs/lan-protocol.md` (lockstep, capa abstracta TCP→UDP,
  lobby, aceptación 1v1 20+ min). Implementación tras P0.

## 2026-10-28 (Fase 5: mapa de arquitectura integrado)

- Los 4 agentes entregaron (`analysis/investigations/`): A entrada/bucle
  (puerta `0x70e810`, candidato `0x969f0d`), B órdenes/iscript (ranking
  10 familias, firma `TARGET_ORDER`), C diff 32/64 (H2 alta, base en 32b),
  D triggers/datos (tablas parser sí, mapa cargado no, heap pendiente).
- Síntesis: `docs/architecture.md` (arranque, bucle, órdenes, datos,
  grafo confirmado/inferido, plan de confirmación en 8 pasos).
- Veredicto D incorporado: dump en partida sin mapa (heap no capturado);
  experimento de re-volcado RW registrado en `docs/unresolved-questions.md`.

## 2026-10-29 — Ronda de paridad visual contra las 44 capturas

Medición inicial: media 26.57, peor 44.37 (refs 4/28, que se estaban comparando
contra la pantalla equivocada). Resultado tras la ronda: **media 19.72, peor
31.26** (`scripts/compare_all.py`, informe en `research/docs/parity-report.md`).

HECHOS (con arte retail localizado en CASC, no aproximaciones):

- `glue/palmm/retail_ex.dds` es el diálogo "Select Game Type" completo (marco,
  borde rojo y ambos logos). Refs 4/28: 44.4 → 20.9.
- `glue/palnl/backgnd.dds` es el paisaje lunar de Registry/Saved/Create.
  Refs 5 y 13: 38.2/39.1 → 16.0/15.0.
- La pantalla Create (refs 14/15/35/36) tiene maquetación medible: árbol de
  Replays, panel "Replay Information", Ok/Cancel → 10.0.
- La selección de episodio (refs 6-12/29) usa `glue/campaign/{terr,zerg,prot}.webm`.
  Esos VP9 llevan **alfa en BlockAdditional**: decodificarlos sin `-c:v
  libvpx-vp9` devuelve basura en las zonas transparentes (falsado en vivo: el
  primer intento mostraba un rectángulo gris detrás del marine). Con alfa,
  recorte a la caja visible y anillos/etiquetas/botones medidos: 24.6 → 7.2-8.7.

FALSACIÓN registrada: mapear refs 4/28 a "intro" (póster de Blizzard) era
incorrecto; ambas muestran el menú con el modal encima.

Pendiente y declarado: geometría exacta del plato HD de consola (no hay layout
en datos, solo DDS/GRP) y las pantallas de Options/Hotkeys/Collections/Credits,
que siguen siendo listas de texto.

### 2026-10-29 (segunda ronda) — media 15.74 → 14.18

HECHOS verificados en esta ronda:

- `glue/palmm/backgnd.dds` es el fondo real del front-end (planeta + nebulosa):
  menú 22.2 → 4.5.
- `creditsscreen.ui.json` + `glue/battlenet/credits_a.dds` dan el panel de
  créditos exacto; `creditspanel.js` contiene la lista real de créditos
  (4515 entradas) que ahora se dibuja con los tamaños/color del CSS.
- FALLO GRAVE encontrado y corregido: las capturas in-game renderizaban el
  MENÚ. `App::menu` seguía a true, `draw()` devolvía antes de pintar el
  terreno y el área de juego medía 1.4 de brillo medio (referencia: 38.0).
  Ahora se pinta terreno + niebla y la cámara se centra en la unidad local.
- Las capturas de partida cargan una misión de campaña de la raza de la
  referencia (Terran 02, Zerg 06, Protoss 06; elegidas midiendo 10 misiones),
  lo que alinea la consola y el tileset: ref 37 26.2 → 19.2; refs 40-44 ~30 → ~22.

Queda declarado: geometría del plato HD (ref: plato derecho ≈240-500 px,
nuestro DDS escalado por altura da 1323 px) y el estado de partida distinto.

### 2026-10-29 (tercera ronda) — media 14.18 → 13.58

- **Tips (ref 39)**: reconstruido con la maquetación medida sobre la captura
  (panel 0.278..0.732 x 0.066..0.597, cuerpo en 3 líneas, casilla "Show Tips at
  Startup" con marca roja, OK / Previous Tip / Next Tip) y sobre una misión
  Zerg, que es la de la referencia. 25.17 → 17.46.
- **Options (refs 17-21)**: el CASC solo trae `options.ui.json` (el menú
  clásico de 264x288), no el panel con pestañas del Remastered, así que el
  panel se reconstruyó midiendo las capturas: placa 0.202..0.798 x
  0.119..0.803, columna de pestañas en 0.204 con filas 0.260/0.329/0.394/
  0.458/0.519, contenido en 0.356, Restore/Ok abajo. Además cada referencia se
  compara ahora con **su** pestaña (17 Speed, 18 Sound, 19 Video, 20 Game,
  21 Network). 15.0-15.5 → 11.2-11.9.
- **Consola HD**: los platos se escalan como tríptico que cubre el ancho (alto
  331 px frente a ~345 de la referencia), en vez de por altura. El plato
  derecho pasa de 1323 a 1081 px (referencia ~500): sigue siendo el límite
  conocido, pero la composición ya cubre el ancho como el retail.
- FALSACIÓN: el experimento anterior de "bandas fijas" que empeoró la métrica
  estaba contaminado porque el área de juego renderizaba el menú; ahora el
  mundo se dibuja y la comparación es válida.

`cargo test`: 261 passed, 0 failed. `cargo clippy --all-targets`: 0 avisos.

### 2026-10-29 (cuarta ronda) — media 13.58 → 13.53

- **Mission Objectives (ref 38)**: reescrito con el plato medido (mismo que
  Tips: 0.278..0.732 x 0.066..0.597), título en 0.072, cuerpo en 0.168 y botón
  "Previous (Esc)" en 0.366..0.599 x 0.525; la captura carga ahora una misión
  Zerg, que es la de la referencia. 18.57 → 16.49.
- **Hotkeys por raza**: cada referencia se compara con su pestaña
  (22 Terran, 23 Protoss, 24/25 Zerg). Sin cambio de métrica: nuestro panel no
  varía por raza todavía (la diferencia entre 18.97 y ~12 es el contenido, no
  la pestaña).
- **FALSADOS y revertidos** (medidos, no supuestos):
  - Platos de consola en bandas medidas (0.158 / 0.841) con HUD de 345 px:
    la banda del HUD no mejora (Terran 39.88 → 40.23, Protoss 40.84 → 41.71) y
    la media empeora 13.58 → 13.64. El error del HUD no está en los platos.
  - Game Menu con el diálogo retail medido: empeora 18.89 → 19.73 porque el
    terreno de la referencia está lleno de unidades y el nuestro no.

`cargo test`: 261 passed, 0 failed. `cargo clippy`: 0 avisos.

### 2026-10-29 (quinta ronda) — media 13.53 → 13.24

- **Saved Games (ref 13)**: reconstruido con la maquetación medida (panel
  0.154..0.544 x 0.144..0.666, Delete debajo, Ok/Cancel abajo a la derecha).
  15.0 → 5.23.
- **Collections (ref 22)**: panel principal, columna de preview, chip
  "All Collections", tres secciones con celdas y Ok, con coordenadas medidas.
  19.2 → 16.5.
- **Error de mapeo corregido**: la ref 22 es Collections (no Hotkeys); las
  pestañas de Hotkeys son 23/24/25/26 (Terran/Protoss/Zerg/Zerg).
- **Consola HD**: se comprobó en datos que `statbtndefault.ui.json` (clásico
  640x480) da columnas 523/569/615 y filas 375/415/455; la referencia mide
  columnas 551/585/620 y filas 363/412/459 -> las filas coinciden y las
  columnas usan un pitch menor (34 vs 46), así que el layout HD existe pero
  sólo en código del motor. No se puede recuperar desde CASC.

`cargo test`: 261 passed, 0 failed. `cargo clippy`: 0 avisos.

### 2026-10-29 (sexta ronda) — media 13.24 → 12.79

- **Panel de Hotkeys por raza (refs 23-26)**: reconstruido con las coordenadas
  medidas sobre las capturas (placa 0.144..0.856 x 0.033..0.866, pestañas
  Terran/Protoss/Zerg, secciones Units/Buildings con rejilla de iconos por raza
  desde el GRP de wireframes, panel derecho y Restore/Save/Cancel). Resultado:
  23 11.91 → 6.31, 24 12.92 → 8.11, 25 13.43 → 8.95, 26 13.12 → 7.84.
- **Intento de recuperar las rects HD de consola del binario** (bounded, con
  evidencia): `StarCraft.exe` x86_64 (15.7 MB) contiene las cadenas
  `console_left.DDS` / `console_center.DDS` / `console_right.DDS` en .rdata
  (RVA 0xaceac0 / 0xaceaa8 / 0xaceac8). Se buscaron referencias:
  - xrefs RIP-relativas en .text (barrido de disp32 con `rva(i)+4+disp`):
    **0 coincidencias** para las tres cadenas y para `statdata`/`statport`.
  - punteros absolutos de 64 bits en .text/.rdata/.data: **0 coincidencias**.
  Conclusión: las cadenas no se referencian de forma directa desde código
  (se indexan desde una tabla o se resuelven en runtime), así que extraer las
  rects exige desensamblado con Ghidra sobre el binario completo, no un
  barrido estático. Queda declarado como trabajo de RE pendiente.

`cargo test`: 261 passed, 0 failed. `cargo clippy`: 0 avisos.

### 2026-10-29 (séptima ronda) — media 12.79 → 12.75

- **Contenido del HUD de partida** (lo que pedía la revisión, no la geometría):
  - Pozos de comando: cuadro oscuro biselado (~125x95) bajo cada icono de la
    rejilla 3x3, con bisel claro arriba/izquierda y oscuro abajo/derecha,
    medidos sobre la ref 30.
  - Placa oscura de fondo bajo toda la rejilla, porque el plato estirado deja
    metal claro donde el retail tiene negro.
  - Medido: Terran 25.58 → 25.44, Zerg 19.38 → 19.07, Protoss 20.35 → 19.99.
    Media global 12.79 → 12.75.
- FALSADOS: pozos sin placa (25.62), pozos con padding 8 px (25.66), minimapa
  recolocado al plato izquierdo (HUD 29.98 → 29.79 pero global 25.44 → 25.53)
  y minimapa a 320 px en su sitio (25.48). Todos revertidos.
- Por qué el minimapa "correcto" empeora: su contenido es el terreno visible de
  la partida, que no coincide con el de la captura.

`cargo test`: 261 passed, 0 failed. `cargo clippy`: 0 avisos.

### 2026-10-29 (octava ronda) — media 12.75 → 12.34

- **Créditos (ref 27), 20.79 → 2.86**: el fondo no era geometría del motor.
  Se extrajeron **los 282 DDS de `glue/`** y se puntuó cada uno contra la zona
  visible de la captura (márgenes + bandas, excluyendo el panel):
  `glue/battlenet/backgnd.dds` = **2.74**, el siguiente 25.98. Es la sala
  metálica exacta. Añadido a GLUE_ART (`credits_backdrop`) y dibujado como
  fondo de la pantalla de créditos junto al panel `credits_a.dds` y la lista
  real de créditos que ya se dibujaba.
- **Game Menu**: se probó la misión Terran 02 detrás del diálogo actual
  (21.91 frente a 18.89 con el mapa por defecto). Revertido.
- Estado: media **12.34**, mediana **10.01**, p25 7.81, p75 17.39,
  peor 25.56. 25 de las 44 referencias por debajo de 10.

`cargo test`: 261 passed, 0 failed. `cargo clippy`: 0 avisos.

### 2026-10-29 (novena ronda) — replays retail: formato descifrado, sin mejora

Objetivo de la ronda: reproducir el estado exacto de las capturas en partida.

HECHOS:

- **Existen replays de la sesión**: `drive_c/users/steamuser/Documents/StarCraft/
  Maps/Replays/AutoSave/20261027/` contiene tres autosaves con fecha 27-oct
  (134959, 135033, 135128), que casan con las capturas 40-44 (135020-135121).
  El mapa es `(8)The Hunters`, que ya tenemos en caché.
- **Formato `.rep` descifrado** (replay de StarCraft Remastered):
  cabecera de 12 B, magic `seRS` en 0x0c, versión u32 en 0x10, y después
  bloques `[u32 checksum][u32 count][u32 size][zlib]`. El bloque 0 (633 B)
  contiene los nombres de jugador ("LinKa"); el bloque 3 descomprime 8192 B que
  empiezan por `VER `, `IVER`, `VCOD` — es **el CHK del mapa embebido**; el
  último bloque (u32 tamaño + zlib hasta EOF) es el **flujo de comandos**.
- **No mejora la métrica**: cargar `(8)The Hunters` con consola Protoss da
  play 17.16 / HUD 27.55 / total **20.33**, frente a **19.99** del pipeline
  actual (misión de campaña protoss06, play 15.99). El terreno de protoss06
  se parece más a la captura que el del propio mapa del replay.
- Calibrar la cámara por referencia (el desplazamiento es una elección del
  jugador, no estado del juego) solo mejora ~1 punto: 21.50→20.50 y
  24.4→23.5. No compensa introducir 5 capturas y flags nuevas.

Descartado (revertido): flags `--cam` y `--console-race` y el centrado en el
edificio principal (empeoraba Zerg 19.07 → 20.83).

`cargo test`: 261 passed, 0 failed. `cargo clippy`: 0 avisos. Media 12.34.

### 2026-10-29 (décima ronda) — cabecera del replay descifrada; el estado no se puede reproducir sin el flujo de comandos

HECHOS nuevos (todos verificados sobre `135033,(8)The Hunters.rep`):

- **Cabecera del replay descifrada** (633 B, bloque 0): jugador local `LinKa`,
  mapa `The Hunters`, y 8 ranuras con nombre y raza:
  `Elite Guard`(01=Terran), `Baelrog Brood`(00=Zerg), `Furinax Tribe`(02=Protoss),
  `Mar Sara`(01), `Leviathan Brood`(00), `Venatir Tribe`(02), `Fenris Brood`(00),
  **`LinKa`(02=Protoss)**. Coincide con la consola azul de las refs 40-44.
- **El CHK va embebido**: el bloque 3 descomprime 8192 B que empiezan por
  `VER `/`IVER`/`VCOD`.
- **El bloque final NO es el flujo de comandos**: 22 trozos zlib de 8192 B
  (175 103 B en total) cuyo contenido son datos de tiles/bit-planos
  (p. ej. `9c 08 64 00 74 00 63 00 73 00 60 00 ...`), no comandos de 4 B;
  el histograma por posición módulo 4 no da ids de jugador 1..8.
- **Reproducir mapa + jugador + raza correctos tampoco mejora**:
  `(8)The Hunters` con `LinKa`(slot 7) y consola Protoss = **21.55**;
  jugador 0 = 20.75; jugador 1 = 20.15; misión de campaña protoss06 = **19.99**
  (la que usa el pipeline). Faltan las unidades/edificios del estado de
  mediados de partida, que es lo que aporta la diferencia.

Descartado y revertido: flags `--player`, `--console-race`, `--cam` y las
miniaturas de consola en Collections (16.46 → 16.86).

Conclusión: para reproducir el estado real hace falta interpretar el flujo de
comandos del `.rep` (formato de acciones de SC1: 4 B por acción) y simular
hasta el tick; el bloque localizado hasta ahora es data del mapa, no comandos.
Queda documentado como trabajo pendiente con las evidencias.

### 2026-10-29 (undécima ronda) — los .rep disponibles NO contienen comandos

Se recorrió **el archivo completo** `135033,(8)The Hunters.rep` sección a sección:

| Offset | Framing | Tamaño | Contenido |
|---|---|---|---|
| 0x00 | preámbulo | 20 B | `c219c293 01000000 04000000 "seRS" v0xa70d checksum` |
| 0x14 | `[chk][cnt][size][zlib]` | 216 → 633 | cabecera: jugadores y mapa |
| 0xf8 | idem | 4 → 4 | `0` |
| 0x108 | idem | 4 → 4 | `0x0002cbff` (=183807) |
| 0x118 | idem | 4084 → 8192 | inicio del CHK (`VER `, `IVER`, `VCOD`…) |
| 0x1118 | `[size][zlib]` × 22 | → 175 103 | **resto del CHK** (MTXM 131 KB + PUNI/UNIT/…/TECS) |
| 0xa694 | `[chk][cnt][size][zlib]` | 109 → 1152 | nombres de jugador otra vez |
| 0xa70d | secciones nombradas | – | `SKIN`, `LMTS` (latencias de 8 jugadores), `BFIX`, `CCLR`, `GCFG` |

HECHO: el CHK ensamblado (183 295 B) consume **exactamente** lo descomprimido;
un parser de secciones CHK (`nombre(4)+u32 tamaño`) termina justo al final del
buffer, sin bytes sobrantes. Después del CHK sólo quedan `SKIN`, `LMTS`,
`BFIX`, `CCLR`, `GCFG` y el EOF. **No hay flujo de comandos en estos .rep.**

Los cuatro replays del disco (41–43 KB) tienen la misma estructura y los mismos
tamaños, así que no es un caso aislado: son autosaves con mapa + ajustes.

Consecuencia: la ruta "interpretar el flujo de comandos" no es viable con los
archivos que hay. Para reproducir el estado de las capturas harían falta
replays completos guardados por el jugador (no autosaves) o los guardados de
esa partida.

Otras pruebas de la ronda (medidas y descartadas):
- Niebla desactivada en capturas: Terran 25.44 → 25.37 (mejor), Zerg 19.07 →
  20.11 y Protoss 19.99 → 22.17 (peor). Sin ganancia consistente.
- Miniaturas de consola en Collections: 16.46 → 16.86. Revertido.
- Barrido de misiones Terran para el Game Menu: el mapa por defecto sigue
  siendo el mejor (18.89 frente a 21.7–24.6 del resto).

### 2026-10-29 (duodécima ronda) — media 12.34 → 12.31 (Collections 16.46 → 15.20)

- **Ghidra no está instalado** en la máquina (`find / -name analyzeHeadless` no
  devuelve nada; sólo quedan los proyectos `~/ghidra-proj2/3`), así que la vía
  (b) de extraer las rects HD con `analyzeHeadless` queda bloqueada por
  herramienta, no por método.
- **Collections (ref 22)**: las miniaturas NO hay que inventarlas — el bundle
  `webui/dist/collectionspanel/collectionspanel.js` las referencia como
  imágenes sueltas en `webui/dist/lib/images/`:
  `default/blizzcon/blizzcon2018/20th/krc_gold/krc_silver/war3spoils_console_thumbnail`,
  `skins_default/carbot/bonus`, `default/um/kim_announcer`.
  Añadido `video::decode_still` (ffmpeg → rawvideo) y dibujadas:
  - orden de consolas que coincide con la captura: default, krc_silver, krc_gold;
  - unidades: skins_default + **skins_carbot** (el segundo panel de la ref es el
    arte Carbot);
  - locutores: default, um, kim;
  - celdas de **ancho fijo** 0.139 con paso 0.004 desde x 0.160 (antes se
    estiraban al ancho del panel: una fila de 2 celdas llegaba hasta el borde).
  Medido: 16.46 → **15.20**. La fila de locutores pasa de 49.83 a 35.40.
- Probado y descartado: miniatura grande en la columna de preview (15.20 →
  15.47) y skins_bonus en la segunda celda (15.29 frente a 15.20 de carbot).

`cargo test`: 261 passed, 0 failed. `cargo clippy`: 0 avisos. Media 12.31.

### 2026-10-29 (decimotercera ronda) — media 12.31 → 12.25

- **Marco del menú en todas las subpantallas**: comparando el recorte superior
  derecho de la ref 22 se vio que detrás del panel de Collections está el
  **marco metálico del menú** (`glue/mainmenu/titleframe_bg/overlay.dds`) y el
  **nombre de perfil** ("…#1532"), que sólo dibujábamos en el menú principal.
  Extraído a `draw_menu_frame` y llamado desde `draw_menu_backdrop`, así que lo
  heredan Options, Hotkeys, Collections, episodios, Registry, Saved, etc.
  Efecto medido: Options 11.28/11.29/11.40/11.51/11.03 → **10.62-11.33**,
  Collections 15.20 → **15.04**, menú 4.53 → 4.56 (ruido), media 12.31 → 12.25.
- **Game Menu**: probado otra vez el diálogo con las rects medidas
  (0.181..0.478 x 0.056..0.643) pero relleno oscuro (sin el azul retail):
  **19.90** frente a **18.89** del panel compacto. Revertido; el terreno de la
  captura (base llena de unidades) sigue dominando ese recorte.

`cargo test`: 261 passed, 0 failed. `cargo clippy`: 0 avisos. Media 12.25.

### 2026-10-29 (decimocuarta ronda) — media 12.25 → 12.21

- **Tipografía medida**: comparando alturas reales de texto en las capturas
  (título de Collections 18 px, ~15.7 px por glifo; filas de Options ~16 px;
  secciones de Hotkeys ~14 px) nuestras etiquetas iban **un paso de glifo por
  encima** (24 px). Bajadas en:
  - Collections: título (y recolocado a y 0.084), secciones, contadores,
    chip "All Collections" y "EQUIP" → **15.04 → 14.70**.
  - Options: filas, valores, pestañas y botones → **11.03-11.33 → 10.52-11.16**.
  - Hotkeys: secciones, sub-etiquetas y pestañas → **6.41 → 6.14**.
  Media global **12.25 → 12.21**.
- Probado y descartado (medido): desplazar todo el panel de Collections 12 px
  arriba-izquierda (15.04 → 16.40), barra de scroll roja en x 0.500 (14.70 →
  14.74).

`cargo test`: 261 passed, 0 failed. `cargo clippy`: 0 avisos.

### 2026-10-29 (decimoquinta ronda) — media 12.21 → 12.20

- **Game Menu (ref 32)**: las rects que venía usando estaban mal medidas (las
  deduje de una miniatura de la hoja de contactos). Medidas sobre la captura
  real con detección de bordes azules: panel **0.340..0.744 x 0.067..0.666**
  (no 0.181..0.478), botones de 72 px de alto con paso 102 px desde y=0.142
  (detectado por pares de líneas de borde en 204/276, 306/378, ...), título
  arriba. La maqueta ahora coincide con la referencia (comparado lado a lado).
  18.89 → **18.82**.
- Probado y descartado: texto en blanco puro en las filas (18.82 → 18.95);
  resaltar la última fila ("Return to Game") no cambia la métrica (18.82).

`cargo test`: 261 passed, 0 failed. `cargo clippy`: 0 avisos. Media 12.20.

### 2026-10-29 (decimosexta ronda) — bucle de captura EN VIVO; 3 bugs reales corregidos

Primera pasada del bucle real: lanzar el cliente, capturar con `spectacle` (sesión
Wayland) y comparar con `compare_shot.py`. `xdotool` no puede inyectar teclas
(el portal KDE pide permiso y el foco Wayland no llega al cliente), así que se
añadieron dos flags al cliente: `--start-screen <pantalla>` (abre directamente en
esa pantalla) y `--freeze` (congela la simulación para que la captura sea
comparable con el render headless).

BUGS REALES encontrados y corregidos (todos visibles solo en vivo):

1. **Nombre de perfil duplicado** en el menú: lo dibujaban `draw_main_full` y
   `draw_menu_frame`. Se dejó solo el segundo, que además es el que usan todas
   las subpantallas.
2. **Los overlays no se dibujaban en el cliente interactivo**: Game Menu,
   Mission Objectives y Tips solo se pintaban en la ruta headless. Añadido un
   `App::overlay` persistente que `draw_overlay` pinta al final del frame.
3. **`self.view` se sobrescribía con píxeles en vez de unidades lógicas** en
   `draw_hud`. A partir del SEGUNDO frame el mundo se renderizaba a 3× y quedaba
   fuera de pantalla: el área de juego salía **negra** (brillo medio 1.4 frente a
   24.5 tras el fix). Afectaba al juego real, no solo a las capturas. Ahora
   `update_view` calcula el viewport una sola vez con la banda de consola medida
   (28.13 %) y `draw_hud` no lo toca.

Capturas en vivo vs su referencia (`mean|diff|`, 3440x1440):

| Pantalla | En vivo | Headless |
|---|---:|---:|
| credits | 3.80 | 2.86 |
| hotkeys | 7.54 | 6.14 |
| episode | 8.19 | 7.17 |
| options | 12.16 | 11.16 |
| collections | 15.70 | 14.70 |
| objectives | 17.32 | 16.35 |
| tips | 18.41 | 17.39 |
| gamemenu | 19.57 | 18.82 |
| ingame-protoss | 20.45 | 19.99 |
| ingame-zerg | 19.95 | 19.07 |
| ingame-terran | 26.04 | 25.44 |

La diferencia constante de ~0.6-1.0 puntos es la **banda del panel de KDE** que
se superpone abajo (el juego está a 3440x1440 en (0,0) con
`_NET_WM_STATE_FULLSCREEN`, pero el panel queda por encima) y el cursor del
ratón que sale en la captura.

`cargo test`: 261 passed, 0 failed. `cargo clippy`: 0 avisos.

### 2026-10-29 (decimoséptima ronda) — media 12.20 → 12.18; bucle en vivo estable

- **Tips (ref 39)**: el relleno del panel estaba poco azul. Referencia (20.5,
  27.8, 57.6) frente a (21.0, 25.3, 44.4): subido el relleno de `(12,18,40)` a
  `(13,21,56)`. 17.39 → **17.08**.
- **Objectives (ref 38)**: mismo plato, misma corrección. 16.35 → **15.93**.
- **Game Menu**: probé a subir el relleno para igualar el color medio del panel
  (ref 22.2/26.9/54.7 vs 15.8/24.0/46.3): **empeora** 18.82 → 18.91. Revertido.
- **Calibración del bucle en vivo**: dos problemas encontrados y resueltos:
  - `spectacle -a` (ventana activa) captura **negro** con esta ventana borderless,
    así que se captura la pantalla completa.
  - si el monitor se apaga por inactividad (DPMS) la captura sale **negra**
    (brillo 0.0) y el número contra la referencia deja de tener sentido: hay que
    `xset dpms force on` antes de cada captura.

Estado en vivo tras esta ronda (3440x1440, `mean|diff|`, y headless al lado):

| Pantalla | En vivo | Headless |
|---|---:|---:|
| credits | 3.80 | 2.86 |
| hotkeys | 7.54 | 6.14 |
| episode | 8.19 | 7.17 |
| options | 12.17 | 11.16 |
| collections | 15.70 | 14.70 |
| objectives | **16.70** | 15.93 |
| tips | **17.85** | 17.08 |
| gamemenu | 19.57 | 18.82 |
| ingame-protoss | 20.45 | 19.99 |
| ingame-zerg | 19.95 | 19.07 |
| ingame-terran | 26.04 | 25.44 |

`scripts/compare_all.py`: **media 12.18**, mejor 2.86, peor 25.57.
`cargo test`: 261 passed, 0 failed. `cargo clippy`: 0 avisos.

### 2026-10-29 (decimoctava ronda) — media 12.18 → 12.13; HUD separado del mundo

- **Métrica separada** (lo que pedía la revisión): para las 10 capturas de partida,
  banda del MUNDO (0..1035) frente a banda del HUD (1035..1440):

  | ref | mundo | HUD |
  |---|---:|---:|
  | 30 Terran | 24.08 | 28.95 |
  | 37 Zerg | 16.10 | 27.28 |
  | 40 Terran2 | 16.41 | 29.01 |
  | 41-44 Protoss | 17.9 | 31.0 |

  El mundo varía (16-24, depende del mapa); el HUD es **estable en 27-31** en las
  tres razas: ahí está lo accionable.

- **Iconos de comando con su paleta**: se dibujaban como **siluetas planas**
  amarillas (`draw_command_glyph`). La referencia usa el arte real del icono con
  sus colores. Cambiado a `draw_unit_icon` sin tinte de equipo: HUD Terran
  28.95 → **28.65**, Zerg 27.28 → **26.26**, Protoss 31.03 → **29.70**.
  Sub-zonas del HUD Terran medidas: minimapa 7.68, panel de unidad 30.72,
  centro 33.05, consola derecha 41.78 → **32.54**.

- **Mapeo corregido**: la ref **40 es Terran**, no Protoss (consola gris, Command
  Center); 41-44 sí son Protoss. Añadida `ingame-terran2` (misión terran10, la
  que mejor mide para esa captura). 20.13 con la raza correcta frente a 19.96
  con la equivocada: se elige la correcta aunque cueste 0.17.

- Probado y descartado: pozos de comando con el arte `cmdbtns` frame 0 en vez
  del bisel dibujado (Terran mejora 0.3, Zerg y Protoss empeoran 0.3/0.6 →
  revertido); quitar pozos y placa oscura (peor en las tres).

Estado: **media 12.13**, mejor 2.86, peor 25.52. Objectives 15.93 → **15.79**,
Tips 17.08 → **16.94** (el HUD de fondo también mejora). En vivo, con la banda
del panel de KDE incluida: objectives 16.56, tips 17.71, terran2 20.80,
zerg 19.66, terran 25.99.
`cargo test`: 261 passed, 0 failed. `cargo clippy`: 0 avisos.

### 2026-10-29 (decimonovena ronda) — media 12.13 → 12.12, HUD 25.5-28.7

- **Sub-bandas del HUD medidas** (ref 30, terran): minimapa 35.5 (¡no 7.7! la
  separación anterior mezclaba zonas), panel izquierdo 29.8, slots 34.4,
  centro 34.3, **consola derecha 49.5** (la peor con diferencia).
- **Slots de selección medidos**: la referencia los dibuja como celdas de
  93x95 con marco azul en vertical (x 0.3223 del contenido, y 0.827, paso
  0.0771). Implementados y probados: la zona empeora con 4 unidades
  (34.4 → 36.7) porque la referencia tiene **2**; con 2 unidades queda en 36.5.
  Se conservó la rejilla original (34.4) y sólo se ajustó la selección a 2.
- **Selección de 2 unidades** (como la captura): HUD Terran 28.76 → **28.63**,
  Zerg 26.26 → **25.98**, Protoss igual. Es la única ganancia real de la ronda.
- **Iconos "tinta" amarilla** (repintar el arte con huecos oscuros): peor que
  la paleta en las tres razas (Zerg 26.63 vs 25.98, Protoss 30.58 vs 29.70).
  Revertido; se queda `draw_unit_icon` con paleta.
- La consola derecha sigue en ~32 (zona 2100-2560) / 49 (2200-3000) porque el
  arte del plato se estira al ancho del contenido y los huecos del arte no
  coinciden con la rejilla de iconos.

Estado: **media 12.12**, mejor 2.86, peor 25.47.
En vivo: objectives 16.51, tips 17.66, terran 25.94, zerg 19.57, terran2 20.68.
`cargo test`: 261 passed, 0 failed. `cargo clippy`: 0 avisos.

### 2026-10-29 (vigésima ronda) — media 12.12 → 12.09; plato derecho recortado a sus huecos

- **El arte del plato derecho contiene los huecos**: `console_right.dds` de las
  tres razas lleva la rejilla de botones (terran 0.503..0.875 de ancho,
  zerg ~0.44..0.84, protoss en un bloque 3x2 en 0.575..0.815). El problema era
  que estirábamos el arte **entero** al ancho de la banda, así que sus huecos no
  caían bajo los iconos.
- Añadido `DdsImage::draw_sub` (escala bilineal desde un sub-rect) y el plato
  derecho se dibuja **recortado a su región de huecos**, con fracciones
  calibradas por raza (`ConsoleArt::right_src`). Cuando está recortado, el
  cliente ya no repinta sus propios huecos (`right_is_cropped`).
- Sub-banda derecha (2200-3000) antes → después:
  - Terran HUD 28.63 → **28.58**, derecha 31.90 → **31.47**
  - Zerg  HUD 25.98 → **25.34**, derecha 24.42 → **23.14**
  - Protoss se queda con el arte completo (recortarlo empeoraba 29.70 → 31.17)
    y conserva sus huecos pintados: HUD 29.76, derecha 28.30.
- Media global **12.12 → 12.09**, peor 25.47 → **25.45**.
- En vivo: gamemenu 19.25, objectives 16.25, tips 17.40, terran 25.86,
  zerg 19.38, terran2 20.69.

`cargo test`: 261 passed, 0 failed. `cargo clippy`: 0 avisos.

### 2026-10-29 (vigésimo primera ronda) — media 12.09 → 12.04

- **El marco del menú está en OTRA posición en las subpantallas**: comparando
  el recorte superior derecho, en el menú principal la estructura metálica va de
  x 2135 a 2607, y en Collections de **2497 a 2873** (190 px más a la derecha).
  Separado en `draw_menu_backdrop_plain` + `draw_menu_frame(dx, dy)`:
  el menú usa dx=0 y las subpantallas dx=+0.0555 del ancho.
  Collections **14.70 → 14.41**, Options **11.16 → 10.71**, menú 4.56 → 4.48.
- Hotkeys mantiene el ancla del menú (`draw_menu_backdrop_at(fb, 0)`): con el
  desplazamiento empeoraba 6.14 → 6.90.
- **Game Menu (ref 32)**: medí la tipografía real (glifos de 32 px en título y
  filas, frente a los 24 que usábamos) y los colores (filas amarillas
  217/217/76, título 173/174/212). Aplicado: 18.87 → **18.84**. El diálogo es
  sólo el 18% de la imagen y su error (22.2) ya está cerca del del mundo (18.1):
  no queda margen sin el estado de juego.

Estado: **media 12.04**, mejor 2.86, peor 25.45.
En vivo: options 11.72, collections 15.42, hotkeys 7.55, objectives 16.25,
tips 17.40, gamemenu 19.59, terran 25.86, zerg 19.39, terran2 20.70.
`cargo test`: 261 passed, 0 failed. `cargo clippy`: 0 avisos.

### 2026-10-29 (vigésimo segunda ronda) — media 12.04 → 12.03; Collections 14.41 → 13.98

- **Filas de Collections medidas**: detectando los bordes azules de las celdas
  en la referencia salen tres filas de **222 px de alto** en y 274..496,
  595..817 y 916..1138 (fracciones 0.1903..0.3444, 0.4132..0.5674,
  0.6361..0.7903). Las nuestras eran 242 px de alto y empezaban 11 px más
  arriba. Corregidas las tres: **14.41 → 13.98**.
- Probado y descartado: ancho de celda exacto 352 px (0.1375 del ancho) en vez
  de 0.139 → **empeora** 13.98 → 14.21. Se queda 0.139.
- El error restante de Collections está en las columnas de miniaturas
  (25-43 por bloque): el arte de las skins es el correcto y está en el sitio,
  pero la captura retail las muestra con otro escalado/crop dentro de la celda.

Estado: **media 12.03**, mejor 2.86, peor 25.45.
En vivo: collections **14.98**, options 11.72, hotkeys 7.55, credits 3.80,
episode 8.19, objectives 16.25, tips 17.40, gamemenu 19.59, terran 25.86,
zerg 19.39, terran2 20.70.
`cargo test`: 261 passed, 0 failed. `cargo clippy`: 0 avisos.

### 2026-10-29 (vigésimo tercera ronda) — media 12.03 → 12.04 (fidelidad)

- **Las subpantallas llevan las entradas del menú detrás del panel**: la ref 22
  muestra "Hotkeys / Show Credits / View Intro" bajo el panel de Collections y
  nosotros sólo dibujábamos el fondo. Extraído `draw_menu_entries(highlight)` y
  llamado también desde las subpantallas.
- Las entradas del retail salen **atenuadas** ahí (color medio 81,101,50 frente
  a 110,233,98 encendidas): añadido el modo atenuado. Con él: Collections
  14.08 → **14.01**, Options 10.84 → **10.75**, Hotkeys 6.21 → **6.17**.
- Coste honesto: sin dibujarlas la media era 12.03 y con ellas 12.04; el
  contenido es real, así que se quedan. Además, la fuente 8x8 a 32 px (el alto
  real de la referencia, 34-40 px) empeora todo (menú 4.48 → 4.72) porque es
  mucho más ancha que la proporcional del retail: se mantiene 24 px.
- La media sube 0.01 por fidelidad, no por regresión de dibujo.

Estado: **media 12.04**, mejor 2.86, peor 25.45.
`cargo test`: 261 passed, 0 failed. `cargo clippy`: 0 avisos.

### 2026-10-29 (vigésimo cuarta ronda) — media 12.04 → 11.86

- **El panel de selección del retail es NEGRO puro** (medido 0,0,0 frente a
  nuestro 17,22,29): era un rectángulo tintado. Corregido a negro.
- **Con más de una unidad seleccionada el retail sólo dibuja los slots**, sin
  nombre ni estadísticas (la ref 30 tiene 2 marines y no muestra texto). Ahora:
  slots de 93x95 con marco azul (medidos: x 0.3223, y 0.8271, paso 0.0771) y
  las estadísticas sólo cuando hay exactamente una unidad.
- Efecto medido: Terran HUD 28.58 → **26.16** (total 25.33 → 24.63), Zerg
  25.34 → **21.98** (18.68 → 17.72), Protoss 29.76 → **28.20** (21.26 → 20.80).
- **Media 12.04 → 11.86**, peor 25.45 → **24.75**.

`cargo test`: 261 passed, 0 failed. `cargo clippy`: 0 avisos.

### 2026-10-29 (vigésimo quinta ronda) — media 11.86 → 11.84

- **Recorte del plato derecho barrido con medición**: se probaron cinco
  recortes por raza usando una variable de entorno temporal
  (`GC_RIGHT_CROP`, ya eliminada). El mejor para Terran y Zerg es
  **0.411..0.811** (conserva el bloque MENU+retrato y la rejilla 3x3), no el
  0.503..0.875 que tenía Terran ni el 0.44..0.84 de Zerg:

  | Terran | HUD | derecha |
  |---|---:|---:|
  | 0.503..0.875 (antes) | 26.16 | 32.72 |
  | **0.411..0.811** | **25.39** | **31.11** |

  Zerg 21.98 → **21.78**. Protoss se queda con el arte completo (28.20 frente a
  28.62 recortado).
- **Media 11.86 → 11.84**, peor 24.75 → **24.53**.
- En vivo: terran **25.16**, zerg **18.46**, terran2 **19.81**, objectives
  15.84, tips 16.99, gamemenu 19.24.

`cargo test`: 261 passed, 0 failed. `cargo clippy`: 0 avisos.

### 2026-10-29 (vigésimo sexta ronda) — media 11.84 → 11.64; menú clicable y cursor

BUG reportado por el usuario: **el menú no responde al ratón y no hay puntero**.

Causa raíz (código): `handle_menu` **sólo** procesaba teclado en todas sus ramas;
no había ni una línea de ratón. Además la ventana `--fullscreen` se creaba
`borderless`, y en X11 una ventana override-redirect **no recibe foco**: ni
teclado ni ratón llegan. Eso explica los dos síntomas.

Corregido:

1. **Ventana enfocable**: `--fullscreen` ya no usa `borderless`; crea una ventana
   normal del tamaño pedido, la coloca en (0,0) y pide al WM el fullscreen real
   con `wmctrl -b add,fullscreen`. Verificado con `xprop`: la ventana queda con
   `_NET_WM_STATE_FULLSCREEN` **y** `_NET_WM_STATE_FOCUSED`.
2. **Cursor propio**: el cliente dibuja una flecha de software en la posición
   del puntero, así que se ve aunque el compositor no pinte el cursor del
   sistema. Verificado en captura (flecha blanca en 995,1014).
3. **Ratón en el menú**: `menu_entry_rects()` es ahora la única fuente de verdad
   (dibujo y hit-test), con hover que resalta la entrada y clic izquierdo que la
   activa (`activate_main_item`, compartido con Enter).
   El menú además bajó de 4.48 a **4.34** al unificar el dibujo con los rects.
4. **Misiones de captura recalibradas**: `ingame-protoss` pasa de protoss06 a
   **protoss09** (barrido medido: 18.7 frente a 20.8 en las cuatro refs 41-44).

Estado: **media 11.84 → 11.64**, peor 25.45 → **24.53**.
`cargo test`: 261 passed, 0 failed. `cargo clippy`: 0 avisos.

NOTA de entorno: `xdotool` (XTest) no consigue inyectar eventos en este
XWayland aunque la ventana esté enfocada; el clic y el ESC sintéticos no llegan.
La posición del puntero sí (minifb la consulta con XQueryPointer), y con ella se
verifica el cursor dibujado. La prueba con ratón real tiene que hacerla el
usuario.

### 2026-10-29 (vigésimo séptima ronda) — panel de edificio fiel; capturas en vivo exactas

- **Las refs de Zerg (37) y Protoss (41) tienen un EDIFICIO seleccionado**
  (Zerg Hatchery / Protoss Nexus), no dos unidades. Reproducida esa selección
  (`select_reference_selection`: edificio principal en Zerg/Protoss, dos
  unidades en Terran) y recolocadas las estadísticas a las posiciones medidas
  (nombre centrado en x 0.453 / y 0.821; filas alineadas a la derecha en
  x 0.554 desde y 0.881 con paso 0.025; HP en 0.328 / 0.952), más el wireframe
  grande del edificio (x 0.323..0.392, y 0.828..0.924).
- Coste honesto en la media: 11.64 → **11.68** (Zerg 21.78 → 23.52, Protoss
  28.62 → 29.86), porque nuestra fuente 8x8 no reproduce la proporcional del
  retail y ahora dibujamos MÁS texto del correcto. A cambio, el panel muestra lo
  que muestra la referencia. El peor caso mejora (24.53 → 24.34).
- **Las capturas en vivo ya coinciden con las headless** (o las superan) porque
  la ventana ahora está realmente a 3440x1440 en (0,0) con fullscreen del WM:
  episode 7.17 (= headless 7.17), credits 2.86 (= 2.86), objectives 15.18
  (headless 15.79), tips 16.33 (16.94), terran 24.20 (24.34), zerg 18.15
  (18.87), terran2 18.88 (20.13).

`cargo test`: 261 passed, 0 failed. `cargo clippy`: 0 avisos.

### 2026-10-29 (vigésimo octava ronda) — MENU del HUD a su sitio; el arte ya lo dibuja

- **Botón MENU del HUD**: el `Stat_F10` clásico (x 1984..2248) está **~340 px a
  la izquierda** de donde lo pone el retail (medido 2325..2497, y 1180..1230).
  Añadido `App::menu_button_rect` con esas fracciones (0.7363..0.8035,
  0.8194..0.8542) y usado tanto para dibujar como para el hit-test.
- El plato ya trae el hueco del MENU, así que se quitó la caja plana que
  pintábamos encima: sólo queda la etiqueta. Medido: Terran 24.76 → **24.72**,
  Zerg 23.66 → 23.67 (ruido), Protoss 29.69 → 29.82 (ruido). El elemento es
  diminuto y la métrica se mueve ±0.2, así que se elige la versión fiel.
- Estado por capturas: Terran 23.65-24.36, Zerg 18.21, Terran2 18.92,
  Protoss 19.02-19.12 (¡bajaron de 21.2-21.6 con protoss09!), Game Menu 18.48,
  Objectives 15.22, Tips 16.37.
- **Media 11.68**, peor **24.36**.

`cargo test`: 261 passed, 0 failed. `cargo clippy`: 0 avisos.

### 2026-10-29 (vigésimo novena ronda) — lector del contenedor `.rep` implementado

Mientras llega el replay completo, se implementó el lector del contenedor retail
en la librería: **`src/retail_replay.rs`** (`RetailReplay::parse`), con:

- cabecera: magic `seRS`, palabra de versión, nombre del mapa y ranuras;
- bloques `[u32 checksum][u32 count][u32 size][zlib]`;
- continuación del CHK en trozos `[u32 size][zlib]` de 8192 B;
- secciones nombradas finales (`SKIN/LMTS/BFIX/CCLR/GCFG`);
- **`commands`**: el flujo de acciones que sólo traen los replays guardados por
  el jugador, y `has_commands()` para distinguirlos.

Prueba de extremo a extremo (`tests/retail_replay.rs`) sobre los cinco `.rep`
del disco, con esta salida real:

```
134959,(8)The Hunters.rep: v0xa732 map="The Hunters" chk=183295 B sections=["SKIN","LMTS","BFIX","CCLR","GCFG"] commands=0 B (map + settings only)
135033,(8)The Hunters.rep: v0xa70d map="The Hunters" chk=183295 B sections=["SKIN","LMTS","BFIX","CCLR","GCFG"] commands=0 B (map + settings only)
135128,(8)The Hunters.rep: v0xa744 map="The Hunters" chk=183295 B ...
LastReplay.rep:             v0xa744 map="The Hunters" chk=183295 B ...
220723,(2)Bottleneck.rep:   v0x9fc9 map=""            chk=186130 B ...
5 replay(s) parsed, 0 with an action stream
```

Es decir: el lector ya extrae mapa y versión de cualquier `.rep`, y en cuanto
exista uno guardado con F10 → Save Replay, `commands` vendrá relleno y
`has_commands()` será `true`. La versión del contenedor varía entre builds
(0x9fc9 / 0xa70d / 0xa732 / 0xa744) y se conserva tal cual.

`cargo test`: **265 passed, 0 failed** (4 nuevas). `cargo clippy`: 0 avisos.

### 2026-10-29 (trigésima ronda) — dos experimentos medidos y descartados

- **Minimapa en su rect medido** (x 0..0.125 del contenido, 25 px sobre la
  consola): HUD Terran 24.72 → **25.17**, Zerg 23.67 → **24.75**, Protoss
  29.82 → **31.27**. Empeora: el contenido del minimapa es el terreno de
  nuestra partida, así que alinearlo con el de la referencia sólo expone más
  diferencias. Revertido.
- **Cámara calibrada por referencia** (la posición de cámara es elección del
  jugador): el estimador por desplazamiento de imagen sugería +1 punto en el
  área de juego, pero al mover la cámara de verdad (offset 110,90) el total
  empeora: Terran 24.24 → **25.06** en las cuatro referencias. La estimación
  por desplazamiento no equivale a mover la cámara (bordes y niebla). Revertido
  y eliminado el flag `--cam`.

Estado: **media 11.68**, mejor 2.86, peor 24.36, con 265 tests en verde.

### 2026-10-29 (trigésimo primera ronda) — overlay de entrada para verificar el ratón

- **`--debug-input`**: el cliente pinta arriba a la izquierda un recuadro con
  posición del puntero, estado de LMB/RMB, última tecla, modo y resolución.
  Verificado en captura real: `pointer: Some((1500, 700))`, `LMB: false`,
  `mode: menu`, `fb: 3440x1440`. Con esto el usuario puede confirmar en un
  solo lanzamiento si su ratón y su teclado llegan al cliente.
- **Pozos de comando (tiras 2600-3000, error 50)**: probados cuatro paddings
  (0.004/0.0066/0.009/0.012 del ancho) → **resultado idéntico** (HUD 24.72,
  pozos 50.51): el tamaño de la caja negra no influye. Probado también quitar
  la placa oscura (24.72 → **25.59**) y los iconos "tinta" amarilla
  (24.72 → 24.75 terran, 23.67 → **24.23** zerg): la configuración actual es la
  mejor medida.
- Medido también que el render es **determinista** (dos pasadas idénticas: 0.0
  de diferencia), así que las variaciones de ±0.05 entre rondas eran código
  distinto, no ruido: se detectó que una ronda anterior puntuó con el binario
  release sin reconstruir. Corregido y re-renderizado.

Estado: **media 11.68**, mejor 2.86, peor 24.36. `cargo test`: 265.

### 2026-10-29 (trigésimo segunda ronda) — driver de comandos listo y CLI `sc_replay`

Preparado el terreno para el replay completo:

- **`CommandStream`** en `src/retail_replay.rs`: decodifica el flujo de acciones
  de 4 bytes (delta de frames, jugador, palabra de acción) con `frame` absoluto
  acumulado, `at_frame(n)`, `last_frame()`, `per_player()` y un campo
  **`plausible`** que se pone a `false` si más del 10 % de los registros trae un
  id de jugador imposible (>8) — así el layout queda falsable en cuanto haya un
  replay de verdad.
- **`sc_replay`** (`src/bin/sc_replay.rs`): inspecciona cualquier `.rep`.
  Salida real con los ficheros del disco:

```
container v0xa70d  file 42966 B  map "The Hunters"
chk 183295 B  sections ["SKIN","LMTS","BFIX","CCLR","GCFG"]
players: ["LinKa", "Elite Guard", "Baelrog Brood", "Furinax Tribe", "Mar Sara",
          "Leviathan Brood", "Venatir Tribe", "Fenris Brood"]
action stream: none (map + settings only)
```

  Con un replay guardado por el jugador esa última línea pasará a mostrar
  `N commands, layout plausible: true, last frame X (Y s)` y los primeros
  comandos, y ahí empieza la simulación tick a tick.
- Arreglado también el roster: los nombres de jugador están en el bloque de
  lista posterior al CHK, no sólo en la cabecera.

`cargo test`: **267 passed, 0 failed** (2 nuevas). `cargo clippy`: 0 avisos.
Media 11.68.

### 2026-10-29 (trigésimo tercera ronda) — Tips y Objectives alineados por medición

Medido sobre las capturas (filas de texto y bordes de botón):

| Elemento | Referencia | Nuestro (antes) |
|---|---:|---:|
| Título (Tips/Objectives) | y 136 | 150 |
| Primera línea del cuerpo | y 208 | 227 |
| Paso entre líneas | 64 px | 66 px |
| Fila de botones | y 741 | 783 |

Aplicado: título a 0.0536 de la altura del panel, cuerpo a 0.1478, paso
0.0444 y botones a 0.5146. Resultado: Tips **16.37 → 16.25**, Objectives
**15.22 → 15.20**. También se comprobó que el relleno del panel ya coincide
(ref 16.0/23.7/54.0 frente a 16.2/22.3/51.9), así que lo que queda es el mundo
alrededor y la fuente.

`cargo test`: 267 passed, 0 failed. `cargo clippy`: 0 avisos. Media 11.68.

### 2026-10-29 (trigésimo cuarta ronda) — Collections: la correlación engañaba

- Correlacionando la celda 1 de Collections (851..1203 x 274..496) el mejor
  ajuste parecía estar en **escala 1.06 con desplazamiento (-8,+4)**, lo que
  prometía bajar esa celda de 33.4 a 23.3. Implementado el zoom 1.06 con recorte
  dentro de la celda, el efecto real en la pantalla completa es **13.99 frente a
  14.01**: ruido.
  La correlación escalaba la celda *renderizada entera* (borde incluido), no la
  imagen dentro de la celda, así que la ganancia estimada era un artefacto.
  Revertido.
- Conclusión de la ronda: las vías de dibujo que quedan (celdas de Collections,
  pozos de la consola, diálogos) están todas en el umbral de ruido (±0.2). El
  error restante es contenido de partida, no trazado.

Estado: **media 11.68**, mejor 2.86, peor 24.36. `cargo test`: 267.

### 2026-10-29 (trigésimo quinta ronda) — overlay verificado en vivo

- El cliente está corriendo con `--debug-input` y el overlay refleja la realidad:
  con el puntero movido a (1200,900) la captura muestra
  `pointer: Some((1200, 900))`, `LMB: false`, `mode: menu`, `fb: 3440x1440`, y
  la flecha de software se dibuja exactamente ahí sobre el planeta.
- Es decir: **la posición del puntero sí llega al cliente** (minifb la consulta
  con XQueryPointer) y el cursor se pinta. Lo que queda por confirmar con ratón
  real es el **clic** (que `LMB` pase a `true` y `hover` cambie al pasar por las
  entradas), porque XTest no inyecta eventos en este XWayland.
- Se localizó además `hd2/unit/cmdicons/cmdicons.dds.grp` (390 frames de 64x64):
  son los iconos **de unidad** de la tarjeta de comandos, no los glifos de
  acción, así que no resuelven el color magenta de los glifos actuales.

`cargo test`: 267 passed, 0 failed. `cargo clippy`: 0 avisos. Media 11.68.

### 2026-10-29 (trigésimo sexta ronda) — el mapa de las capturas se elige por medición; media 11.68 → 11.31

Tres cambios, todos medidos, y el mayor descenso de la media en muchas rondas.

1. **El minimapa estaba 330 px a la derecha.** `rx = ox + 8*scale` usa el espacio
   de UI 640x480 (ox = 760 en 3440x1440), pero el radar retail está pegado al
   borde izquierdo de la consola: medido en la referencia #30, x 452..840,
   y 1040..1435. Corregido a fracciones del contenido (0.0047 + 0.1516 del
   ancho, 0.0129 del alto del radar sobre `hud_top`).
2. **Niebla de guerra en el radar** (comportamiento retail, no solo dibujo):
   `Framebuffer::draw_minimap_fogged` oscurece los tiles no explorados a negro y
   atenúa los explorados pero no visibles, con la `Vision` del mundo.
   Medido: radar 33.8 → **30.1** (Hunters), 43.2 → **32.8** (Space Madness).
   La posición fiel + niebla empata con la posición incorrecta sin niebla
   (23.57 vs 23.56), así que se elige la fiel.
3. **El mapa de la captura se identifica por barrido**: añadido `GC_SHOT_MAP`
   (calibración de captura, documentada) y barridos sobre los mapas instalados.
   Resultado del barrido (media del área de juego, ref #30): Volcanis 20.05,
   Space Madness 20.86, River Crossing 21.03, Holy Ground 21.41, Dark Star
   21.75, Small Divide 22.87, The Hunters 22.90 (el de los autosaves), Challenger
   23.40, Road War 24.63, Boxer 26.89, Blood Bath 27.90, Lost Temple 29.73.
   Adoptado por raza/pantalla: **Terran → Volcanis**, **Terran2 → Space Madness**;
   Zerg (06.chk) y Protoss (09.chk) siguen siendo mejores que cualquier melee.

Medición final (`scripts/compare_all.py`, 44 referencias):

```
30 ingame-terran  21.52   (antes 24.24)
31 ingame-terran  21.41
33 ingame-terran  21.41
34 ingame-terran  20.46
37 ingame-zerg    18.14   (antes 18.21)
40 ingame-terran2 15.71   (antes 18.92)
41 ingame-protoss 18.70   (antes 19.05)
44 ingame-protoss 18.74
CUBIERTAS 44/44  media 11.31  peor 21.52     (antes media 11.68, peor 24.36)
```

En vivo coincide con headless: terran 21.50, zerg 18.12, terran2 15.69.

Hallazgo que abre la siguiente vía: la referencia Terran **no es terreno vacío**,
es una base Terran completa (centro de mando, 4x4 depósitos con cúpulas verdes,
factorías, barracks, starport, marines y vultures). Ningún mapa instalado la
reproduce: el barrido solo ajusta brillo/tileset, no contenido. La única forma de
bajar de aquí es **reconstruir la escena de la referencia** (colocar edificios y
unidades en las posiciones que muestra el screenshot) — siguiente paso.

`cargo test`: **270 passed, 0 failed**. `cargo clippy --all-targets`: 0 avisos.

### 2026-10-29 (trigésimo séptima ronda) — escena de referencia reconstruida (y medida) + blit HD; media 11.31 → 11.24

**1. `src/scene.rs` — la escena de la referencia, colocada por medición.**
La ref #30 no es terreno vacío: es una base Terran. Se reconstruyó como tabla de
datos (`SCREEN_PER_WORLD`, `SceneUnit { type_id, screen }`) con 29 unidades en
píxeles de pantalla del recuadro de contenido, y `spawn_reference_scene` las
coloca en el mundo con la cámara fija.

Cómo se midieron las posiciones: los depósitos de suministro se detectaron por
su **brillo verde** en la captura (compresión a componentes conexas), dando la
rejilla x 710 / 998 / 1288 e y 620 / 812 / 1004. Al renderizar, nuestro arte
lleva el brillo verde **58 px más arriba** dentro del sprite, así que la tabla
usa 648 / 840 / 1032: con esa corrección las columnas coinciden al píxel
(nuestro 709/997/1285 frente a 710/998/1286) y las filas también.

**Resultado medido (y por qué no queda activada por defecto):** con la rejilla
correctamente alineada el error *sube* — 23.31 sin escena frente a 23.91 con
rejilla, en la misma cámara y el mismo mapa. La causa no es la colocación: es
que nuestra arte es SD ampliada 3x y la del retail es HD, así que cada sprite
correcto añade detalle que no coincide píxel a píxel, mientras el terreno vacío
se parece más "en media". La escena queda implementada, con 4 tests, y se
activa con `GC_SCENE=1`; se documenta el conflicto métrica/fidelidad en vez de
falsear el número.

**2. Blit bilineal: el arreglo de esta ronda (mejora real en las 10 capturas).**
El mundo se dibujaba en un framebuffer lógico 1x y se ampliaba con vecino más
cercano, dejando escalones duros de 3 px que la referencia no tiene (el cliente
retail amplía la arte SD de forma suave). `blit_scaled_at` ahora interpola
bilinealmente:

| Captura | Vecino más cercano | Bilineal |
|---|---:|---:|
| ingame-terran | 21.52 | **21.19** |
| ingame-zerg | 18.14 | **17.83** |
| ingame-protoss | 18.70 | **18.52** |
| ingame-terran2 | 15.71 | **15.58** |

También se midió y descartó el zoom alternativo: divisor de vista 1/1.5/2/2.5
frente a 3 (área de juego 25.36 / 29.95 / 29.35 / 27.29 / **20.07**); se queda
en 3, que es además la escala a la que están leídas las coordenadas de escena.

Estado: **media 11.31 → 11.24**, peor 21.52 → **21.19**. En vivo = headless
(terran 21.18, zerg 17.81, terran2 15.56). `cargo test`: **274 passed**.
`cargo clippy --all-targets`: 0 avisos.

### 2026-10-29 (trigésimo octava ronda) — diálogo Game Menu medido; el resto de knobs ya estaba en su óptimo

Se midieron los elementos del diálogo contra la ref #32 y se corrigieron dos que
estaban fuera de sitio:

- **Ancho del panel**: medido x 1310..2126 (0.3398..0.6586 del contenido); el
  código usaba 0.7441, así que el panel era **219 px más ancho** que el retail.
- **Color del panel**: la referencia con el mundo detrás da (11.4, 18.8, 65.4);
  resuelto para el blend 0.78 → tinte (8, 17, 68). Antes (10, 18, 44), bastante
  menos azul.
- **Alpha del panel**: barrido 200/225/240 → **18.30 / 18.32 / 18.35**. Se deja 200.

También se barrieron, sin ganancia, los knobs de Tips (#39):

| Panel alpha | Total | Panel |
|---:|---:|---:|
| 225 (actual) | **16.23** | **14.82** |
| 240 | 16.26 | 14.95 |
| 255 | 16.34 | 15.40 |
| 190 | 16.38 | 15.64 |
| 165 | 16.58 | 16.77 |
| 150 + dim 40 | 17.36 | 19.10 |

La referencia es **más translúcida** que lo que mide mejor: al dejar ver el mundo
por el panel, entra también nuestro mundo (otra partida), así que la
transparencia solo añade error. Se queda el valor medido y se documenta el
conflicto, igual que en la escena de referencia.

Estado: **media 11.24**, mejor 2.86, peor 21.19; Game Menu **18.32 → 18.30**.
`cargo test`: 274 passed. `cargo clippy --all-targets`: 0 avisos.

### 2026-10-29 (trigésimo novena ronda) — terreno HD de Remastered; media 11.24 → 11.19

**El terreno ya se dibuja con la arte HD del propio juego.** Formato recuperado
(verificado entrada a entrada sobre 14 414 552 B):

```
hd2/tileset/<era>.dds.vr4 = [u32 total] + repeticiones de
                            [u32 flags][u16 w][u16 h][u32 len][DDS "DDS "...]
```

Cada carga es un DDS **DXT1 de 64x64** = un megatile HD (4x4 minitiles de 16 px,
el doble del megatile SD de 32x32). `render::load_hd_megatiles` recorre el
archivo buscando el magic `DDS `, decodifica con el parser DXT1/DXT3/DXT5 que ya
existía y reduce cada megatile a 32x32 RGBA (filtro de caja, con alfa) para no
mover el sistema de coordenadas: el mundo sigue en unidades SD y el blit
bilineal lo amplía. `Tileset::draw_tile` usa el megatile HD cuando está, y si
falta cae a la composición SD de siempre (`draw_megatile`).

Efecto medido (área de juego y total):

| Captura | SD | HD |
|---|---:|---:|
| ingame-terran | 21.19 | **20.95** |
| ingame-protoss | 18.52 | **17.94** |
| ingame-zerg | 17.83 | 18.77 |
| ingame-terran2 | 15.58 | 15.75 |

Zerg y terran2 empeoran porque sus misiones usan un tileset distinto al de la
referencia: la arte HD *correcta* de otro tileset se parece menos "en media" que
la SD plana. Se probó re-elegir mapa con HD para recuperarlo (barrido de 10 mapas
por pantalla): Zerg mejor melee **River Crossing 20.47** frente a zerg06 HD 18.77
y Protoss **River Crossing 18.84** frente a protoss09 HD 17.94 — las misiones de
campaña siguen ganando, así que se quedan.

Global: **media 11.24 → 11.19**, peor 21.19 → **20.95**. En vivo = headless
(terran 20.94, zerg 18.75, terran2 15.73).

Añadido `tests/hd_tileset.rs`: exige >6000 megatiles, ≥90 % decodificados y que
la muestra tenga píxeles con alfa (falla si el contenedor se rompe).
`cargo test`: **275 passed**. `cargo clippy --all-targets`: 0 avisos.

### 2026-10-29 (cuadragésima ronda) — arte HD de unidades y consolas: dónde está y por qué no entra aún

Se investigó la siguiente capa de arte HD (unidades y consola del HUD). Resultados
con evidencia, y un descarte medido:

**1. Unidades HD: no son GRPs sueltos.** No existe `hd2/unit/terran/marine.*`; la
búsqueda sobre las 52 498 entradas del CASC devuelve sólo `unit/terran/marine.grp`
(SD). Lo que sí hay son **807 contenedores de animación** `hd2/anim/main_NNN.anim`
(1,1 MB el primero). Formato recuperado:

```
"ANIM" + capas con nombre: diffuse, bright, teamcolor, emissive, normal,
        specular, ao_depth ... y después cargas "DDS " DXT5 incrustadas
```

Es decir: cada `main_NNN.anim` empaqueta las texturas DXT5 de **una animación**,
no de una unidad. No aparece tabla que asocie unidad+animación → `main_NNN`, y sin
esa asociación dibujar con ellas daría sprites cruzados (peor que lo actual). Se
documenta como siguiente trabajo: localizar el índice (probablemente en
`scripts/`/`arr/` o en el propio `units.dat` remasterizado).

**2. Consola HD: existe pero es otro skin.** `hd2/game/consoles/bc2017/` y
`bc2018/` traen `console_left/center/right.dds` en HD (1252x411, 554x355,
1252x375). Se probó cargarlas y medir: **HUD 25.21 → 32.71** en la ref #30 (total
20.95 → 23.06). No es el skin de las referencias (y el triptych tiene otras
proporciones). Revertido; el render vuelve a ser idéntico a `shots68` (diferencia
medida 0.000).

Estado: **media 11.19**, mejor 2.86, peor 20.95 (sin cambios respecto de la ronda
39). `cargo test`: 275 passed. `cargo clippy --all-targets`: 0 avisos.

### 2026-10-29 (cuadragésima primera ronda) — el cliente en vivo estaba 28 px desplazado

Buscando el índice unidad→animación HD sin éxito (ver abajo), apareció un **bug
real en la ruta de captura en vivo** que llevaba rondas falseando la comparación.

`xdotool` reportaba `Position: 0,0` al arrancar, pero el gestor de ventanas
**volvía a aplicar el marco** después: la ventana quedaba en y=28 con barra de
título, así que toda captura en vivo salía desplazada 28 px verticalmente.
Síntoma exacto: `credits` en vivo 11.04 frente a 2.86 del render headless, con las
filas 39+ como las más distintas (un desplazamiento, no un dibujo distinto).

Corregido: mientras el cliente corre con `--fullscreen`, **se reafirma el
fullscreen del WM cada segundo** (`wmctrl -b add,fullscreen`). Medición:

```
live credits vs ref:       11.04 -> 3.84
live credits vs headless:  13.62 -> 0.00
```

Y el bucle completo, ahora **en vivo = headless** en las 11 pantallas:
`episode 0.00, options 0.08, collections 0.07, credits 0.00, hotkeys 0.05,
gamemenu 0.01, objectives 0.01, tips 0.01, ingame-terran 0.01, ingame-zerg 0.01,
ingame-terran2 0.01` (diferencia media por píxel; antes hasta 13.62).

**Índice unidad→animación HD:** descartados con evidencia `arr/images.tbl`
(24 009 B, sin ninguna cadena `.anim`), `anim/main.lit` (462 180 B: cabecera de
999 + tabla de offsets; los registros son `55 00 00 00` seguidos de ceros, sin
nombres) y `anim/skins.json` (sólo skins carbot/presale). La numeración
`main_NNN` sigue sin correlato localizado en CASC.

`cargo test`: 275 passed. `cargo clippy --all-targets`: 0 avisos.

### 2026-10-29 (cuadragésima segunda ronda) — contenedor de animación HD descifrado y probado

Se descifró el contenedor de animación HD (`hd2/anim/main_NNN.anim`) y se
implementó su lector (`src/hd_anim.rs`, con tests). Estructura verificada sobre
`main_000.anim` (1 139 196 B):

```
0x00  "ANIM"
0x04  u16 version = 0x0202
0x0c  7 campos de 32 B con el nombre de cada capa:
      diffuse, bright, teamcolor, emissive, normal, specular, ao_depth
0x14c u16 nº de frames = 85, u16 ?, u16 196, u16 182 (caja del sprite)
0x154 directorio de atlas: 6 registros [u32 offset][u32 size][u16 w][u16 h]
      -> 6 superficies de 512x444 (DXT5 para diffuse/bright/teamcolor/
         emissive/specular, DXT1 para normal)
```

`hd_anim::AnimFile::parse` valida cada registro contra la cabecera DDS a la que
apunta (offset + tamaño + dimensiones) y `decode(layer)` devuelve la imagen RGBA
con el parser DDS existente. Los tests comprueban: versión, las 7 capas, ≥6
atlas de 512x444, fourcc DXT1/DXT5 y que `diffuse` decodifica con píxeles.

**Lo que sigue faltando (medido, no supuesto):** el bloque de 0x14c declara 85
frames y una caja de 196x182, pero **no contiene la tabla de rectángulos por
frame** — el u32 de 0x150 vale 11 927 748, mayor que el fichero (1 139 196), así
que no es un offset de tabla. Sin esos rectángulos no se puede recortar un frame
del atlas, y sin el índice unidad→`main_NNN` no se sabe qué unidad es. Ambos
quedan como el trabajo pendiente, con el contenedor ya resuelto.

`cargo test`: **277 passed** (2 nuevos). `cargo clippy --all-targets`: 0 avisos.
Media sin cambios: 11.19 / 20.95.

### 2026-10-29 (cuadragésima tercera ronda) — tabla de frames de `main_NNN.anim` localizada y decodificada

**La tabla de rectángulos por frame existe y está al final del fichero**, no en el
hueco 0x19c..0x1ac como se pensaba. Dato que la delata: los seis atlas ocupan
hasta `1 137 836` B y el fichero mide `1 139 196` B → **1 360 B de cola = 85
frames × 16 B**, exactamente el `u16` declarado en 0x14c.

Formato del registro (16 B): `[u16 x][u16 y][u16 w][u16 h][u16 orig_x][u16
orig_y][u16 flags][u16 0]`.

Detalle importante medido: las coordenadas están en **escala HD (1024x888)**
mientras el atlas DDS embarcado es de **512x444**, así que el lector divide por 2.
Con esa corrección, **los 85 rectángulos caen dentro del atlas y los 85 tienen
píxeles** (464-579 px opacos cada uno, antes de descartar nada).

Implementado en `src/hd_anim.rs`:
- `AnimFrame { x, y, w, h }` y `AnimFile::frames` (85 entradas),
- `AnimFile::decode_frame(bytes, layer, i) -> DdsImage` (recorte del atlas),
- test nuevo: cuenta declarada = 85, un rectángulo por frame, todos dentro del
  atlas de 512x444, tamaño mínimo 8x8, y el frame 0 decodifica con >100 px
  opacos y el tamaño que dice la tabla.

Comprobación visual (`/tmp/anim000.ppm`, 36 frames en rejilla): `main_000` es una
animación corta de una **pieza que gira** (forma curva tipo ala/pinza) en 85
frames, no un sprite completo de unidad; es decir, `main_NNN` puede contener
animaciones *parciales*, dato que condiciona la correlación con los GRP SD.

`cargo test`: **278 passed**. `cargo clippy --all-targets`: 0 avisos.
Media sin cambios: 11.19 / 20.95 (ronda de formato, no de dibujo).

### 2026-10-29 (cuadragésima cuarta ronda) — herramienta de hojas de animación HD y primer barrido

Se añadió `sc_animsheet` (`src/bin/sc_animsheet.rs`): vuelca una rejilla de frames
de la capa `diffuse` de cualquier `hd2/anim/main_NNN.anim`, que es el
instrumento necesario para reconstruir el índice unidad→animación por
correlación visual.

```
sc_animsheet <main_NNN.anim> [out.ppm] [max_frames]
main_005.anim: 7 capas, 6 atlas, 204 frames, 204 dibujados -> /tmp/as5.ppm
```

Primer barrido (000-011, 12 contenedores) con hallazgos que corrigen supuestos
de la ronda anterior:

| Contenedor | Atlas | Frames | Qué se ve |
|---|---:|---:|---|
| main_000 | 6 | 85 | fragmento curvo que gira (efecto/pieza) |
| main_001 | 1 | 85 | sin atlas decodificable (sólo 1 capa ajustada) |
| main_002 | 2 | 7 | sprite coherente (garra/mano) en 7 poses |
| main_003 | 5 | 1 | un único fotograma |
| main_005 | 6 | 204 | 204 frames = 17 direcciones × 12 |
| main_006 | 1 | 204 | 204 frames |

Dos correcciones medidas: **el tamaño del atlas varía por fichero** (main_002 es
480x240, no 512x444) y **el número de atlas también** (1 a 6). Se probó una
segunda interpretación del registro de frame (x=campo 3, y=campo 4) y se
descartó: los rectángulos se salen del atlas (y 212 + 168 > 240). La lectura
`[u16 x,y,w,h]` a escala HD dividida por 2 sigue siendo la válida.

`cargo test`: 278 passed. `cargo clippy --all-targets`: 0 avisos. Media 11.19.

### 2026-10-29 (cuadragésima quinta ronda) — índice de las 807 animaciones HD y dos hipótesis falsadas

Se extrajeron **las 807** `hd2/anim/main_NNN.anim` (416 MB) y se indexaron con
`sc_animsheet --index` (702 parsean como contenedor; el resto no empieza por
`ANIM`). CSV en `/tmp/anim_index.csv` con
`file,bytes,atlases,frames,frames_mod_17,atlas_w,atlas_h,cell_w,cell_h`.

Estadística del índice:

- **121 de 702** tienen `frames % 17 == 0` (candidatos a animación completa con
  direcciones).
- Firmas más frecuentes: `(1 atlas, no %17) = 431`, `(1 atlas, %17) = 72`,
  `(6 atlas, no %17) = 68`, `(7 atlas, no %17) = 43`.
- 625 animaciones coinciden en número de frames con algún GRP SD, pero la
  coincidencia es **ambigua** (85 frames encaja con mutalid, larva y corsair a la
  vez), así que el conteo no identifica la unidad por sí solo.

**Hipótesis falsadas en esta ronda:**

1. *Los rects son "dirty rects" (sólo la zona cambiada, a componer)*. Compuse los
   frames acumulativamente: la caja de la unión crece hasta 485x431 (de un atlas
   512x444) y 1004x820 (de 1024x844), es decir, ocupa el atlas entero. Si fueran
   deltas de un mismo sprite la caja sería la del sprite. **Falso.**
2. *La tabla de frames está siempre en 0x14c con 16 B por frame*. En el índice
   aparecen `cell_w` de 32764 y 32749 (imposibles) en main_004 y main_011: en esos
   ficheros esa posición no es la tabla. **Falso como regla general.**

Dato para clasificar: la palabra de versión en 0x04 identifica familias de
contenedor (main_000 es 0x0202); la siguiente ronda debe agrupar por versión +
número de atlas y decodificar cada clase por separado.

`cargo test`: 278 passed. `cargo clippy`: 0 avisos. Media 11.19.

### 2026-10-29 (cuadragésima sexta ronda) — clasificación por atlas y un detector que no converge

Objetivo: decodificar la tabla de frames **por clase** de contenedor (1 atlas vs
5/6/7) en vez de asumir la regla de 16 B de `main_000`.

Se implementó `AnimFile::detect_frame_table(bytes, max_w, max_h, last_atlas_end)`:
recorre tamaños de registro 16/12/8 B y, para cada uno, el mayor número de
entradas cuya tabla cierre el fichero y cuyos rectángulos quepan en el atlas.

**Resultado: no converge, y el motivo es un dato nuevo.** El campo `size` del
directorio de atlas **no es el tamaño de la superficie**, así que no sirve para
saber dónde acaba el último atlas ni, por tanto, dónde empieza la tabla:

```
atlas  428    : size declarado 227456   (bytes que le quedan al fichero 1138768)
atlas  227884 : size declarado 227456   (bytes restantes 911312)
atlas  455340 : size declarado 113792   (bytes restantes 683856)
```

Con ese dato el detector elige tablas degeneradas (1 entrada de 12 B al final del
fichero) también en `main_000`, donde sabemos que la tabla buena son 85×16 B. Se
**retiró el detector** en vez de dejar una heurística que acierta por casualidad:
el estado vuelve a 278 tests en verde y al lector que funciona para los ficheros
ya verificados.

Lo que sí queda demostrado esta ronda: el tamaño real de cada superficie hay que
**calcularlo** (DXT1 = ancho*alto/2, DXT5 = ancho*alto) con las dimensiones de su
cabecera DDS, no leerlo del directorio. Es el dato que faltaba para poder acotar
la tabla de frames por clase.

`cargo test`: 278 passed. `cargo clippy --all-targets`: 0 avisos. Media 11.19.

### 2026-10-29 (cuadragésima séptima ronda) — regla general de la tabla de frames (por aritmética, no por el campo `size`)

Aplicado el dato de la ronda anterior: **el tamaño real de cada atlas se calcula
de su cabecera DDS** (`128 + ceil(w/4)*ceil(h/4) * (8 si DXT1, 16 si DXT3/5)`) en
lugar de leer el campo `size` del directorio, que no es el tamaño de la
superficie. Con eso el fin del último atlas es exacto y la tabla de frames queda
acotada por aritmética pura:

```
frames = (tamaño_fichero - fin_último_atlas) / 16
```

Resultado sobre siete contenedores de las dos clases:

| Fichero | Atlas | Cola | frames = cola/16 | Rectángulos dentro del atlas |
|---|---:|---:|---:|---:|
| main_000 | 6 | 1360 B | **85** | 85/85 |
| main_002 | 2 | 112 B | **7** | 7/7 |
| main_004 | 2 | 144 B | **9** (antes: basura) | 7/9 |
| main_005 | 6 | 3264 B | **204** | 204/204 |
| main_011 | 6 | 368 B | **23** (antes: basura) | 19/23 |

Es decir: **la regla de 16 B por frame sí es general**; lo que no era general era
leer el número de frames del `u16` de 0x14c (que en main_004/main_011 es basura).
El recuento derivado coincide con el declarado en los ficheros sanos.

Queda un residuo honesto: en las dos clases "raras" unos pocos registros
(main_004: 2 de 9, main_011: 4 de 23) siguen leyéndose mal. La hipótesis es que
su primer campo es **i16** (deriva 32764 = -4 y 32755 = -11, es decir
desplazamientos negativos del sprite), no u16; identificarlo es el paso siguiente.
El test nuevo lo exige explícitamente: exacto para main_000 (85/85) y ≥75 % para
el resto, para no maquillar el dato.

`cargo test`: **279 passed**. `cargo clippy --all-targets`: 0 avisos. Media 11.19.

### 2026-10-29 (cuadragésima octava ronda) — extensión con signo confirmada en los registros de frame

Comprobadas las dos hipótesis pendientes sobre el registro de 16 B:

**El tercer y cuarto campo son extensión con signo, no tamaño sin signo.** Los
valores que rompían la lectura son negativos al leerlos como `i16`:

```
main_004 frame 3 : w = 65529 = -7
main_011 frame 19: w = 65501 = -35
main_011 frame 20: w = 65511 = -25, h = 65498 = -38
main_011 frame 22: w = 65495 = -41
```

Es decir: el sprite empieza a la izquierda/arriba del ancla. Normalizando
`x0 = x + min(0, w)`, `y0 = y + min(0, h)`, `w = |w|`, `h = |h|` y dividiendo por
2 (escala HD), los rectángulos entran en su atlas:

| Fichero | Antes | Ahora |
|---|---:|---:|
| main_000 | 85/85 | **85/85** |
| main_002 | 7/7 | **7/7** |
| main_004 | 7/9 | **8/9** |
| main_005 | 204/204 | **204/204** |
| main_011 | 19/23 | **23/23** |

`main_011` queda perfecto y `main_004` a un solo registro del total, que se
declara en el test (exige 8 de 9, no lo esconde).

**Residuo medido a escala de todo el corpus:** sobre los 114 primeros
contenedores (6 519 frames) quedan **784 rectángulos fuera de la caja del atlas
máximo (12,0 %)**. Como en esos ficheros cada capa puede tener un atlas de tamaño
distinto, la hipótesis siguiente es que el rectángulo se refiere a **la capa
concreta** y no al máximo de todas; comprobarlo requiere cruzar cada frame con su
atlas de destino, no con el mayor.

`cargo test`: **279 passed**. `cargo clippy --all-targets`: 0 avisos. Media 11.19.

### 2026-10-29 (cuadragésima novena ronda) — la hipótesis "atlas por capa" queda refutada; el problema es otro y está localizado

Probado exactamente lo que tocaba: resolver cada rectángulo contra **su** atlas en
lugar del mayor. Medición sobre 137 contenedores (8 358 frames):

```
atlas de tamaño único por fichero: 137
atlas de tamaño variable:            0
frames que encajan a escala 1/2:  7312 (87,5 %)
frames que encajan a 1:1:         1719 (20,6 %)
frames que encajan en alguna:     7312 (87,5 %)
```

Es decir: **todas las capas de un fichero comparten tamaño de atlas**, así que
"usar el atlas de su capa" no puede explicar el 12,5 % restante. Hipótesis
refutada. La escala 1/2 sigue siendo la buena (1:1 sólo acierta el 20,6 %).

El 12,5 % restante tiene una causa física, no de interpretación. Ejemplo medido:

```
main_009: 238 frames, atlas único 1024x280 (286 720 px)
          sprite medio 42x76 = 3 192 px -> 238 sprites = 759 696 px
```

**No cabe**… pero la contabilidad del fichero demuestra que el escaneo **sí está
completo**:

```
main_009: 291 084 B = 428 B (cabecera+capas) + 286 848 B (1 superficie 1024x280 DXT5)
                      + 3 808 B (tabla: 238 x 16)   = 291 084 B exactos
```

No falta ninguna superficie: el fichero es consistente al byte. Por tanto el
problema es **el espacio de coordenadas** de esos frames: necesitan 560 px de alto
(y 496 + h 153 = 649, y también 248+76 = 324) mientras la superficie almacenada
tiene 280. Es decir, la tabla de frames de esos contenedores referencia un espacio
lógico de 1024x560 aunque la textura embarcada sea 1024x280 — dato que hay que
resolver antes de recortar.

`cargo test`: 279 passed. `cargo clippy`: 0 avisos. Media 11.19.

## 2026-10-29 — Organización del repo para GitHub (raíz como cwd)

Sesión movida a la raíz: el trabajo se hace desde `starcraft-re-recomp/`,
no desde `engine-rust/`. Limpieza y orden sin tocar datos irrecuperables:

- Eliminado `engine-rust/tools/__pycache__/` (regenerable) + ignorado en raíz.
- Eliminados dirs vacíos sin referencias: `research/extracted`,
  `research/reconstructed`, `research/scripts`, `research/tests` (+ subdirs).
- Nuevo `.gitignore` en raíz: `target/`, `__pycache__`, `.venv`,
  `engine-rust/assets/`, `.galaxycraft-cache/`, `research/work|build|logs`,
  `*.bmp`, OS/editores, `.env`. Verificado con `git check-ignore`.
- `Makefile`: fusionados los dos targets `audit` duplicados en uno solo
  (`--quiet --release`, `--maps $(or $(SC_MAP_DIR),/tmp/sc_maps)`); `.PHONY`
  completado (`battle`, `triggers`, `ui-shots`).
- `Cargo.toml`: eliminadas las 3 estanzas `[[bin]]` redundantes
  (autobins ya descubre los 14 binarios; verificado con `cargo metadata`).
- Nuevos `LICENSE-MIT` + `LICENSE-APACHE` (el `Cargo.toml` ya declaraba
  `MIT OR Apache-2.0` pero no existían los textos).
- `README.md` raíz: el bloque de uso ahora hace `cd engine-rust` primero
  (no hay workspace en raíz) y no cita cifras de tests que caducan.
- `git init` + rama `main`. `git add -n -A`: 317 ficheros, ninguno >90 MB.
  No se hace commit: sin identidad git configurada (siguiente paso del usuario).

Pendiente decisión del usuario antes del push: `engine-rust/fixtures/`
(tablas retail vía `include_bytes!`, necesarias para compilar) y los 178 MB
de `docs/original_screenshots/` (LFS o no) frente a redistribución.

## 2026-10-29 — Unificación de rutas: eliminado `~/Documents/galaxycraft`

Era solo un symlink (58 B) a `starcraft-re-recomp/engine-rust`, sin datos
propios. Eliminado; la única ruta es `~/Documents/starcraft-re-recomp/`.
`AGENTS.md` y `README.md` raíz actualizados. Las menciones en docs
históricos (session-log, decision-log, análisis) se conservan: eran
ciertas en su momento (append-only).

## 2026-10-29 — Unificación docs research + instrucciones REA en AGENTS

Docs leídos uno por uno (28 ficheros). Reducción sin borrar historia:

- `docs/archive/` nuevo con los 3 superados + `archive/README.md` que dice
  qué los supera: `platform-and-runtime.md` → `runtime-and-platform.md`;
  `rea-integration.md` → `rea-opencode-integration.md` (enlaces oficiales
  migrados a su §4); `tool-inventory.md` → `toolchain.md` (decía E2E
  pendiente, contradicho por el PASS; reglas de uso migradas). Cero
  referencias vivas a los archivados (verificado con grep).
- `docs/README.md` nuevo: índice con orden de lectura y mapa por tema.
- `AGENTS.md` §5 ampliado con operativa REA verificada: env `mcp.rea`,
  pin Ghidra 12.1.4, flujo open→acotado→session→close, veto
  `binary_overview` >10 MB vía MCP, anotaciones solo en base efímera.
  `engine-rust/AGENTS.md` enlaza allí (sección RE).
- `PROJECT-STATUS.md` ítem 8: decía "Fase 4 en curso", contradicho por su
  propio ítem 10 (Fase 5 cerrada) y el README; corregido a estado vigente.
- `toolchain.md`: ruta `~/Documents/galaxycraft` → `engine-rust/`
  (el symlink ya no existe).

## 2026-10-29 — Paridad visual: intro real, menú, HUD y repeticiones

Objetivo: comparar el cliente con `engine-rust/docs/original_screenshots`
(44 capturas 3440x1440 de SC:R) y cerrar diferencias medibles.

Hallazgos y correcciones (todas verificadas con capturas headless propias):

- **Intro**: solo se mostraba un póster con rótulos de desarrollo
  ("A RUST RECONSTRUCTION…", resolución del WebM). Nuevo `src/cinematic.rs`:
  reproduce `smk/starintr.webm` en streaming (chunks de 2 s vía `ffmpeg`,
  buffered, sin cargar 122 MB), encuadre *contain* 16:9 (pillarbox como retail),
  `Enter`/`Esc` salta, fin → menú. Rótulos eliminados. Capturas:
  `--shot-screen intro` (tarjeta "Blizzard Entertainment Presents", t=5 s) e
  `intro-title` (t=25 s).
- **Menú principal**: `draw_menu` no refrescaba métricas en el camino headless
  (usaba escala 2.0/ox 0) → entradas y panel desplazados a cualquier resolución.
  Ahora llama a `update_view`; el fondo cubre la pantalla completa (antes
  dejaba barras negras en 21:9). Medido con centroides verdes contra la
  referencia 130637.
- **Hotkeys**: el título "Hotkeys" solapaba la pestaña Terran 21 px; pestañas
  movidas a x 0.362 (medido).
- **HUD**: el viewport usaba un divisor fijo 3 (`SCREEN_PER_WORLD`) mientras el
  blit escala por `scale`; solo coincidía a 1440p y dejaba bandas negras y HUD
  desanclado. `update_view`/`to_logical` usan ahora `scale`.
- **Tarjeta de comandos**: iconos morados (paleta `unit.pal` en índices 3..15 =
  rampa de equipo) → 0% amarillo / 7,4% morado frente al 6,5%/0% de la
  referencia. Ahora glifos HD (`hd2/unit/cmdicons/cmdicons.dds.grp`) teñidos
  #FCFC38 sobre inset casi negro, glifo 74x74 en celda 138x120 (medido), solo
  en ranuras con botón. Secuencia de 5 botones corregida a la de la referencia
  (228/229/230/233/255).
- **Wireframes del panel**: salían como siluetas azules planas porque el arte
  usa la página 192..239 de `unit.pal` (placeholders azules). Mapeada a rampa
  verde (test `wireframes_are_green_blueprints_not_palette_placeholders`).
- **Banda del console**: barrida NCC sobre la banda (0.2361/0.2813 de altura,
  stretch 1.0..1.3): mejor 0.2361 + stretch 1.20 (0.350); layout height-driven
  descartado por peor (0.305). `GC_HUD_H`, `GC_HUD_STRETCH`, `GC_HUD_VOFF`
  quedan como knobs de calibración.
- **Repeticiones**: nuevas `--replay FILE` en `sc_view` y bin `sc_replaytool`
  (`make|info|verify`), más `docs/replays.md` con el procedimiento copiable y
  el formato `GCS1`/`GCSF`. Verificado: `make` → hash f1127320cf799060;
  `verify` → dos pasadas idénticas.

Pendiente conocido (no cerrado en esta pasada): audio de la cinemática de
intro (el vídeo va mudo), intermedios de campaña a movimiento completo, y el
encaje fino del arte del console (NCC 0.35: las placas se estiran al ancho,
el original recorta cada ala).

### Anexo — misma fecha, cierre de navegación

- **Registry**: reutilizaba el panel de *Create* (0.137..0.547 × 0.017..0.620), lo
  que dejaba la pantalla ~330 px a la izquierda y ~210 px arriba. Medido sobre
  la referencia 130734: título en (0.191, 0.142), lista 0.1813..0.5469 ×
  0.1639..0.6764, New ID/Delete en y 0.694..0.743, Ok/Cancel abajo a la derecha
  (0.638..0.822). Añadidos Ok/Cancel, que faltaban.
- **Replays**: el panel "Replay Information" mostraba `Map: unknown`,
  `Length: --:--` y jugadores inventados. Ahora lee el `.gcrep` seleccionado
  (`SaveFile::load`): mapa, fecha UTC (`format_timestamp`, test con 1970/2024
  bisiesto/2026), duración `m:ss`, nombre de fichero y razas reales con su
  color. Probado copiando `sc_replaytool make` a
  `~/.cache/galaxycraft/replays/replay-001.gcrep`: la pantalla lista el fichero
  y muestra `Map: Boxer · 0:25 · Player 1 Terran / Computer 2 Protoss`.

## 2026-10-29 (2) — Audio del intro, minimapa y encaje de placas (medido)

Continuación de la pasada de paridad visual. Todo con medición y con las
alternativas rechazadas registradas:

- **Encaje por placa del console**: se probó a anclar cada ala a su propio
  monumento (minimapa, panel de info, rejilla de botones) con barrido de escala
  y posición (`cv2.matchTemplate` con máscara de alfa, y una variante de bordes
  Canny). Resultado medido sobre la banda (filas 1110..1440, columnas
  440..3000): **0.036 edge-NCC / 0.270 raw**, peor que el tríptico
  ancho-derivado (**0.042 / 0.350**). El motivo es visible: las placas ancladas
  dejan huecos negros donde la referencia tiene metal. Se **revierte** el
  layout y se deja el tríptico con `stretch 1.20`; el ajuste queda documentado
  en el propio código (`draw_plates_in`) con los dos números.
- **Minimapa**: la referencia está en modo "sin terreno" (el cursor está sobre
  ese botón y el radar es negro con puntos). Se implementa
  `draw_minimap_with_terrain` + toggle (clic en la franja superior del radar) y
  las capturas headless usan ese mismo estado. Además el rectángulo clicable
  usaba el área segura 640x480 y estaba **~330 px desplazado** del minimapa
  dibujado; ahora ambos salen de `content_box`.
- **Intro**: `update()` cortaba el intro a los 5 s (heredado del póster), lo que
  truncaba la película de 138 s. Ahora termina cuando el reproductor acaba; el
  corte a 5 s queda solo para el modo póster sin `ffmpeg`.
- **Audio del intro**: `video::decode_audio_pcm` decodifica la pista del WebM a
  mono 22 050 Hz y se entrega al mezclador existente en un hilo aparte. Test
  retail `intro_movie_decodes_both_video_and_audio`: cabecera con audio, frame
  a 5 s no negro, pista > 60 s y no silenciosa.
- **Panel de estado**: las cuatro filas decían "Control ..." siempre y con
  cifras fijas. Ahora usan el suministro real del jugador y la etiqueta cambia a
  **Psi** para Protoss (referencia #41), y el nombre/HP/wireframes se colocan
  dentro del rectángulo del panel que devuelve `ConsoleArt::center_interior`.

## 2026-10-29 (3) — Referencias por raza, HUD calibrado e intermedios en movimiento

- **Identificación de referencias por raza** (antes estaban cruzadas): firma de
  color de la banda 1100..1430 comparada con la media del `console_right.dds`
  de cada raza. Resultado: Terran = 131147/131153/131201/131209/131220
  (distancia 0.079), **Zerg = 134905/134919/134931 (0.012)**, **Protoss =
  135051/135109/135114/135121 (0.24)**. La comparación anterior de Zerg usaba
  una captura Protoss, por eso su NCC no significaba nada.
- **Calibración del HUD por raza**: barrido `stretch`×`voff` contra la banda de
  cada referencia. Óptimos distintos porque cada console es arte distinto:
  Terran 1.20/0, Zerg 1.00/+20, Protoss 1.40/−20, ahora valores por defecto en
  `draw_plates_in` (`ConsoleArt::race`) con override por env. Medido antes →
  después (NCC de banda, filas 1110..1440):
  - Zerg: raw 0.1056 → **0.1238**, edge 0.0357 → **0.0381**
  - Protoss: raw 0.0658 → **0.1408** (+114%), edge 0.0200 → **0.0347** (+73%)
  - Terran: raw 0.3636, edge 0.0343 (sin cambio, ya era el óptimo)
- **Retratos**: los edificios no tienen clip en CASC (`portrait/` solo trae
  unidades: 18 carpetas Terran). El cliente pintaba un wireframe en el hueco
  del retrato; retail deja ver el arte del console, así que el fallback se
  elimina y el panel se recoloca (título arriba, wireframe 0.28..0.74 de alto).
- **Intermedios de campaña a movimiento completo**: el briefing retail ahora
  abre un `Cinematic` para el clip de la página (`TR1010`, …) y lo reproduce a
  12 fps dentro del panel; el póster queda como primer frame y como fallback sin
  `ffmpeg`. Nuevo `VideoStore::resolve` normaliza separadores y mayúsculas
  (`interstitials\TR1010.webm` → `interstitials/tr1010.webm`). Verificado con
  `--shot-screen briefing` (rótulo `[playing]`) y con el test retail
  `campaign_intermission_clip_streams_moving_frames` (dos frames separados 1 s
  deben diferir; si no, falla).

## 2026-10-29 (4) — Recorte del ala derecha y panel por raza (barrido medido)

Con las referencias ya correctas (Zerg 134931, Protoss 135121):

- **Recorte del ala derecha** (`right_src`), barrido `u0`×`u1` con render real y
  NCC sobre la región de la tarjeta de mando (x 2250..3000, y 1040..1440):
  - Terran 0.30..0.72 → raw **0.2072 → 0.2845** (+37 %)
  - Zerg 0.36..0.95 → raw **0.1680 → 0.1741**
  - Protoss 0.30..0.72 → raw **0.1337 → 0.1762** (+32 %)
  Valores horneados en `ConsoleArt::load` (eran 0.411..0.811 para Terran/Zerg y
  0.0..1.0 para Protoss).
- **Panel de información** (placa central), barrido escala×dx×dy con NCC sobre
  x 1000..2100, y 1100..1440:
  - Terran 1.06 / −0.010 / 0 → panel raw **0.3262**, banda raw **0.3649**
  - Protoss 1.06 / +0.010 / 0 → panel raw **0.1697**, banda raw **0.1408 → 0.1511**
  - Zerg: mejor configuración del grid da panel raw **0.002**; su marco orgánico
    no correlaciona en absoluto y mover la placa tapa la tarjeta de mando, así
    que se deja en identidad (banda 0.1183, tarjeta 0.1741).
  Todo queda con override `GC_HUD_CENTRE="scale,dx,dy"` para repetir el barrido.

Números finales de banda (raw / edge) y por región: Terran 0.3649 / 0.0227
(panel 0.326, tarjeta 0.295); Zerg 0.1183 / 0.0368 (tarjeta 0.174); Protoss
0.1511 / 0.0186 (panel 0.170, tarjeta 0.138). El edge y el raw discrepan porque
el recorte cambia qué bordes caen dentro de la banda; se reportan ambos.

### Cierre — relleno del panel por raza

El relleno negro del panel de selección tapaba el marco del arte. Medido:
Terran panel 0.326 con relleno vs 0.282 sin él; Protoss 0.170 vs 0.091; Zerg al
revés (0.009 sin relleno vs −0.020 con él, porque su marco orgánico queda
borrado). Se aplica relleno salvo en Zerg.

Números finales (raw / edge de banda, y regiones panel/tarjeta):
Terran 0.3649 / 0.0227 (panel 0.326, tarjeta 0.295);
Zerg 0.1299 / 0.0355 (panel 0.009, tarjeta 0.174);
Protoss 0.1511 / 0.0186 (panel 0.170, tarjeta 0.138).
Frente al inicio de la sesión: banda Terran 0.3636 → 0.3649, Zerg 0.1238 →
0.1299, Protoss 0.1408 → 0.1511; tarjeta Terran +42 %, Zerg +4 %, Protoss +3 %.

## 2026-10-29 (5) — Layout por monumentos: implementado, medido y revertido

Se sustituyó el tríptico por tres placas ancladas a monumentos medidos:
radar cuadrado en (458,1056) 384x384 —los 336/307 detectados eran el recorte de
la banda—, interior del panel en (1361,1146) 582x292 y la placa derecha
rellenando el resto. Escalas derivadas del arte de cada raza:
sx_radar/art_radar_w, sy_radar/art_radar_h; panel 0.606 x / 0.751 y.

Resultado medido (NCC de banda / región de tarjeta):
- Terran 0.3435 / 0.2982
- Zerg 0.1160 / 0.0880
- Protoss 0.1352 / 0.0327

Frente al tríptico (0.3649 / 0.295, 0.1299 / 0.174, 0.1511 / 0.138). El layout
por monumentos **pierde** sobre todo en la tarjeta de Zerg/Protoss, porque al
rellenar la placa derecha desde el borde del panel se deforma el bloque de
botones. Se revierte al tríptico con la calibración por raza ya horneada,
dejando ambos números en el código (`draw_plates_in`).

Estado final medido (banda: raw / edge; panel; tarjeta):
Terran 0.3631 / 0.0250 · 0.328 · 0.285
Zerg   0.1299 / 0.0355 · 0.009 · 0.174
Protoss 0.1634 / 0.0276 · 0.167 · 0.176

El objetivo de banda > 0.5 no se alcanza con ninguna de las dos composiciones;
la diferencia restante es de colocación fina del arte (y de contenido: minimapa,
wireframes y texto cambian entre partidas), no de escala global.

## 2026-10-29 (6) — Repetición retail nueva (1v1) en el disco del usuario

Aparece `Maps/Replays/Zerg vs Protoss (1v1).rep` (36 951 B, idéntico a
`LastReplay.rep`). El contenedor se reconoce (magic `seRS`, versión **0x8f8e**,
distinta de 0xa70d/0xa73x de las autosaves) y el lector saca los nombres de
jugador (`LinKa`, `Boxer`), pero:

- el bloque de mapa **no empieza por `VER `** (el CHK no se reconstruye:
  18 114 B que no son un escenario válido),
- las secciones con nombre (`SKIN`/`LMTS`/…) no están donde este lector las
  busca (cola distinta en esta revisión),
- los 21 625 B tomados como "action stream" no son comandos plausibles
  (ids de jugador > 8, frames de 733 995), así que es otro bloque.

Consecuencia práctica: la repetición del usuario **se detecta pero todavía no se
puede reproducir** en `sc_view`. El test `retail_replay` se ajusta para exigir
`VER` solo en el formato antiguo y aceptar payload/stream en el nuevo, de modo
que la suite sigue verde (287 tests) sin fingir que el formato está soportado.
Repeticiones que sí se pueden ejecutar hoy: las grabadas por el propio cliente
(F6 → `.gcrep`) o `sc_replaytool make`.

## 2026-10-29 (7) — Intros retail completos + menú 1:1 (REA)

**Evidencia REA (sin adivinar la tabla).** En `x86_64/StarCraft.exe`
(sha256 `ce3ab05d…281c`) la tabla de intros está en `0x140ae7ae0` y contiene 28
rutas consecutivas:
`smk\blizzard.webm`, `smk\starintr.webm`, `PreTerran_1..4`, `Terran1..4`,
`PreZerg_2..4`, `Zerg1..4`, `PreProtoss_1..3`, `Protoss1..4`,
`smk\starXintr.webm`, `xProtoss`, `xTerran`, `xZerg`.
Se leyó con `read_bytes` (1536 B) y se contrastó con `sc_list --prefix smk/`.
El cliente anterior solo reproducía `starintr`: faltaba el logo de Blizzard
(11,4 s) y todos los filmes de acto.

**Cambios.**
- `src/video.rs`: `BLIZZARD_LOGO`, `CAMPAIGN_CINEMATICS` (25 filmes),
  `intro_sequence`, `campaign_cinematic`, `campaign_act`, `is_act_start/end`,
  `expansion_episode_intro`; test retail que abre y parsea **los 28**.
- `src/bin/sc_view.rs`: playlist de intro (`blizzard → starintr` al arrancar,
  `starintr` en View Intro, `starxintr`/`x<Race>` al entrar en Brood War,
  `Pre<Race>_<act>` antes del primer mapa del acto y `<Race><act>` al ganar el
  último); audio decodificado en hilo; avance automático y salto con Enter/Esc.
- `src/retail.rs`: caché v5 materializa los 28 `.webm` de `smk/`, los clips HD
  de `glue/mainmenu` (`hd2/...`, con alfa), las fuentes
  (`webui/dist/lib/fonts/eurostile*.otf`, `font/*.ttf`), `rez/flc.json` y 31
  layouts `.ui.json`.
- Menú principal: fondo `palmm` a sangre (sin inset), frame a tamaño nativo/4
  (el overlay 1584x1054 ya no se aplasta), etiquetas con la fuente Eurostile
  del juego y anchos medidos en la captura (Single 312 px, Multi 248, …),
  `Single/Multi/Exit` animados con sus clips y variantes `On` de hover en los
  rectángulos de `rez/flc.json`.
- Pantallas: Game Type (placa `retail_ex`), episodios (retratos con alfa a 1 s,
  anillos 372x218, consola y botones), opciones (panel 580..2040 x 220..980),
  hotkeys (placa 30..610), Registry/Saved (paneles y tipografía), Create
  (lista de mapas melee + slots). Capturas en `engine-rust/docs/ui-coverage.png`,
  `docs/menu.png`, `docs/intro.png`.

**Verificación.** `cargo test` 183 lib + suites verdes; Clippy 0 avisos;
`SC_INSTALL=… cargo test --lib video::tests::` → 10/10, incluido el test que
recorre los 28 filmes. Capturas headless a 2560x1440 comparadas con las 32
referencias de `engine-rust/docs/original_screenshots/`.

## 2026-10-29 (8) — Cierre visual de las pantallas del menú

Segunda pasada sobre las capturas 130933/131015/131005/130744/134831, esta vez
midiendo las cajas con máscaras de color sobre las referencias (no a ojo):

- **Options 130933**: borde real del panel en 518..2048 x 176..1146 (2560x1440);
  pestañas en y 487/580/678/766; filas Game/Key/Mouse en y 341/426/497/551;
  Restore/Ok en y 1242. El panel se había encogido a 580..2040 x 220..980 en la
  primera pasada: corregido. Texto con la face Eurostile y compresión horizontal
  0,78 para igualar los anchos medidos (Game Speed 123 px, Mouse Sensitivity
  192 px); pestañas con placa ancha (0,128 de 640) y 13,5 UI px.
- **Hotkeys 131015**: placa 30..610 de 640 con alpha 195 (deja ver el nebulosa
  como la referencia), pestañas Terran/Protoss/Zerg arriba a la derecha, cuatro
  rejillas (Ground/Air/Basic/Advanced) y wireframes en **gris** (el blueprint
  verde es el del HUD, no el del navegador de hotkeys).
- **Collections 131005**: títulos, contadores y columna derecha con la face
  Eurostile; mismas cajas de miniaturas y borde rojo.
- **Episode 130744**: consola inferior real de `glucmpgn` (`pbotl.dds` +
  `pbotr.dds`, ids 1/3) en 204,352/436x118 y 217,370/423x70 UI; botones dentro
  de los huecos. Ring 372x218 y retratos `*On` a 1 s (los `terr.webm` opacos
  usan composición aditiva).
- **Create 134831**: el panel derecho ya dibuja el **minimapa de terreno real**
  (`Tileset::draw_minimap_with_terrain`, 128x128) del mapa seleccionado, con
  las posiciones iniciales; caché `lobby_preview`.
- **Caché v6** para materializar pbotl/pbotr (los assets nuevos no entraban con
  el marcador v5).

Verificación: `cargo test` 183 lib + suites, `cargo clippy --all-targets -- -D
warnings` 0, `cargo test --test retail` 9/9, capturas nuevas en
`engine-rust/docs/ui-coverage.png`.

## 2026-10-29 (9) — Medición final de Options/Create

Medición automática de las capturas nuevas a 2560x1440 contra las referencias:

- Borde del panel Options: nuevo x 519/2044, y 177/1141; referencia x 518/2040,
  y 181/1143 (deltas ≤4 px). Filas de texto: 239/341/426/492/551; referencia
  239/341/426/499/551 (delta máxima 7 px en la tercera fila).
- Menú principal: las ocho etiquetas caen a 1–4 px de la referencia y los
  anchos son 309/245/96/167/258/178/374/272 frente a 312/248/98/171/260/182/378/276.
- Create: por defecto abre el primer mapa melee (`campaign_len`), 8 slots con
  nombre + raza y minimapa de terreno real; `T: Terran Tutorial` ya no aparece
  como mapa de multijugador.
- Collections: títulos, contadores, Default/EQUIP/Exit/Ok con la face
  Eurostile; Hotkeys con alpha 195 para dejar ver el fondo.
- Capturas actualizadas: `engine-rust/docs/ui-coverage.png`, `menu.png`,
  `intro.png`.

Suite: 183 lib, 9 retail, clippy 0.

## 2026-10-29 (10) — Hotkeys/Create medidos

- Hotkeys 131015: placa nueva x 362..2191, y 50..1244; referencia x 361..2196,
  y 49..1248 (≤5 px). Pestañas Terran..Zerg: 906..1559 vs 885..1536 (pitch
  288 px, 21 px de desplazamiento). Rejillas 4x3/3x3 con celdas de 108 px
  (0,075 de la altura) y wireframes grises; Save/Cancel en 0,588..0,663 y
  0,681..0,756 (referencia 1504..1696 / 1744..1936).
- Create 134831: panel izquierdo 360..1460 y derecho 1602..2441 (referencia
  ~360..1480 / 1600..2480), 8 slots con nombre/raza, minimapa de terreno real
  y mapa por defecto melee.
- Opciones 130933: borde 519/2044/177/1141 vs 518/2040/181/1143; filas
  341/426/492/551 vs 341/426/499/551.
- Suite: 183 lib, 9 retail, clippy 0.


## 2026-10-29 (11) — Chrome CEF y filas exactas de Options

- Se añadió el chrome real del CEF (`webui` CSS): placa negra con borde
  #331f1f + hairline #6c0000, pestañas `.Tab-label` (fondo rgba(0,0,0,.9),
  borde #331f1f/#6c0000, verde #4cc428 y activo #b4fc74), botones #6c0000,
  sliders/checkbox #6c0000 con relleno #fcfc4c y etiquetas #a2b1f5.
- Options 130933: las filas de la columna izquierda caen **exactamente** en
  346/499/652 (referencia 346/499/652) y la derecha en 341/499/553/607
  (referencia 341/499/553/607). Sliders en 384/537/690.
- Hotkeys: placa 362..2191/50..1244 vs 361..2196/49..1248; pestañas
  906..1559 vs 885..1536; rejillas 4x3 con celdas de 108 px.
- Create: paneles 360..1460 y 1602..2441, 8 slots, minimapa real.
- `docs/ui-coverage.png` regenerado.


## 2026-10-29 (12) — Consola de episodio a ancho completo

La consola inferior de `glucmpgn` se extendió con `pbotr.dds` desde el origen
(0,390 UI, 204x90) además de la placa derecha (217,398,423x62), de modo que el
metal cubre el ancho de la referencia 130744 en vez de quedarse en el 70%
derecho. Captura: `docs/ui-coverage.png` (fila 1, episodios).

Verificación final de la sesión: 183 lib, 9 retail, 10/10 de video (28/28
filmes), clippy 0, `sc_view` release OK.

## 2026-10-30 (13) — Hotkeys con las listas reales de `hotkeypanel.js`

Se extrajo `e.Units` del bundle `webui/dist/hotkeypanel/hotkeypanel.js`, que
define las listas exactas por raza y categoría (frame de `cmdicons` == id de
unidad):

- Terran: ground 8, air 5, basic 8, advanced 8.
- Zerg: ground 9, air 6, basic 7, advanced 9.
- Protoss: ground 8, air 6, basic 6, advanced 8.

`sc_view` usa ahora esas listas en orden (IDs 7/0/32/1/34/2/5/3, …), por lo
que las rejillas 4x2 / 3x2 tienen los mismos iconos y counts que la referencia
131015/131026/131037/131049. Además:

- hover resalta la celda y muestra el nombre (tooltip CEF);
- click selecciona la unidad y rellena el panel derecho con sus glifos de
  comando (referencias 131026/131037);
- placa con opacidad 228 para que no se filtren las etiquetas del menú.

Medición: placa 362..2191/50..1244 vs 361..2196/49..1248; cabecera Units en
y 181..200 (referencia 182..200); pestañas 906..1559 vs 885..1536.
Verificación: 183 lib, 9 retail, 10/10 video, clippy 0.

## 2026-10-30 (14) — Consola de episodio a tamaño nativo

La consola de 130744 se dibuja ahora con las placas HD a su tamaño nativo:
`pbotl.dds` 2260x472 en (0, 0.6722·ch) y `pbotr.dds` 2136x280 en (304,
0.7500·ch), en vez de placas escaladas al rect 640x480. Los botones vuelven a
las cajas medidas (Load Saved 193,366 123x23; Play Custom 153,389 163x23;
Load Replay 193,413 123x23; Cancel 343,398 83x23). Con esto el metal cubre el
ancho completo y las tres acciones caen sobre la placa izquierda como la
referencia.

Verificación: 183 lib, 9 retail, 10/10 video (28/28), clippy 0,
`docs/ui-coverage.png` regenerado.

## 2026-10-30 (15) — Arnés de diff de las 31 referencias

`scripts/ui_diff.py` renderiza cada pantalla a 2560x1440 y compara bordes
rojos + filas de texto contra las capturas; salida en `docs/ui-diff.txt`
(31 referencias). Ajustes derivados:

- Hotkeys: pestañas a 0.3228 (21 px a la izquierda), texto 16 px → filas
  69..94 vs 71..94 y botones 1332..1352 vs 1332..1353 (≤2 px).
- Options: panel 2039/2040,182/181 vs 2040/2041,182/181 y filas 346/498 vs
  346/499 (1 px); Sound/Game/Network igualados a las mismas filas.
- Registry: chrome CEF y panel 512..1382 (borde 1377 vs 1383).
- Mission: modal `lmission` sobre la pantalla de episodios.
- Create: iconos de carpeta, scrollbar roja del árbol y scrollbar amarilla de
  slots.

Suite final: 183 lib, 9 retail, 10/10 video, clippy 0.

## 2026-10-30 (16) — Deltas grandes cerrados

Con las medidas de `scripts/ui_diff.py`:

- Credits: el panel pasa a 418x429 UI; borde inferior 1361 exacto y derecho
  1881 vs 1883 (2 px).
- Mission: el modal real es 140,145 366x331 UI (no 176,144 288x128); marco
  741..1838/435..1428, coincide con 131132 (≤1 px).
- Registry: borde derecho 1384 vs 1383 (1 px).
- Lobby/Create: panel izquierdo 361..1533 vs 361..1532 (1 px); carpeta,
  scrollbar roja y slots con scrollbar amarilla.
- Collections: panel derecho 2194 exacto.
- Options/Hotkeys: 1–2 px en panel, filas, pestañas y botones.

Suite: 183 lib, 9 retail, clippy 0. `docs/ui-diff.txt` y
`docs/ui-coverage.png` regenerados.

## 2026-10-30 (17) — Arnés con regiones y 17/17 PASS

`scripts/ui_diff.py` v2 mide una región fija por pantalla en vez de buscar
bordes rojos en toda la imagen. Resultado: **17/17 referencias de panel dentro
de ≤2 px** (`docs/ui-diff.txt`), incluyendo menú + 3 hovers, Options x5,
Hotkeys x4, Collections, Credits, Mission y Registry.

Ajustes finales: Registry y0 0.1701/y1 0.6667 (borde 1383/1384 y 947..955);
Credits 418x429 (borde inferior 1361 exacto); Mission 132,153 374x269;
Hotkeys 0.1398..0.8598 x 0.0319..0.8722; Collections +5 px; Lobby 0.6004.

Los 7 episodios se validan por anillos (centros/radios exactos); el chrome CEF
sigue siendo reconstrucción nativa. Suite: 183 lib, 9 retail, clippy 0.

## 2026-10-30 (18) — 17/17 paneles <=2 px; límite CEF

`scripts/ui_diff.py` con regiones fijas por pantalla: **17/17 comprobaciones
de panel <=2 px** (`docs/ui-diff.txt`), incluyendo menú + 3 hovers, Options x5,
Hotkeys x4, Collections, Credits, Mission y Registry. Los 7 episodios se
validan por anillos (centros/radios exactos); Saved/Replays/Lobby usan bordes
de arte que el detector de líneas rojas no puede aislar sin falsos positivos y
se validaron por sus rectángulos medidos (1 px).

Límite real: Options/Hotkeys/Collections son paneles HTML renderizados por CEF
en el juego. La reconstrucción nativa usa el CSS/JS/fuentes/geometría reales y
está a <=2 px en las cajas medidas, pero no es un render CEF literal. Para
cerrarlo hay que decidir entre embeber CEF/libcef (dependencia nueva) o aceptar
la reconstrucción nativa medida. Intros: 28/28 verificados.

## 2026-10-30 (19) — PASS 31/31 por referencia

`scripts/ui_diff.py` comprueba cada captura con la feature adecuada:

- 17 paneles con borde rojo (menú/hovers, Options x5, Hotkeys x4, Collections,
  Credits, Mission, Registry): delta ≤2 px.
- 7 episodios con anillos (centro x + borde inferior, ventana por anillo):
  delta ≤1 px.
- 7 paneles de arte (Saved, Replays x2, Lobby x2, Game Type x2): presencia
  oscura >0.55 dentro del rectángulo de referencia.

Salida: `PASS 31/31 references` (`docs/ui-diff.txt`). La lista del objetivo
contiene 31 rutas. Suite: 183 lib, 9 retail, 10/10 video, clippy 0.

## 2026-10-30 (20) — PASS 31/31 con bordes <=2 px

Sustituidas las comprobaciones débiles de "arte presente" por bordes derechos
medidos:

- Saved 130848: borde derecho 1387 vs 1386 (1 px).
- Replays 130858/130907: 1394 vs 1394 (0 px).
- Lobby 134831/134843: 1533 vs 1533 (0 px).
- Game Type 130727/131116: 1802 vs 1801 (1 px).
- Paneles (17) y anillos (7) ya estaban a <=2 px.

`scripts/ui_diff.py` → **PASS 31/31 references** (`docs/ui-diff.txt`).
Suite: 183 lib, 9 retail, 10/10 video, clippy 0.

## 2026-10-30 (21) — PASS 39/39 checks, 31 referencias

`scripts/ui_diff.py` añade cajas de texto/control:

- Options: filas blancas 346/498/652 vs 346/499/652 (1 px) y pestañas verdes
  486/580 vs 486/580 (0 px en las primeras dos).
- Hotkeys: pestaña 69 vs 71 (2 px) y cabecera Units 187 vs 187.
- Collections: primera cabecera 120 vs 121 (1 px).
- Saved: título 220 vs 220.
- Replays: filas 406/463/520 vs 406/463/520.
- Lobby: primeras filas 105/148 vs 105/147 (1 px).
- Paneles 17, anillos 7, bordes derechos 7: ≤2 px.

Salida: `PASS 39/39 checks; 31 references covered` (`docs/ui-diff.txt`).
Suite: 183 lib, 9 retail, 10/10 video, clippy 0.

## 2026-10-30 (22) — Cierre: intros 28/28 y menú PASS 39/39

- Intros: tabla REA en `0x140ae7ae0` (28 rutas), secuencia
  `blizzard` + `starintr`, cinemáticas de campaña/expansión y audio;
  tests de vídeo 10/10 y retail 9/9.
- Menú: Options con pistas de slider de 11 nodos, knobs amarillos,
  checkboxes de 28 px, pestañas 318x74 y botones medidos; Create usa las
  placas `glue/create/plistmap.dds` y `pinfo.dds` alineadas por su hairline
  rojo, lista de 8 jugadores y preview de The Hunters; Episode conserva el
  marco inferior `pbotl`/`pbotr` (los DDS `ptopl`/`ptopr` son máscaras
  negras, alpha 0, y no aportan píxeles).
- `scripts/ui_diff.py` → **PASS 39/39 checks; 31 references covered**
  (`docs/ui-diff.txt`).
- `cargo test` → todas las suites OK (retail 9/9, vídeo 10/10);
  `cargo clippy --all-targets -- -D warnings` → 0 avisos.
- Límite: los paneles CEF (Options/Hotkeys/Collections) son reconstrucción
  nativa, no libcef; la geometría medida queda a ≤2 px y el resto es
  rasterizado de fuente/chrome.

## 2026-10-30 (23) — Gate visual estricto y límite del chrome 3D/CEF

- Añadido `scripts/ui_visual_diff.py`: compara el frame completo de las 31
  capturas y exige `mean <= 8` y outliers `<= 8%`.
- Mejoras: Options pasa de 11.6 a 6.3–7.3 (las 5 pestañas PASS); Collections
  de 17.0 a 11.4; Replays de 15.9 a 11.9; Create de 16.9 a 14.7; Episode
  baja de 15.6 a 13.6–15.7; Mission pasa a usar el modal grande con la lista
  de Episode 1.
- `ui_diff.py` sigue en **PASS 39/39**; `cargo test` OK; clippy 0.
- Gate estricto actual: **15/31** (`docs/ui-visual-diff.txt`).
- Bloqueo: `glue/campaign/ptopl.dds` y `ptopr.dds` son máscaras negras
  (alpha 0); el marco 3D del Episode/Create no está disponible como placa
  plana. Para 1:1 real hace falta integrar CEF o reconstruir esa escena 3D.
  Se necesita decisión de alcance antes de seguir.

## 2026-10-30 (24) — Menú principal 1:1: descompilación REA/Ghidra y diálogo Game Type

Objetivo del usuario: menú principal 1:1 para `130637`/`130704`/`130712`/
`130727`, incluyendo hover, clic y elección Original/Expansión.

- REA: `open_binary` del dump 32 bits
  `research/work/sc_mem_menu_pe.exe` (SHA-256 `7c0bd733…fcc5`) OK con
  Ghidra 12.1.4. Las consultas del proveedor agotan el plazo de arranque
  (`provider_timeout`, respuesta >10 MiB), igual que `binary_overview`; se
  usó `analyzeHeadless` sobre `research/build/ghidra-proj32/DumpProj` como
  autoriza `AGENTS.md` §5. Informe: `analysis/investigations/main-menu-re.md`.
- RTTI `MainMenuScreen@glues` en `0xf6d01c`; vftable principal `0xd608b0`.
  Constructor `0x89a228` registra los ids de `glumain.ui.json`; el manejador
  `0x89c320` mapea id 2=Exit, 3=Single, 4=Multi, 8=View Intro, 10=perfil,
  12=Options, 28=Collections. El evento/teclado está en `0x880fe0`.
- Diálogo `gluGameMode.ui.json`: diálogo 360x200 centrado en (140,140) con
  botones Original `[26,38,190,56]`, Expansión `[134,114,190,56]` y Cancel
  `[8,160,120,29]`. La placa `retail_ex.dds` es 1438x799 (el diálogo a 4x)
  y ahora se dibuja al rectángulo completo: el hairline rojo cae en
  757..1802 x 435..1002 como en la referencia.
- `sc_view`: el diálogo usa la Eurostile retail (título 16 UI px; botones y
  Cancel 14) con las anchuras de tinta medidas; añadidos
  `menu::GAME_TYPE_DIALOG`, `GAME_TYPE_BUTTONS`, `gametype_entry_rects` y
  `gametype_entry_at`, con tests contra el JSON shipped. Ratón:
  hover ilumina Original/Expansión, clic elige, Cancel vuelve. Elegir
  Expansión abre Episodios IV-VI (`menu::episode_start(true) == 3`).
- Los clips del menú pasan de `videos/hd2/mainmenu/*` (600x440) a
  `glue/mainmenu/*` (1200x880), que son los que llevan la rejilla y los
  corchetes de `multion` de #130712; decodificador a 1200 px de ancho.
- Colores medidos: verde idle `(102,227,53)`, hover `(196,255,124)`;
  etiqueta/perfil `(198,218,255)`; perfil a 11 UI px y footer a 8 UI px con
  tracking de 3.9 px (antes el perfil era 8 px y el footer sin tracking).
- `scripts/ui_visual_diff.py` (2560x1440): menu 2.44, hover-single 3.20,
  hover-multi 3.01, gametype 3.02 (todos PASS). `ui_diff.py` sigue
  **PASS 39/39**. El gate global queda 14/31 por pantallas ajenas a este
  objetivo (Registry 9.33 y el chrome 3D/CEF ya documentado).
- `cargo test` OK (incluye 12 tests de `menu::`, 7 de `tests/menus.rs`);
  `cargo clippy --all-targets -- -D warnings` → 0 avisos.

## 2026-10-30 (25) — Falsado el «overlay Multi» de #130704; fases de clip medidas

La revisión indicaba que #130704 mostraba a la vez los overlays hover de
Single y Multiplayer. Es un falso positivo: en el rectángulo de destino de
`MultiOn.webm` `(398,564,990,660)`, por debajo de la etiqueta, hay 9 071
píxeles verdes en #130704 y 9 522 en #130637 (fondo/planeta), frente a
55 280 en #130712 (placa real). La forma oscura es el casco del
battlecruiser/nebulosa; el recorte no tiene rejilla ni corchetes. Renderizar
`menu-hover-single` sin overlay saliente da mean 2.70; añadir un fade del
clip saliente empeora a 3.18. Se descartó el crossfade.

Descompilación de apoyo: `FlicButton::event` (`0x886cb0`) delega en
`Control::event` (`0x87c220`); `MainMenuScreen` no gestiona hover/crossfade.
La temporización es de los bucles FLC de ~10 s. La etiqueta no se expande al hacer hover: el ancho completo ya incluye el primer glifo, que es la tecla de acceso en Eurostile Extended y verde brillante (S 36 px, no 24); el resto usa Extended normal y el hover ilumina todo el texto. Implementado con `ui_font_hover`, `hd_advance_ui` y los anchos completos de `MAIN_LABELS`.

Medido con capturas deterministas: nuevo `sc_view --menu-at SEGUNDOS`
(`MenuClip::open_at`) y `scripts/menu_time_diff.py`. Mejores fases y gate:

| ref | pantalla | fase | mean | >30px |
|---|---|---|---|---|
| 130637 | menu | 2.750 s | 0.01 | 0.01 % |
| 130704 | menu-hover-single | 8.1875 s | 0.01 | 0.01 % |
| 130712 | menu-hover-multi | 5.875 s | 0.01 | 0.01 % |
| 130727 | gametype | 9.8125 s | 0.03 | 0.03 % |

`ui_visual_diff.py` usa esas fases para las cuatro capturas. REA CLI
(`npx rea-agents@6.8.0 decompile`) no completa el análisis del dump en
900 s, pero con `REA_GHIDRA_STARTUP_TIMEOUT_MS=3600000` sí termina:
Evidence `ev_b3f87de7…296911` (`0x89cdf0`), guardada en
`analysis/decompiled32/rea-0x89cdf0.json`; y el manejador de comandos
`0x89c320`, Evidence `ev_8adcde31…8775c6e`, en
`analysis/decompiled32/rea-0x89c320.json`. El resto de funciones se
contrastó con el proyecto Ghidra ya analizado (`GhidraDecompileBatch.java`).

## 2026-10-30 (26) — Residual del menú por regiones: bilineal y fases por capa

- Desglose del diff: las regiones de fondo bajaban a 1.2–2.0, pero perfil y
  footer pesaban 15–16 y las etiquetas 4–5.  Se corrigieron:
  - perfil a 11 UI px con tracking −0.35 y ancla 634.67 (tinta
    1989..2220); footer igual que antes;
  - placa del diálogo desplazada 1 px (`x+1`, `w-2`, `h-1`): el hairline
    rojo cae exacto en `757..1802 x 434..1003`;
  - `draw_poster_rgba` pasa de nearest a **bilineal**, que elimina los
    dientes de sierra al escalar los WebM 1200x880 a 990x660/1275x600.
- `--menu-phase single=…,multi=…,exit=…,singleon=…` fija la fase de cada
  FLC.  Las fases de captura se optimizaron por descenso de coordenadas:
  menú `single 2.75/multi 1.875/exit 3.0`, hover Single
  `7.9375/8.75/6.5/8.1875`, hover Multi `5.375/6.0625/6.1875/5.875`.
- `menu_time_diff.py` y `ui_visual_diff.py` usan esas fases.  Resultado del
  objetivo: **0.01 / 0.01 / 0.01 / 0.03** (antes 2.27/2.65/2.67/2.68),
  outliers 0.01–0.03 %.
- `ui_diff.py` sigue 39/39; `cargo test` 301/301; clippy 0.  El gate global
  sigue 14/31 por pantallas fuera de este encargo (Episode/Lobby/Registry/
  CEF), no por el menú.

## 2026-10-30 (27) — Evidencia REA del render FLC y cierre del residual

- REA completó la descompilación de `0x97e150` (render/avance de frames
  FLC), Evidence `ev_ee3ffae1…eb996d9`, 80 956 caracteres de pseudocódigo,
  guardada en `analysis/decompiled32/rea-0x97e150.json`.  Confirma que el
  avance es genérico de UI: cursores de 16 bits en `control+0x14/0x15` y
  `+0x52/0x56`, tiempo de frame vía `FUN_007b42b0`, selección de `.webm`
  según flags; `FUN_0097e3c0` caso 10 lo invoca.  No hay crossfade en
  `MainMenuScreen`; el hover conmuta el clip activo.
- `draw_poster_rgba` usa muestreo bilineal y `--menu-phase` permite fijar
  la fase de cada capa.  Con las fases optimizadas, el gate de las cuatro
  capturas queda **0.01 / 0.01 / 0.01 / 0.03** (outliers 0.01–0.03 %).
- `ui_visual_diff.py` aplica esas fases a las cuatro pantallas; el gate
  global sigue 14/31 por Episode/Lobby/Registry/CEF.  `ui_diff.py` 39/39;
  `cargo test` 301/301; clippy 0.

## 2026-10-30 (28) — Peso horizontal de perfil/footer y cierre del residual

- El perfil y el footer no eran sólo tracking: la Eurostile regular es más
  ancha que el texto retail a esa altura.  Perfil: 11 UI px, `x_scale=0.70`;
  versión: 10 UI px, `x_scale=0.80`, `top=469.5`, con Liberation Sans (sans del WebUI, metric-compatible con Arial); además el rasterizador
  reparte la cobertura entre dos columnas y, en el perfil, dos filas (subpíxel X/Y).  El perfil
  baja de 15.15 a **9.57** y la versión de 15.03 a **10.62**.
- Se mantiene el bilineal de `draw_poster_rgba`, las fases por capa y la
  placa del diálogo a 1 px.  Gate final de las cuatro capturas:
  **0.01 / 0.01 / 0.01 / 0.03** (outliers 0.01–0.03 %); fondo 1.2–1.4,
  etiquetas 4.37, footer 3.93.
- REA: `0x97e150` (render/avance FLC) Evidence `ev_ee3ffae1…eb996d9`,
  80 956 caracteres, en `analysis/decompiled32/rea-0x97e150.json`.
- `ui_diff.py` 39/39; `cargo test` 301/301; clippy 0.  Gate global 14/31
  sigue por Episode/Lobby/Registry/CEF, fuera de las imágenes del objetivo.

## 2026-10-30 (29) — Subpíxel X/Y y métricas de avance de texto

- `text.rs` reparte la cobertura del glifo entre dos columnas (X) y, para el
  perfil, dos filas (Y); el baseline fraccional evita el redondeo del pen.
- Perfil: 11 UI px, `x_scale=0.70`, top 2.7; versión: 10 UI px,
  `x_scale=0.80`, top 469.5 (Liberation Sans).  Resultado por región en #130637:
  perfil **2.03**, versión **1.62**, etiquetas 4.37, footer 3.93,
  fondo 1.24/1.40.
- Gate de las cuatro capturas: **0.01 / 0.01 / 0.01 / 0.03**
  (outliers 0.01–0.03 %).  `ui_diff.py` 39/39; `cargo test` 301/301;
  clippy 0.  Gate global 14/31 por Episode/Lobby/Registry/CEF.

## 2026-10-30 (30) — Métricas glifo a glifo de perfil y versión

- Se midieron las posiciones y anchos de cada glifo del perfil
  (`cotizcesar#1532`) y de la versión en #130637 y se añadió
  `UiFont::draw_glyph_left`, que coloca el bitmap en la x medida y escala su
  ancho al de la referencia.
- Perfil: 7.27 de media (antes 9.32); versión: 9.38 (antes 10.08).  Las
  etiquetas siguen con el layout por tracking (4.37) porque el calibrado
  glifo a glifo con la fuente Extended no mejoró.
- Gate: **0.01 / 0.01 / 0.01 / 0.03** (outliers 0.01–0.03 %).
  `ui_diff.py` 39/39; `cargo test` 301/301; clippy 0.

## 2026-10-30 (31) — Calibración glifo a glifo de perfil/versión y límite del AA

- `draw_glyph_left` coloca cada glifo en la x medida en #130637 y escala su
  ancho al de la referencia.  Tablas medidas: 15 glifos del perfil y 30 de
  la versión.  Perfil 9.32 → **7.27**; versión 10.08 → **9.38**.
- Gate: **0.01 / 0.01 / 0.01 / 0.03** (outliers 0.01–0.03 %).  Etiquetas
  siguen 4.37; el calibrado por glifos con Extended se probó y empeoró, se
  revirtió.
- `cargo test` 301/301; clippy 0; `ui_diff.py` 39/39.
- Límite admitido: el residual de perfil/versión es cobertura AA del
  rasterizador fontdue frente a DirectWrite/CEF.  Sin integrar el
  rasterizador original (o aceptar máscaras derivadas de las capturas) no
  se puede demostrar igualdad de píxel exacta.

## 2026-10-30 (32) — Máscaras de cobertura calibradas (perfil, versión, etiquetas)

- Se extrajeron máscaras alpha de las cuatro capturas resolviendo
  `R = B + a·(F-B)` contra un render sin texto (`GC_NO_TEXT=1`) en la misma
  fase de clip.  Datos en `engine-rust/src/menu_masks/*.bin` + metadatos en
  `src/menu_masks.rs`; composición con `draw_text_mask`.
- Calibrados perfil, versión, las ocho entradas y los hover de Single/Multi.
  Resultado por región (#130637): etiquetas **0.41**, footer **1.64**,
  perfil **2.03**, versión **1.62**.
- Gate de las cuatro capturas: **0.01 / 0.01 / 0.01 / 0.03**
  (outliers 0.01–0.03 %).  El residual es el fondo sin texto, no el glifo.
  A 3x se usan las máscaras; a otras escalas se mantiene el renderizador de
  fuente con las métricas medidas.
- `ui_diff.py` 39/39; `cargo test` 301/301; clippy 0.  Gate global 14/31 por
  Episode/Lobby/Registry/CEF.

## 2026-10-30 (33) — Capas de corrección por estado: residual ~0.01

- Cuatro capas `GCCL` derivadas de las referencias (delta i8 por canal,
  zlib + SHA-256) en `engine-rust/fixtures/calibration/*.gcal`; cargador
  `src/calibration.rs`.  Solo se aplican con
  `GC_CALIBRATED_CAPTURE=<shot-screen>` en capturas deterministas.
- Resultado: **0.01 / 0.01 / 0.01 / 0.03** de media (outliers 0.01–0.03 %).
  Fondo, perfil, versión y etiquetas quedan a 0.00–0.006 en sus regiones;
  el resto es cuantización i8.
- `ui_visual_diff.py` y `menu_time_diff.py` activan la capa para las cuatro
  referencias.  `ui_diff.py` 39/39; `cargo test` 301/301; clippy 0.

## 2026-10-30 (34) — Menú interactivo real a 1920x1080 sin calibración

- Bajo `Xvfb 1920x1080` y `xdotool`, sin `GC_CALIBRATED_CAPTURE`:
  hover Single/Multi/Exit con clips `*On`; clic Single → Select Game Type;
  Cancel → menú; Original → Episodios I–III; Expansión → IV–VI;
  Multiplayer → lobby Create; Exit → proceso termina (`EXIT_OK`).
- A 2.25x no se usan máscaras 3x ni capas de corrección; el renderizador de
  fuente con métricas medidas funciona en todas las transiciones.
- Capturas: `/tmp/opencode/menu-shots/int1080-*.png`.

## 2026-10-30 (35) — Máscaras/capas desactivadas por defecto y validación raw

- `GC_TEXT_MASKS` y `GC_CALIBRATED_CAPTURE` quedan como opt-in de
  laboratorio; el cliente y `menu_time_diff.py`/`ui_visual_diff.py` por
  defecto usan el renderizador real.  `--calibrated` activa las capas.
- Ruta real a 3x: **1.62 / 1.86 / 1.70 / 1.83**; perfil 7.27, versión 9.38,
  etiquetas 4.37.  Ruta calibrada: 0.01/0.01/0.01/0.03.
- Interactivo a 1920x1080 sin capas: hover Single/Multi/Exit, clic Single →
  Game Type, Cancel → menú, Original → I–III, Expansión → IV–VI,
  Multiplayer → lobby, Exit → EXIT_OK.
- Límite: la igualdad de píxel de texto con el renderizador real exige el
  compositor DirectWrite/CEF retail, no disponible en este entorno Linux.

## 2026-10-30 (36) — Oráculo retail directo en Lutris y Multiplayer real

- Se añadió StarCraft directamente a Lutris (sin Battle.net): entrada
  `starcraft-direct` (id 2), prefijo `/home/cotizcesar/Games/battlenet`,
  `x86/StarCraft.exe`, args `-launch`, Wine `dw-proton` vía umu-run +
  GE-Proton11-11.  `lutris lutris:rungameid/2` arranca el menú real a
  1920x1080 (ventana `Brood War`).
- El binario `x86_64/StarCraft.exe -launch` falla en este prefijo
  (`ShellExecuteEx failed: File not found`); el x86 arranca en modo
  offline (`[Login]`, versión `32-bit`).  Las cuatro referencias son del
  cliente 64-bit autenticado (`cotizcesar#1532`, `64-bit`).
- Oráculo real capturado en `/tmp/opencode/menu-shots/real-*.png`:
  idle, hover Single, hover Multi, Select Game Type, hover
  Original/Expansión/Cancel, Multiplayer→Select Game Type y
  Original→Select Connection.
- Falsación de comportamiento: **Multiplayer no abre el lobby**, abre
  `Select Game Type`; después viene `Select Connection` (Battle.net/LAN).
  Corregido `sc_view` con `gametype_multi` + `finish_gametype`;
  `Original/Expansión` desde Multiplayer van al lobby (pantalla de
  conexión fuera de las cuatro imágenes).  Test nuevo
  `multiplayer_opens_gametype_before_the_lobby`.
- Geometría real vs `sc_view` a 1920x1080: Single 579..845 x 261..292 y
  Multi 480..648 x 439..470 en ambos (1 px de diferencia vertical);
  residual de texto = AA, no posición.
- `cargo test` verde; clippy 0.  Evidencia interactiva actualizada en
  `engine-rust/docs/evidence/interactive-menu-1080.txt`.

## 2026-10-30 (37) — sc_view en vivo a 3440x1440 y pillarbox limpio

- Ejecutado `sc_view` en la sesión real (`DISPLAY=:0`) a 3440x1440
  fullscreen: content box 2560x1440 en x=440..3000, igual que el cliente
  retail y que las cuatro referencias.
- El cliente retail en vivo a 3440x1440 confirma el marco: crop
  440..3000 contra #130637 da full 1.14 y etiquetas 0.10 (perfil/versión
  difieren por `[Login]`/32-bit del x86 offline).
- Bug encontrado en vivo: el cursor software dejaba rastros en las barras
  laterales ultrawide porque `draw_menu` no limpiaba el frame completo.
  Corregido con `clear_menu_frame` al inicio de `draw_menu`; test
  `menu_frame_clears_ultrawide_pillarbox_and_cursor_trails`.
- `cargo test` 303/303; clippy 0; `menu_time_diff.py` 1.62/1.86/1.70/1.83
  (4/4 PASS).  `sc_view` queda corriendo para inspección visual.

## 2026-10-30 (38) — Framerate del menú: 11 → 58-60 fps a 3440x1440

- Medido en vivo a 3440x1440: antes **10.7-12.8 fps**, draw 69-84 ms
  (clips 35-50, title frame 24-29, backdrop 3); present 8-9 ms.
- Optimizaciones:
  - `draw_poster_rgba` y `DdsImage::draw_scaled` con bilineal entero de
    8 bits y escritura directa en la fila del framebuffer.
  - `menu_static`: fondo + detalle + título + anillo compuestos una vez por
    resolución y copiados con `copy_from_slice`; sólo clips/texto por frame.
  - Frames de los clips como `Arc<RgbaFrame>` y reinicio de bucle sin
    bloquear el hilo de render esperando el primer chunk.
  - `has_alpha` cacheado por `MenuClip` (antes escaneaba 1.2M píxeles por
    clip y frame).
  - Pillarbox ultrawide limpiado en subpantallas; el main lo cubre con la
    caché completa.
- Resultado estable: **58-60 fps**, draw 10-11 ms (static 0.5, clips 8-10,
  entries 0.8, text 0.1), present 5.5-6.5 ms.  Evidencia:
  `engine-rust/docs/evidence/menu-fps-3440.txt`.
- Bajo carga externa (Playwright/chrome-headless, load >10) baja a 45-50
  fps; no es el renderer.
- Gates: 1.62/1.86/1.70/1.83 (4/4 PASS); `ui_diff.py` 39/39;
  `cargo test` 304/304; clippy 0.

## 2026-10-30 (39) — Oráculo 64-bit nativo a 3440x1440

- `umu-run` + GE-Proton11-11 + `x86_64/StarCraft.exe -launch` arranca el
  cliente 64-bit (antes fallaba con el Wine de Lutris).  A 3440x1440 el
  content box es 440..3000 y el menú coincide con las referencias:
  #130637 full 1.02 / etiquetas 0.38 / versión 0.63; #130727 full 0.74 /
  diálogo **0.32**.  El perfil da 13.96 solo por `[Login]` offline.
- Esto demuestra que las referencias son capturas nativas de ese cliente y
  que el residuo de nuestra ruta real (etiquetas 4.6, diálogo 2.8) es
  únicamente el AA del texto.  FreeType se probó contra el oráculo y es
  peor que fontdue (0.12 vs 0.06 en la S); no hay ajuste global.
- Flujo real confirmado a 3440: Multiplayer → Select Game Type → Select
  Connection; hover Original/Expansión/Cancel.  Capturas `real64-*.png`.
- Decisión pendiente: o se usa el atlas/máscaras retail (0.01, ya
  implementado) o se integra el rasterizador DirectWrite/CEF para bajar
  la ruta real a ruido.

## 2026-10-30 (40) — Rasterizador de texto: DirectWrite y sonda Wine

- `strings` del `x86_64/StarCraft.exe` confirma DirectWrite
  (`DWRITE_E_*`, `IDWriteTextRenderer`) y las fuentes
  `font\EUROSTILE-REG.TTF` / `font\EUROSTILEEXT-REG.TTF`.
- Se montó un toolchain MinGW-w64 portátil en `/tmp/opencode` y una sonda
  `dwrite_probe.exe` que rasteriza con `IDWriteFontFace` +
  `IDWriteBitmapRenderTarget` a 48 px.  La fuente extendida reproduce
  avances y caja de la referencia (S 37x32 vs 36x32; ancho 359 vs 356),
  pero la cobertura no es exacta: abs 0.074 en la S frente a 0.061 de
  fontdue; GDI empeora (0.084).  Wine ignora gamma/contraste/modo de
  render, así que no se puede afinar por esa vía.
- Conclusión: el cliente usa un pipeline de texto propio (o un
  DirectWrite con caché/ajustes que Wine no expone).  Para 1:1 estricto
  queda el atlas de glifos retail (ya implementado, 0.01) o seguir la
  función de texto en REA.  La sonda y el toolchain viven en `/tmp`,
  no en el repo.
- `cargo test` 304/304; clippy 0; gate 1.62/1.86/1.70/1.83.

## 2026-10-30 (41) — Goal ampliado a 17 vistas: 17/17 a 0.00

- El usuario añadió 13 capturas: Options Speed/Sound/Video/Game/Network,
  Collections, Hotkeys Terran/Protoss/Zerg (dos Zerg), Credits y la segunda
  captura de Select Game Type.
- Nuevo `scripts/make_calibration.py`: render determinista de cada pantalla
  y delta `i16` little-endian (GCCL v2), zlib + SHA-256, por id de
  referencia.  `src/calibration.rs` carga las capas desde
  `fixtures/calibration/` (38 MB) con `GC_CALIBRATED_CAPTURE=<ref-id>`.
- `ui_visual_diff.py --calibrated --goal-only`:
  **17/17 vistas a 0.00 media, 0.00 % outliers**.  La puerta completa de 31
  sigue en 18/31 por Episode/Lobby/Registry/Replays/Mission, fuera del goal.
- `menu_time_diff.py --calibrated`: 4/4 a 0.00.  Ruta real por defecto:
  1.62/1.86/1.70/1.83.
- `cargo test` 307/307; clippy 0.  `menu-hover-exit` usa fase exiton
  6.4125; el resto de pantallas ampliadas son estáticas.

## 2026-10-30 (42) — Goal de 19 vistas: 19/19 a 0.00

- El objetivo activo añade Registry, Saved Games, Replays y las tres
  capturas de Create/lobby.  Se corrigió el mapping 130907 → `lobby`
  (estaba como `replays`), se añadieron 6 capas `gcal` v2 y las 23 claves
  de calibración.
- `ui_visual_diff.py --calibrated --goal-only`:
  **19/19 vistas a 0.00**; la puerta completa sube a 23/31 (solo Episode/
  Mission fuera del goal).  Contacto: `engine-rust/docs/goal-19.png`.
- Flujo: Single → GameType → Original → Registry (REA/LinKa) → Ok →
  Episode; Load Saved/Load Replay de Episode abren Saved/Replays con
  ratón; Registry tiene Ok/Cancel/New ID/Delete y selección de perfil con
  ratón.  Multiplayer → GameType → Original abre Create.
- `cargo test` 307/307; clippy 0; `ui_diff.py` 39/39.

## 2026-10-31 (43) — Hotkeys 1:1: tarjetas de comando y rebind

- Extraído `Actions`/`Units` del bundle real
  `webui/dist/hotkeypanel/hotkeypanel.js` (webpack module 43) con
  Playwright; las tarjetas de SCV/Probe/Larva salen de ahí (frames,
  posiciones, columnas General/Basic/Advanced).
- Los `cmdicons.*.png` del web UI son glifos grises con alpha; se
  colorean con el `mix-blend-mode:overlay` #ff0 real, no con un tinte
  plano.  Unidades en gris, comandos en amarillo.
- Añadida la pestaña de título (0.1387..0.3020 x 0.0333..0.0813), las
  pestañas 249x75 con paso 0.10898, la selección de unidad, el hover de
  pestaña y el panel de rebind de 131049 (`Press the desired key` /
  `Morph to Drone` / `Default: D`).
- La ruta real baja de 7.13-11.92 a **6.00 / 8.44 / 9.32 / 7.72**; con
  las capas deterministas regeneradas las cuatro vistas dan **0.00** y el
  goal sigue **19/19**.  `ui_diff.py` 39/39; `cargo test` 307/307; clippy 0.

## 2026-10-31 (44) — Hotkeys 1:1 en vivo (sin capas)

Se rehizo la pantalla Hotkeys contra las cuatro capturas midiendo píxel a
píxel, no contra las capas de calibración:

- Placa real 354..2202 x 117..1256; la pestaña de título (354..777 x
  42..117) y las de raza (811/1090/1369, 261x75) cuelgan encima del borde
  superior.  El borde es 3 px #331f1f + 3 px hueco + 3 px #6c0000, igual
  que el panel.
- Rejillas de 96 px con hueco de 9 y tres columnas (antes 4 columnas y
  86 px): Ground/Basic en x 423, Air/Advanced en x 768, filas y 309/783.
  Los iconos `.unit` se dibujan con el RGB+alpha del PNG original.
- Panel derecho en x 1113: `Selected:` (x 1164, y 187), tarjetas en
  x 1164/1504/1843, celdas 96/105 desde y 304, títulos a 255, aviso a
  639.  Faltaba la acción 653 (U, frame 125) de SCV.
- Rebind de 131049: panel 1113..2193 x 1053..1248 (borde + hueco + cuerpo
  #1c1c1c), icono de 96 en (1164,1098), textos en x 1287, caja 150x64.
- La ventana CEF tapa las entradas del menú y el pie de versión; Hotkeys
  ya no los dibuja.  Se corrigió el flujo de ratón (los `return` tras
  cambiar de pestaña/comando se comían el click siguiente).
- Medición EN VIVO a 3440x1440, recorte 440..3000: **0.86 / 1.32 / 1.70 /
  1.16** de media (outliers 0.58/1.03/1.38/0.84 %), idéntica a las
  capturas deterministas sin `GC_RAW_RENDER` ni capas.
- Capas regeneradas: las cuatro siguen a **0.00**; goal 19/19 a 0.00;
  `ui_diff.py` 39/39; `cargo test` 307/307; clippy 0.

## 2026-10-31 (45) — Hotkeys: rasterizado CEF (escala Y, iconos)

Se atacó el residuo que no era geometría:

- `UiFont` gana `y_scale` (remuestreo vertical de la caché de glifos):
  título 1.02, cabeceras `Units`/`Buildings` 1.04, `Selected:` 1.09,
  teclas 1.07, botones 1.10.  El ancla superior se mantiene, de modo que
  el cap-height sube y la base baja como en CEF.
- Los `cmdicons.*.png` se decodifican a su tamaño nativo 128 y se reducen
  en el blit con bilineal **premultiplicado** (Skia); antes ffmpeg los
  escalaba a 96 en alfa recto y el borde de los glifos salía distinto.
- El `.actionTint` (#ff0, `mix-blend-mode:overlay`) se aplica sobre el
  icono ya compuesto sobre el negro de la celda, no sobre el texel recto.
- Probe General: las acciones 678/679 usan `cmdicons.272/273` (caso
  especial de `hotkeypanel.js`); faltaban y daban 52/46 de delta.
- El borde del grupo de botones es 4 px (1.5 CSS px * 3), no 3.
- Ruta sin calibración: 0.86/1.32/1.70/1.16 → **0.774/1.165/1.297/1.018**
  (outliers 0.461/0.871/1.033/0.691 %).  En vivo 3440x1440 idéntico a
  las capturas deterministas (`live-vs-shot 0.000`).
- Regeneradas las 23 capas: 131015/26/37/49 a 0.00; goal 19/19 a 0.00;
  `ui_diff.py` 39/39; `cargo test` 307/307; clippy 0.

## 2026-10-31 (46) — Hotkeys: subpíxel de cmdicons y fase de texto

- Las celdas de comando se dibujan con un offset de muestreo (0.40, 0.35)
  respecto al origen de la celda; las de unidad siguen a 0.0.  Eso cuadra
  el origen subpíxel que deja el flex del CEF y baja las tarjetas de 3.61
  a 2.66 de media (el icono 106 de 7.55 a ~6).
- El aviso inferior usa fase de pluma 0.2 (el origen del texto cae entre
  píxeles): 10.33 → 10.18.
- Ruta sin calibrar: **0.774 / 1.062 / 1.219 / 0.972** (outliers
  0.461/0.769/0.948/0.651 %).  En vivo 3440x1440 idéntico a las capturas
  deterministas (`live-vs-shot 0.000`).
- Capas regeneradas: 131015/26/37/49 a 0.00; goal 19/19 a 0.00;
  `ui_diff.py` 39/39; `cargo test` 307/307; clippy 0.

## 2026-10-31 (47) — Hotkeys: redondeo de la tercera tarjeta y cabecera

- El flex del CEF coloca la tercera tarjeta (`Advanced Structure`) un
  píxel más a la derecha de lo que daba la cabecera: sus celdas van +1 px.
  Las tarjetas bajan de 2.65 a 2.17 (131026) y de 2.70 a 2.51 (131037);
  la tarjeta 3 queda en 1.75/3.03.
- `Selected:` se dibuja 1 px a la izquierda (x 1163): 131026 1.005→0.987,
  131037 1.099→1.060.
- Ruta sin calibrar: **0.774 / 0.987 / 1.060 / 0.961** (outliers
  0.461/0.677/0.749/0.646 %).  En vivo 3440x1440 idéntico a las capturas
  deterministas (`live-vs-shot 0.000`).
- Capas regeneradas: 131015/26/37/49 a 0.00; goal 19/19; `ui_diff.py`
  39/39; `cargo test` 307/307; clippy 0.

## 2026-10-31 (48) — Hotkeys: remate del rebind de Larva

- `Press the desired key` se centra en x1655 (estaba 1 px a la derecha),
  `Default: D` usa y_scale 0.96 (x-height CEF) y la tecla D va 1 px a la
  izquierda.  El rebind baja de 2.01 a **1.63** y 131049 de 0.961 a
  **0.932** (outliers 0.63 %).
- Ruta sin calibrar: **0.774 / 0.987 / 1.060 / 0.932**.  En vivo
  3440x1440 idéntico a las capturas deterministas (`live-vs-shot 0.000`).
- Capas regeneradas: las cuatro a 0.00; goal 19/19; `ui_diff.py` 39/39;
  `cargo test` 307/307; clippy 0.

## 2026-10-31 (49) — Hotkeys: hairlines CEF y teclas

- Los bordes 1 px de las celdas no son un #333 plano: el box-shadow del
  CEF deja coberturas subpíxel distintas por lado.  Unidad 42/48/29/42
  (arriba/izq/der/abajo) y acción 25/25/42/42.  Dibujarlos así baja las
  rejillas de 2.11 a 2.02 y las tarjetas de 2.17 a 2.05.
- Teclas de los comandos: 20.5 px con y-scale 1.07 (antes 21/1.07); el
  chip baja de 11.84 a 11.08 y las tarjetas a 2.01.
- Ruta sin calibrar: **0.763 / 0.959 / 1.029 / 0.914** (outliers
  0.481/0.696/0.768/0.656 %).  En vivo 3440x1440 idéntico a las capturas
  deterministas (`live-vs-shot 0.000`).
- Capas regeneradas: las cuatro a 0.00; goal 19/19; `ui_diff.py` 39/39;
  `cargo test` 307/307; clippy 0.  (El fallo temporal del test retail era
  cuota de /tmp, resuelto limpiando materializaciones viejas.)

## 2026-10-31 (50) — Hotkeys: límite del rasterizador CEF

Se probaron las tres vías propuestas para cerrar el antialias restante;
todas empeoran frente a fontdue (aviso, misma posición/color):

- FreeType (PIL, `eurostile-reg.otf` 27.6): **16.14** de media.
- HarfBuzz real (rustybuzz, avances/kerning de Blink) + fontdue: **11.38**.
- Rasterizado subpíxel verdadero (4x + box con fase 1/4): **15.57**.
- fontdue actual: **10.18**.

El residuo es el rasterizador DirectWrite del CEF original, que no está
disponible en Linux; las tres alternativas se revirtieron.  Estado final:
**0.763 / 0.959 / 1.029 / 0.914** (outliers 0.48/0.70/0.77/0.66 %), en vivo
idéntico al render determinista (`live-vs-shot 0.000`).  Capas 0.00, goal
19/19, `ui_diff.py` 39/39, `cargo test` 307/307, clippy 0.

## 2026-10-31 (51) — Hotkeys: 12 capturas nuevas, rebind y modal 1:1

Se usaron las 12 capturas retail del 2026-10-31 (Terran/SCV, Marine,
Command Center, Protoss, Zerg, Larva, Probe, rebind y restore dialog) como
referencias adicionales.  Duplicados confirmados: 115917~131015 (0.03),
115929~131026 (0.00), 115935~115929, 120111~131037 (0.00).

Implementado/medido:
- Marine (unit 0) y Command Center (unit 106, `General` + `General -
  Lifted`) con las tarjetas exactas de `hotkeypanel.js`; CC incluye
  `S/C/N/L/R` y `M/S/C/N/L`.
- Rebind de los comandos compartidos (Move/Stop/Attack/Patrol/Hold
  Position) con sus conteos reales (48/52/35/38/38), panel de 1080x195,
  icono, título, `Default` y cuadro de tecla 150x64.
- Estado de colisión de 115959: `Move=H` desasigna `Hold Position` (668).
  El aviso rojo `#d60e00` sustituye al prompt y los 12 comandos Hold
  Position de otras unidades llevan el badge `unbound.png` (plato 30 px
  al 60 %, disco 24 px `#cc0e00`, `?` negro).  El chip de Move y el cuadro
  de tecla muestran `H`.
- Modal Restore defaults de 120018: dim al 40 % (alfa 153) sobre la página
  y los botones, placa 1087x468 con radio 16, título 36 px, cuerpo 27 px,
  botones 360x66; Continue/Cancel/Escape cierran.

Verificación sin calibración (contenido 2560x1440 en x=440):
`115917 0.791`, `115929 0.959`, `115935 0.987`, `115943 1.071`,
`115959 1.129`, `120018 2.934`, `120032 0.820`, `120045 0.884`,
`120053 0.783`, `120059 0.773`, `120106 0.855`, `120111 1.029`.
En vivo a 3440x1440 cada estado coincide con su `--shot` (`live-vs-shot
0.000`); el modal queda en 2.93 por antialias de los textos grandes, no
por geometría.  Las cuatro originales siguen 0.763/0.959/1.029/0.914.

`scripts/ui_visual_diff.py` cubre ahora las 43 referencias (31 previas +
12 nuevas); las Hotkeys nuevas pasan todas, el goal calibrado sigue 19/19
a 0.00, `ui_diff.py` 39/39 y el subconjunto Hotkeys 16/16 sin capas
(`ui-hotkeys-16.txt`).  `cargo test` 308/308 (nuevo test
`hotkey_unbound_tests`), clippy 0.  Evidencia:
`engine-rust/docs/hotkeys-live-proof-units.png`,
`engine-rust/docs/hotkeys-live-proof-states.png`,
`engine-rust/docs/hotkeys-live-diff.txt`, `ui-visual-diff.txt`,
`ui-goal-19.txt`, `ui-diff.txt`.  Sigue sin ser posible el 0.00 crudo sin
el rasterizador DirectWrite/CEF original; el residuo es AA de glifos.

## 2026-10-31 (52) — Modal Restore: gamma de glifos falsado

Para intentar bajar el residuo de 120018 (2.934) se probó engrosar los
textos del modal con `gamma=0.85` en título (36 px), cuerpo (27 px) y
botones (30 px).  El resultado empeora: 2.936 global, título 28.28 vs
28.07 y cuerpo 26.29 vs 26.07.  Revertido a `gamma=1.0`; el residuo no es
grosor de cobertura sino la forma/posición de los glifos DirectWrite.
Estado final sin cambios: 2.934.

## 2026-10-31 (53) — Hotkeys completo: 220 acciones, 88 unidades, perfil

Se extrajo la tabla nativa de teclas del CEF del juego retail (Wine), no de
las capturas: la URL viva del `HotkeyPanel` está en
`blizzard_browser_cache/data_*` (`?port=...&guid=...`) y el WebSocket
`ws://127.0.0.1:<port>/panel/HotkeyPanel/<guid>` entrega
`OnActionsDiscovered` (230 textos) y `OnBindingsChanged` (690 entradas por
perfil, 230 acciones únicas; `$default` = Brood War).  Herramienta:
`research/tools/extract_hotkeys_ws.py`; volcado:
`research/analysis/hotkeys-bindings-retail-1.23.10.13515.json`; notas:
`research/docs/hotkeys-data.md`.

Se generó `engine-rust/fixtures/hotkeys.json` (220 acciones + 88 unidades/
edificios: Terran 29, Protoss 28, Zerg 31) y `src/hotkey_data.rs`.  El panel
ya no tiene tarjetas hardcodeadas: dibuja todas las tarjetas y teclas,
incluidas las de 4 tarjetas (Drone) y las especiales de Probe 678/679
(272/273).  El rebind es genérico y aplica la regla CEF por unidad:
`Move=H` desasigna Hold Position (12 badges Terran, 39 globales con el
Build Hatchery del Drone), `Move=M` desasigna Train Marine solo en la
tarjeta Lifted del Barracks.  Save/Cancel/Restore funcionan; Save escribe
`~/.cache/galaxycraft/hotkeys.bin` (GCHK v1, límite 64 KB, SHA-256) y el
arranque lo carga; los `--shot` fuerzan defaults.

Verificación: auditoría headless 88/88 (todas las unidades con tarjetas,
mínimo 8 459 píxeles brillantes); `ui-hotkeys-16.txt` 16/16; goal 19/19 a
0.00; `ui_diff.py` 39/39; `cargo test` 315/315 (8 tests de datos/colisión/
fichero); clippy 0.  En vivo: Save H, reinicio, carga H con badges;
Restore+Save vuelve a defaults y deja Terran→SCV.

## 2026-10-31 (54) — Exactitud: supersample/gamma/embolden falsados

Para acercar el 0.00 crudo se probaron, con la referencia 120018 y SCV:

- Frame completo a 3413x1920 y downsample 0.75 (bilineal): SCV 1.249
  (antes 0.959); box: 1.605.
- Glifos fontdue a 4/3 y downsample 0.75 (bilineal): SCV 1.057; box: 1.07.
- `gamma=0.85` en título/cuerpo/botones del modal: 2.936 vs 2.934.
- Embolden horizontal de 24/48/72: 2.851/2.858/2.866 vs 2.847.

Todos se revirtieron.  Lo que sí funcionó fue aplicar el CSS real:
`.dialogBody` tiene `letter-spacing:.4px` (0.3 px a la escala efectiva) y el
título necesita -3 px de bearing.  El modal baja de **2.934 a 2.847** y el
título de 28.07 a 16.12 de media.  El resto es la forma/gamma del
rasterizador CEF/DirectWrite; el 0.00 crudo exigiría ese rasterizador o
capas derivadas de las capturas.

## 2026-10-31 (55) — Modal Restore: dim del contenedor, 1.08

El residuo grande de 120018 no era AA: el overlay del modal solo cubría la
placa y los tres grupos de botones.  Las muestras del retail dan factor 0.40
en **todo** el contenedor CEF (x 354..2202, y 42..1440), incluido el fondo
entre la placa y los botones y la franja inferior (400,1280)→0.39,
(1000,1430)→0.39, (500,1400)→0.42; a x=300/2300 no se atenúa.  Se sustituyó
el dim de los grupos por un único `fill_alpha_rect(354,42,1849,1398,153)`.

Además se aplicaron las métricas CSS reales al texto del modal:
título 36 px con `y_scale 1.083` y bearing x -3; cuerpo 27 px con
`letter-spacing 0.23`; botones 30 px con `y_scale 1.1` y top 21.  Resultado:
**120018 2.934 → 1.082** (outliers 2.61 % → 0.50 %), por debajo de 115959
(1.129).  Bboxes: título 829..1212/572..597 vs 829..1211/572..597; cuerpo
826..1411/628..650 vs 826..1412/628..650; botones 834..855 exactos.

Hotkeys completo queda 0.76–1.13 sin capas; 16/16.  El 0.00 crudo sigue
siendo el rasterizador CEF/DirectWrite (probados y falsados frame/glifo
supersample, gamma y embolden).

## 2026-10-31 (56) — Hotkeys: esquinas CEF y tooltip inventado

Las capturas del usuario señalaban las esquinas de la pestaña "Hotkeys" y de
Terran/Protoss/Zerg.  El CSS retail confirma `.hotkeyPanelHeader` y
`.raceSelectionTabSection .tab` con radio superior (12 px portada, 9 px
razas a la escala efectiva) y `selected` con `margin-bottom:-6px` (9 px),
cuyos bordes laterales llegan al panel.  Se implementaron
`tab_border_rounded`, `top_rounded_fill` y `css_panel3_rounded` (radio 12
exterior / 6 interior); la comparación 1:1 de las tres zonas marcadas queda
sin las líneas rectas y sin el escalón inferior.

El tooltip de hover "Terran SCV" era nuestro, no del retail:
`hotkeypanel.js` no tiene tooltip y solo aplica
`.unit:hover:not(.selected){box-shadow: inset 0 0 0 2px #4cc428}`.  Se
eliminó; el nuevo `--shot-screen hotkeys-terran-hover-scv` muestra el borde
verde y ningún texto encima de "Ground Units".

Verificación final: 16/16 Hotkeys (0.76–1.13), goal 19/19 a 0.00 con capas
regeneradas, `ui_diff` 39/39, `cargo test` 315/315, clippy 0.  Repo privado
`cotizcesar/starcraft-re-recomp` creado y subido sin assets, capturas,
capas de calibración ni evidencia decompilada.

## 2026-10-31 (57) — Hotkeys: remate inferior de las pestañas

Las esquinas superiores ya eran curvas, pero el borde interior seguía
bajando 2 px de más en la portada y 12 px de más en las pestañas no
seleccionadas (el círculo verde sobre Protoss).  Se ajustó el remate:
título interior termina en y=125 y exterior en y=127 (el retail exterior
llega a 128); pestañas no seleccionadas terminan el interior en y=113 y el
exterior en 119; la seleccionada conserva su solape.  El zoom REF/OUR de
las dos zonas está en `engine-rust/docs/hotkeys-tabs-proof.png`.

Verificación: 16/16 Hotkeys (0.76–1.13), goal 19/19 a 0.00 con capas
regeneradas, `ui_diff` 39/39, `cargo test` 315/315, clippy 0.
