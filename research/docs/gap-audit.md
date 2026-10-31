# Auditoría de paridad visual vs `docs/original_screenshots` (44 refs, 3440x1440)

Referencia: 44 capturas del cliente Remastered a 21:9. Nuestro cliente se
renderiza headless con `scripts/render_screens.sh` a la misma resolución y se
compara pantalla por pantalla.

## Hallazgo estructural (corregido)

Todas las subpantallas usaban el **collage clásico** (`glue/title/title.dds`)
como fondo. La referencia usa la **escena espacial Remastered** en todo el
front-end. Ahora `draw_menu_backdrop` reutiliza `draw_space_scene` (capas
WebM animadas en aditivo sobre nebulosa). Esto alinea de golpe menú,
opciones, multijugador, colecciones, hotkeys, saved/replays/LAN.

## Tabla de faltantes (estado tras la ronda del 2026-10-29)

| Ref | Pantalla real | Estado | Falta |
|---|---|---|---|
| 1-3, 16 | Menú principal | escena espacial + 8 entradas + marco + perfil + versión | planeta y nebulosa 3-D del fondo (geometría del motor), parallax, panel del marco vacío |
| 4, 28 | Modal "Select Game Type" | **[hecho]** `glue/palmm/retail_ex.dds` + etiquetas medidas (20.9) | — |
| 5 | Registry | **[hecho]** fondo `glue/palnl/backgnd.dds` (16.0) | marco metálico del panel |
| 6-12, 29 | Selección de episodio | **[hecho]** retratos WebM con alfa, anillos, etiquetas y botones (7.2-8.7) | marco metálico del borde |
| 13 | Saved Games | **[hecho]** fondo lunar (15.0) | panel grande del retail |
| 14, 15, 35, 36 | Create | **[hecho]** árbol + Replay Information + Ok/Cancel (10.0-10.7) | pestaña Maps y preview de mapa |
| 17-21 | Options (5 pestañas) | lista de texto sobre escena | filas con sliders/checks y arte por pestaña |
| 22-25 | Hotkeys (3 razas) | lista de texto | rejilla de iconos de unidad y Selected |
| 26 | Collections | lista de texto | rejilla de skins con miniaturas |
| 27 | Credits | panel | scroll de créditos sobre negro |
| 30-34, 37, 40-44 | Partida (Terran/Zerg/Protoss) | pillarbox, retrato pintado, iconos amarillos, wireframes | geometría exacta del plato HD (bloqueo declarado) y estado de mapa distinto |
| 32 | Game Menu | arte `gamemenu.ui.json` | alineación fina |
| 38 | Mission Objectives | panel | maquetación retail |
| 39 | Tips | panel | maquetación retail |

## Medición objetiva (44 referencias, 2026-10-29)

`scripts/compare_all.py` compara las 44 capturas y escribe
`research/docs/parity-report.md`. Métrica: diferencia absoluta media por canal
(0..255) entre nuestro render headless 3440x1440 y la captura retail.

| Momento | Media | Peor |
|---|---:|---:|
| Primera medición (mapeo grueso) | 26.57 | 44.37 (#4/#28 intro) |
| Tras corregir el mapeo y el modal | 25.26 | 31.26 |
| Tras Registry/Create/episodios | 19.72 | 31.26 |
| Tras fondo real del front-end | 15.74 | 31.26 |
| Tras créditos, registro y partidas por raza | 14.18 | 25.82 |
| Tras Tips retail, Options por pestaña y consola a escala | 13.58 | 25.70 |
| Tras Mission Objectives retail | 13.53 | 25.70 |
| Tras Saved Games y Collections medidos | 13.24 | 25.70 |
| Tras panel de Hotkeys por raza | 12.79 | 25.70 |
| Tras pozos de comando + placa oscura del HUD | 12.75 | 25.56 |
| Tras fondo real de créditos | 12.34 | 25.56 |
| Tras miniaturas reales de Collections | 12.31 | 25.56 |
| Tras marco y perfil del menú | 12.25 | 25.56 |
| Tras tipografía medida | 12.21 | 25.56 |
| **Ahora (Game Menu con las rects reales)** | **12.20** | **25.56 (#31, partida Terran)** |

Última ronda:

- `glue/palmm/backgnd.dds` resultó ser **el fondo exacto del front-end**
  (planeta + nebulosa). Menú 22.2 → **4.5**, Options 21.0 → 15.3,
  Hotkeys 22.9 → 12-19, Collections 23.2 → 13.4, modal 20.9 → **5.7**.
- **Créditos**: `creditsscreen.ui.json` da el panel (`glue/battlenet/credits_a.dds`
  a (120,30,420,450)) y `creditspanel.js` contiene la lista real de 4515
  entradas `{name,style}`; se dibuja con tamaños/colores del CSS (#b4fc74).
- **Registro** (ref 5): panel medido como el de Create → 16.0 → **9.4**.
- **BUG grande corregido**: todas las capturas de partida renderizaban el
  **menú** en lugar del mundo (la bandera `menu` seguía activa, así que
  `draw()` salía antes de dibujar el terreno; el área de juego medía 1.4 de
  brillo medio frente a 38 en la referencia). Ahora se dibuja terreno + niebla.
- **Partidas por raza**: las capturas cargan una misión de campaña de la raza
  correspondiente (Terran 02, Zerg 06, Protoss 06, elegidas por medición), así
  que la consola (gris/roja/azul) coincide con la referencia:
  ref 37  26.2 → **19.2**, refs 40-44  ~30 → **~22**.
- **Tips (ref 39)**: diálogo reconstruido con la maquetación medida en la
  captura (panel 0.278..0.732 x 0.066..0.597 del recuadro de contenido, título,
  tres líneas de cuerpo, casilla con marca roja y los botones OK / Previous
  Tip / Next Tip) sobre una misión Zerg → 25.2 → **17.5**.
- **Options (refs 17-21)**: panel, columna de pestañas, filas y botones
  Restore/Ok con las coordenadas medidas, y una captura **por pestaña**
  (Speed/Sound/Video/Game/Network) en vez de repetir la misma:
  15.0-15.5 → **11.2-11.9**.
- **Game Menu (ref 32)**: mis rects anteriores estaban mal medidas (las saqué de
  una miniatura). Medidas sobre la captura real: panel **0.340..0.744 x
  0.067..0.666**, siete botones de 72 px con paso 102 px desde y=0.142, título
  arriba. Con eso la maqueta coincide (antes el diálogo salía a la izquierda y
  más estrecho). 18.89 → **18.82**. El resto del error de esa captura es el
  terreno (base llena de unidades), no el diálogo.
- **Tipografía de las pantallas de menú**: medí el tamaño real del texto en las
  capturas (título de Collections 18 px de alto y ~15.7 px por glifo; filas de
  Options ~16 px) y nuestras etiquetas iban un paso de glifo por encima.
  Bajadas un paso en Collections (título, secciones, contadores, chip, EQUIP),
  Options (filas, tabs, botones) y Hotkeys (secciones, sub-etiquetas, tabs):
  Collections 15.04 → **14.70**, Options 11.03-11.33 → **10.52-11.16**,
  Hotkeys Terran 6.41 → **6.14**, media 12.25 → **12.21**.
- **Marco del menú compartido**: las capturas de subpantalla (Options,
  Hotkeys, Collections, episodios…) muestran detrás el **marco metálico del
  menú** y el **nombre de perfil** arriba a la derecha, que solo dibujábamos en
  el menú principal. Movido a `draw_menu_frame`, invocado desde
  `draw_menu_backdrop`: Options 11.2-11.9 → **10.6-11.3**, Collections 15.20 →
  **15.04**, media 12.31 → **12.25**.
- **Collections (ref 22)**: las miniaturas **sí están en CASC** como imágenes
  sueltas referenciadas por `webui/dist/collectionspanel/collectionspanel.js`
  (`default/blizzcon/krc_gold/krc_silver/war3spoils_console_thumbnail`,
  `skins_default/carbot/bonus`, `default/um/kim_announcer`). Se decodifican con
  `video::decode_still` (ffmpeg → RGB) y se dibujan en celdas de ancho fijo
  0.139 con paso 0.004 desde x 0.160, como en la referencia.
  **16.46 → 15.20.**
- **Créditos (ref 27)**: el fondo NO era geometría 3-D. Barrido de los **282
  DDS de `glue/`** comparando la zona visible alrededor del panel contra la
  captura: `glue/battlenet/backgnd.dds` puntúa **2.74** y el siguiente candidato
  25.98 — es exactamente la sala metálica del retail. Con ese fondo,
  **20.79 → 2.86**.
- **Contenido del HUD de partida**: se añadieron los **pozos de comando**
  (cuadros oscuros biselados de ~125x95 bajo cada icono, con bisel claro
  arriba/izquierda y oscuro abajo/derecha) y una **placa oscura de fondo** bajo
  toda la rejilla 3x3, porque el plato estirado deja metal claro donde el
  retail tiene negro. Efecto medido: Terran 25.58 → **25.44**, Zerg 19.38 →
  **19.07**, Protoss 20.35 → **19.99**.
- **Hotkeys (refs 23-26)**: panel completo reconstruido con las coordenadas
  medidas (placa 0.144..0.856 x 0.033..0.866, pestañas Terran/Protoss/Zerg,
  secciones Units/Buildings con rejilla de iconos por raza sacada del GRP de
  wireframes, panel derecho de comandos y Restore/Save/Cancel):
  23 **11.9 → 6.3**, 24 **12.9 → 8.1**, 25 **13.4 → 9.0**, 26 **13.1 → 7.8**.
- **Saved Games (ref 13)**: panel izquierdo con la lista, botón Delete
  debajo y Ok / Cancel abajo a la derecha, todo con coordenadas medidas sobre
  la captura → 15.0 → **5.23**.
- **Collections (ref 22)**: panel principal 0.141..0.601 x 0.045..0.836,
  columna de previsualización 0.618..0.863, chip "All Collections", las tres
  secciones (Console / Unit & Structure / Announcers) con sus celdas y el Ok
  medido → 19.2 → **16.5**.
- **Mapeo corregido**: la referencia 22 es **Collections**, no Hotkeys; las
  pestañas de Hotkeys son 23 (Terran), 24 (Protoss), 25 y 26 (Zerg).
- **Mission Objectives (ref 38)**: mismo plato medido que Tips
  (0.278..0.732 x 0.066..0.597), título, cuerpo y el botón "Previous (Esc)"
  en 0.366..0.599 x 0.525, sobre una misión Zerg → 18.6 → **16.5**.
- **Consola HD**: ahora los tres platos se escalan como un tríptico que cubre
  el ancho del contenido (alto resultante ≈331 px, referencia ≈345) en vez de
  escalarse por altura. El plato derecho baja de 1323 px a 1081 px; la
  referencia mide ~500, así que sigue sin converger, pero la composición ya
  cubre el ancho como el retail. Test nuevo: `console_triptych_spans_the_content_width`.

### Qué se corrigió en esta ronda (cada punto con su arte retail)

1. **"Select Game Type" (refs 4, 28): 44.4 → 20.9.**
   El arte del diálogo es `glue/palmm/retail_ex.dds` (marco metálico, borde
   rojo y **ambos logos**). Se dibuja como modal sobre el menú, que es lo que
   muestra la referencia, con las etiquetas medidas (título 0.076, Original
   0.388, Expansion 0.794, Cancel 0.908 de la altura del panel).

2. **Registro y partidas guardadas (refs 5, 13): 38.2/39.1 → 16.0/15.0.**
   Fondo `glue/palnl/backgnd.dds` (paisaje lunar), que es el que usa el retail
   en Registry / Saved Games / Create.

3. **Pantalla Create (refs 14, 15, 35, 36): 26.3/26.8 → 10.0/10.7.**
   Maquetación medida sobre la referencia: panel izquierdo con el árbol de
   Replays, panel derecho "Replay Information" y botones Ok / Cancel.

4. **Selección de episodio (refs 6-12, 29): 24.6-25.4 → 7.2-8.7.**
   Placa negra, los tres retratos de raza desde `glue/campaign/{terr,zerg,prot}.webm`
   decodificados **con canal alfa** (`decode_rgba_frame_at`, VP9 + BlockAdditional;
   sin alfa el fondo sale como basura), recortados a su caja visible y
   ajustados a 0.38 de la altura; anillos violetas con halo, etiquetas
   raza/episodio y los botones Load Saved / Play Custom / Load Replay / Cancel.

5. **Mapeo de referencias corregido.** La tabla anterior asignaba pantallas
   equivocadas: las refs 5-15 son Registry / episodios / Saved / Create (no
   "episode"/"mission"), 17-21 son las cinco pestañas de Options, 22-25 son
   Hotkeys, 26 Collections, 27 Credits, 28 el modal otra vez, 29 episodios con
   el diálogo de misión, y 30-44 partidas (con Game Menu en 32, Mission
   Objectives en 38 y Tips en 39).

### Lo que sigue dominando el error

Las diez referencias de partida (30-34, 37, 40-44) están en 24-31 y son ahora
el peor bloque, por dos razones distintas:

1. **Estado de partida distinto**: el terreno, las unidades y la cámara de
   nuestras capturas no coinciden con los de la referencia (no hay forma de
   saber qué partida exacta era), así que esa parte del error es irreducible
   sin replicar la partida. Las cuatro referencias Terran son la *misma*
   misión con la cámara en sitios distintos y solo tenemos un frame.
2. **Geometría del plato HD**: no existe layout en datos (solo DDS/GRP), es
   código del motor. Medición nueva sobre la ref 30 (3440x1440): la consola
   ocupa de y≈1095 a 1440 y el plato derecho va de x≈2760 a 3000, es decir
   ~240-500 px de ancho; el nuestro mide 1323 px porque el DDS se escala por
   altura. Sin las rects del motor no converge (intentos de bandas fijas ya
   empeoraron la métrica y se revirtieron).

Quedan por debajo del resto: Credits (21.8, sin scroll ni panel retail),
Hotkeys pestaña Terran (19.0) y Registry/Options (15-16, sin el marco
metálico de los paneles).

## Orden de ejecución

1. [hecho] Escena espacial compartida en todas las subpantallas.
2. [hecho] Pillarbox 16:9 a 21:9 (440..2999) + HUD sin etiquetas de texto,
   botón MENU, platos anclados y **wireframes de selección** en rejilla 4x3
   con nombre, stats, HP y "N selected".
   [pendiente] marco de botón (bisel), alineación y tamaño de la rejilla,
   retrato real, iconos con arte (hoy vectoriales).
3. Fondo `glue/pal*` para episodio y misión.
4. Opciones/Hotkeys/Collections con sus layouts.
5. Créditos con scroll.
6. Briefing con maquetación retail.
7. Intro reproduciendo vídeo.

## Ronda 36 (2026-10-29) — media 11.68 → 11.31

| Cambio | Evidencia |
|---|---|
| Minimapa a su posición retail (x 452..840, y 1040..1435) | medido en ref #30; antes 330 px a la derecha |
| Niebla de guerra en el radar (`draw_minimap_fogged`) | radar 33.8→30.1 (Hunters), 43.2→32.8 (Space Madness) |
| Mapa de captura elegido por barrido (Terran→Volcanis, Terran2→Space Madness) | área de juego ref #30: 24.24→20.05 |
| Contenido de las capturas sigue sin reproducirse | la ref es una base Terran completa; ningún mapa la tiene |

`compare_all.py`: media **11.31**, mejor 2.86, peor **21.52** (antes 11.68 / 24.36).

## Ronda 37 (2026-10-29) — media 11.31 → 11.24

| Cambio | Evidencia |
|---|---|
| Escena de referencia en `src/scene.rs` (29 unidades, 4 tests) | rejilla de depósitos medida por brillo verde: x 710/998/1288, y 620/812/1004 |
| Corrección de anclaje del sprite (+58 px) | columnas alineadas al píxel (709/997/1285 vs 710/998/1286) |
| Escena **opt-in** (`GC_SCENE=1`) | con escena 23.91 vs 23.31 sin escena, misma cámara y mapa |
| Blit bilineal del mundo | terran 21.52→21.19, zerg 18.14→17.83, protoss 18.70→18.52, terran2 15.71→15.58 |
| Zoom alternativo descartado | divisor 1/1.5/2/2.5/3 → 25.36/29.95/29.35/27.29/20.07 |

`compare_all.py`: media **11.24**, mejor 2.86, peor **21.19**.

## Ronda 39 (2026-10-29) — terreno HD; media 11.24 → 11.19

| Cambio | Evidencia |
|---|---|
| Formato `hd2/tileset/<era>.dds.vr4` recuperado | `[u32 total]` + `[flags][w][h][len][DDS DXT1 64x64]`; 6588 entradas verificadas |
| `render::load_hd_megatiles` + `Tileset::draw_tile` HD | terran 21.19→20.95, protoss 18.52→17.94 |
| Zerg/terran2 pierden (tileset ajeno a la referencia) | zerg 17.83→18.77, terran2 15.58→15.75 |
| Barrido de mapas repetido con HD | zerg mejor melee 20.47 y protoss 18.84: siguen ganando las campañas |
| `tests/hd_tileset.rs` | >6000 megatiles, ≥90 % decodificados, alfa presente |

`compare_all.py`: media **11.19**, mejor 2.86, peor **20.95**.

## Ronda 40 (2026-10-29) — unidades/consola HD: investigación y descarte medido

- Unidades HD no son GRPs: 807 `hd2/anim/main_NNN.anim` con capas DXT5
  (`diffuse/bright/teamcolor/emissive/...`), sin tabla unidad→animación localizada.
- Consolas HD (`hd2/game/consoles/bc2017|bc2018`) son otro skin: HUD 25.21 → **32.71**
  en la ref #30. Revertido (render idéntico a shots68, diferencia 0.000).

`compare_all.py`: media **11.19**, mejor 2.86, peor **20.95**.

## Ronda 41 (2026-10-29) — captura en vivo alineada al píxel

| Cambio | Evidencia |
|---|---|
| Reafirmar fullscreen del WM cada segundo | `credits` en vivo 11.04 → **3.84**; en vivo vs headless 13.62 → **0.00** |
| Las 11 pantallas en vivo igualan al headless | diferencias 0.00–0.08 |
| Índice unidad→animación HD: descartados | `arr/images.tbl` sin `.anim`; `anim/main.lit` con registros `55 00…` sin nombres; `skins.json` sólo skins |

`compare_all.py`: media **11.19**, mejor 2.86, peor **20.95** (sin cambios; el
arreglo afecta a la captura en vivo, que ahora coincide con el render).

## Ronda 42 (2026-10-29) — contenedor ANIM HD descifrado

- `src/hd_anim.rs` + tests: cabecera `ANIM` v0x0202, 7 capas por nombre (32 B
  cada una), directorio de 6 atlas `[off][size][w][h]` de 512x444 (DXT5/DXT1),
  `decode(layer)` a RGBA.
- Falta la tabla de rectángulos por frame (0x14c declara 85 frames y caja
  196x182, pero el u32 de 0x150 no es un offset válido: 11 927 748 > 1 139 196)
  y el índice unidad→`main_NNN`.

`compare_all.py`: media **11.19**, mejor 2.86, peor **20.95**.

## Ronda 43 (2026-10-29) — tabla de frames ANIM decodificada

- Localizada al final del fichero: `1 139 196 - 1 137 836 = 1 360 B = 85 × 16 B`.
- Registro `[u16 x,y,w,h][u16 orig_x,orig_y,flags,0]`; coordenadas en escala HD
  (1024x888) sobre un atlas de 512x444 → se dividen por 2.
- Verificado: los 85 rectángulos caen dentro del atlas y tienen píxeles
  (464-579 px opacos). `AnimFile::frames` + `decode_frame()` con 1 test nuevo.
- `main_000` es una animación de una pieza que gira (no un sprite completo).

`compare_all.py`: media **11.19**, mejor 2.86, peor **20.95**.

## Ronda 44 (2026-10-29) — `sc_animsheet` y primer barrido de animaciones

- Nueva herramienta `sc_animsheet` para volcar hojas de frames de `hd2/anim/*`.
- Barrido 000-011: atlas de tamaño variable por fichero (main_002 = 480x240) y
  número de atlas 1..6; main_005/006 tienen 204 frames (17x12).
- Interpretación alternativa del registro de frame descartada (rect fuera del
  atlas: y 212 + 168 > 240).

`compare_all.py`: media **11.19**, mejor 2.86, peor **20.95**.

## Ronda 45 (2026-10-29) — índice de 807 animaciones HD; dos hipótesis falsadas

- 807 contenedores extraídos (416 MB); 702 parsean; CSV de firmas en `/tmp/anim_index.csv`.
- 121/702 con `frames % 17 == 0`; firmas dominantes: (1 atlas) 431, (1 atlas,%17) 72, (6 atlas) 68.
- Falsado "dirty rects": la unión acumulada ocupa el atlas completo (485x431 de 512x444).
- Falsado "tabla de frames siempre en 0x14c": aparecen celdas de 32764 px en main_004/main_011.

`compare_all.py`: media **11.19**, mejor 2.86, peor **20.95**.

## Ronda 46 (2026-10-29) — clasificación por atlas; detector retirado

- Probado `detect_frame_table` (registros 16/12/8 B, mayor tabla que quepa): **no converge**.
- Causa medida: el campo `size` del directorio de atlas no es el tamaño de la
  superficie (428 → declara 227456; 227884 → 227456; 455340 → 113792).
- El tamaño real debe calcularse de la cabecera DDS (DXT1 = w*h/2, DXT5 = w*h).
- Detector retirado; 278 tests en verde.

`compare_all.py`: media **11.19**, mejor 2.86, peor **20.95**.

## Ronda 47 (2026-10-29) — regla general de la tabla de frames

- Extensión real del atlas calculada de la cabecera DDS (DXT1 8 B/bloque, DXT5 16 B),
  no del campo `size` (que resulta no ser el tamaño de la superficie).
- `frames = (len - fin_último_atlas) / 16`: coincide en las dos clases
  (main_000 85, main_002 7, main_004 9, main_005 204, main_011 23).
- Residuo: 2/9 y 4/23 rectángulos mal leídos; hipótesis i16 en el primer campo
  (32764 = -4, 32755 = -11).
- Test nuevo: exacto en main_000, ≥75 % en el resto (279 tests).

`compare_all.py`: media **11.19**, mejor 2.86, peor **20.95**.

## Ronda 48 (2026-10-29) — extensión con signo en los frames

- Campos 3-4 del registro son **i16**: 65529=-7, 65501=-35, 65495=-41.
- Normalizando `x0 = x+min(0,w)`, `w = |w|` y dividiendo por 2: main_000 85/85,
  main_002 7/7, main_004 8/9, main_005 204/204, **main_011 23/23** (antes 19/23).
- Residuo a escala de corpus: 784/6 519 rectángulos fuera del atlas máximo (12,0 %)
  en 114 ficheros; hipótesis: el rect se refiere a su capa, no al atlas mayor.

`compare_all.py`: media **11.19**, mejor 2.86, peor **20.95**.

## Ronda 49 (2026-10-29) — "atlas por capa" refutada; el escaneo de superficies está incompleto

- 137 ficheros: **todos** con atlas del mismo tamaño por capa → la hipótesis no explica el 12,5 %.
- Escala: 1/2 acierta 87,5 %; 1:1 sólo 20,6 %.
- El escaneo está completo: main_009 = 428 + 286 848 + 3 808 = 291 084 B exactos.
- El residuo es de **espacio de coordenadas**: los frames piden 560 px de alto
  (y 496 + h 153) sobre una superficie de 1024x280.

`compare_all.py`: media **11.19**, mejor 2.86, peor **20.95**.
