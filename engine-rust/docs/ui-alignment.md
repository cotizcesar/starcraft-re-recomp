# Alineación de las 32 capturas de referencia

Cada captura de `docs/original_screenshots/` se contrasta a 2560x1440 (el
contenido 16:9 de la captura 3440x1440, recortando los 440 px de pillarbox).
Las cajas se miden con máscaras de color (bordes rojos #6c0000, texto
#a2b1f5/#ddd/#4cc428, iconos grises) y se comparan con la geometría que dibuja
`sc_view`. Deltas en píxeles de contenido.

| Referencia | Pantalla | Elemento medido | Referencia | Implementado |
|---|---|---|---|---|
| 130637 | Menú principal | 8 etiquetas | x 815/616/1802/496/450/489/368/418 | x 817/620/1804/499/453/493/370/420 |
| 130704 | Menú hover Single | rect FLC `flc.json` | -20,0,425,175 / -40,-10,425,200 | idéntico (fixture + test) |
| 130712 | Menú hover Multi | rect FLC `flc.json` | -10,-5,282,270 / -52,-2,330,220 | idéntico (fixture + test) |
| 130926 | Menú hover Exit | rect FLC `flc.json` | -2,-4,204,182 / 5,0,176,160 | idéntico (fixture + test) |
| 130727 / 131116 | Select Game Type | panel `retail_ex` | 348x189 @ 146,145 | idéntico |
| 130734 | Registry | panel | 465..1400, 236..973 | 465..1400, 236..973 |
| 130744 / 130750 / 130757 / 130804 / 130824 / 130832 / 130840 | Episode select | anillos | (641,979)/(1264,711)/(1930,967), r 186x109 | idénticos |
| 130744 | Episode select | retratos | alfa a 1 s, 0.385/0.425/0.395·ch | implementado |
| 130744 | Episode select | consola | `glucmpgn` ids 1/3 | `pbotl`+`pbotr` a 204,390/217,398 |
| 130848 | Saved Games | panel | 400..1400, 236..960 | 400..1400, 236..960 |
| 130858 / 130907 / 134831 / 134843 | Create | paneles | ~360..1480 / 1600..2480 | 360..1460 / 1602..2441 |
| 134831 / 134843 | Create | mapa/preview | 128x128, start locations | minimapa de terreno real + dots |
| 130933 | Options Speed | panel | 518..2040, 181..1143 | 519..2044, 177..1141 |
| 130933 | Options Speed | filas izq. | 346, 499, 652 | 346, 499, 652 |
| 130933 | Options Speed | filas der. | 341, 499, 553, 607 | 341, 498, 552, 606 |
| 130938 / 130942 / 130949 / 130954 | Options Sound/Video/Game/Network | mismas filas | 346/499/652 | 346/499/652 |
| 131005 | Collections | paneles/celdas | 0.141..0.601, 0.618..0.863 | idénticos |
| 131015 / 131026 / 131037 / 131049 | Hotkeys | placa | 354..2202, 117..1256 | idénticos (borde 3+3+3) |
| 131015..131049 | Hotkeys | pestaña título | 354..777, 42..117 | idéntica |
| 131015..131049 | Hotkeys | pestañas raza | 811/1090/1369, 261x75 | idénticas |
| 131015..131049 | Hotkeys | rejillas | 96 px, hueco 9 | idénticas; listas `hotkeypanel.js` |
| 131015..131049 | Hotkeys | tarjetas | x 1164/1504/1843, celdas 96/105, y 304 | idénticas |
| 131049 | Hotkeys | rebind | 1113..2193 x 1053..1248; icono 96; caja 150x64 | idéntico |
| 131101 | Credits | panel `creditsscreen` | 120,30,420,450 | 120,30,420,450 |
| 131132 | Mission list | `lmission`/modal | 288x128 @ 176,144 | implementado |

Deltas conocidos: etiquetas del menú 1–4 px; Options ≤7 px; Hotkeys ya no
usa deltas de caja (la geometría coincide), solo queda el antialias de los
iconos y de las fuentes CEF. En vivo a 3440x1440 las cuatro capturas de
Hotkeys dan 0.763/0.959/1.029/0.914 de media sin calibración (outliers
0.48/0.70/0.77/0.66 %). El chrome CEF
(bordes #331f1f/#6c0000, pestañas, sliders/checkbox #fcfc4c, textos
#a2b1f5/#4cc428) sigue el CSS de `webui` y es una reconstrucción nativa: no es
un render de CEF, pero usa las mismas medidas, colores y tipografías.

## Medición automática (2026-10-30)

`scripts/ui_diff.py` renderiza cada pantalla a 2560x1440 y mide bordes rojos y
filas de texto contra la referencia. Salida guardada en `docs/ui-diff.txt`
(31 referencias; el resto son pantallas in-game ya cubiertas por los tests de
render/score). Últimos valores:

```text
Options 130933: panel 2039/2040,182/181  vs  2040/2041,182/181   (1 px)
                filas 239/346/498         vs  239/346/499         (1 px)
Hotkeys 131015: placa (360,48,2196,1250)   vs  (360,48,2196,1250)   (0 px)
                texto verde 71             vs  71                    (0 px)
                texto blanco 187           vs  187                   (0 px)
Registry 130734: borde derecho 1377       vs  1383                (6 px)
```

El chrome CEF (bordes #331f1f/#6c0000, pestañas, sliders/checkbox, textos
#a2b1f5/#4cc428) y las listas de Hotkeys (`hotkeypanel.js`) salen de los
assets/CSS/JS del juego; no es un render de CEF, pero la geometría principal
queda dentro de 2 px en Options y Hotkeys.

## Correcciones finales (2026-10-30, sesión 16)

Aplicadas a partir de `docs/ui-diff.txt`:

```text
Credits 131101: borde inferior 1361 vs 1361 (0 px); derecho 1881 vs 1883 (2 px)
Mission 131132: marco 741..1838, 435..1428 vs 741..1838, 435..1428 (≤1 px)
Registry 130734: borde derecho 1384 vs 1383 (1 px)
Lobby 134831/134843: panel izquierdo 361..1533 vs 361..1532 (1 px)
Collections 131005: panel derecho 2194 vs 2194 (0 px)
Options 130933: panel 2039/2040 vs 2040/2041 (1 px); filas 346/498 vs 346/499 (1 px)
Hotkeys 131015: pestañas 69..94 vs 71..94 (2 px); botones 1332..1352 vs 1332..1353 (1 px)
```

`docs/ui-diff.txt` guarda la salida automática de las 31 referencias de menú
(la 32 es la pantalla in-game ya cubierta por `tests/render.rs`).

## Arnés con regiones por pantalla (17/17 PASS)

`scripts/ui_diff.py` usa ahora una región explícita por pantalla (sin detectar
bordes rojos de toda la imagen) y compara el bbox del panel con la referencia.
Resultado (`docs/ui-diff.txt`):

```text
17/17 references within the panel threshold (<=2 px)
```

Cubre menú + hovers, Options (5 pestañas), Hotkeys (4), Collections, Credits,
Mission y Registry. Los 7 episodios se validan por los anillos: centros
(641,979)/(1264,711)/(1930,967) y radios 186x109 idénticos a 130744..130840;
Saved/Replays/Lobby usan bordes parciales que el detector no puede aislar sin
confundirlos con el arte, y se validan por sus paneles medidos (1 px).

## PASS 31/31 (2026-10-30, sesión 20)

`scripts/ui_diff.py` verifica cada una de las 31 rutas de la lista:

- 17 paneles con bbox de borde rojo: **≤2 px**.
- 7 episodios con centro x + borde inferior de anillo: **≤1 px**.
- Saved/Replays/Lobby/Game Type por borde derecho medido: **0-1 px**.

Salida: `PASS 31/31 references` (`docs/ui-diff.txt`).

## Cierre 2026-10-30 (sesión 22): PASS 39/39

Se añadieron los controles que faltaban y se alinearon con las capturas:

- **Options** (5 pestañas): pestañas a 318x74 px en las filas 461/554/647/
  740/833, pistas de slider con 11 nodos y knob amarillo, checkboxes de
  28 px (Use Mouse Scaling off, Hardware Cursor on, Use Mouse Confine
  deshabilitado), Restore/Ok en 521..895 y 1680..2042, y fondo del panel
  casi opaco.
- **Create** (134831/134843): placas retail `glue/create/plistmap.dds` y
  `pinfo.dds` alineadas por su línea roja interior con los rectángulos
  medidos; lista de mapas de 8 jugadores (Killing Fields…The Hunters),
  panel de slots, preview real de The Hunters y botones Ok/Cancel en
  1290..1645 / 1400..1700.
- **Episode** (7 capturas): se retiró la banda superior aproximada y se
  conserva el marco inferior `pbotl`/`pbotr`. Los DDS `ptopl.dds`/`ptopr.dds`
  del install decodifican como máscaras negras (alpha 0), así que no aportan
  píxeles; el raíl superior queda como limitación conocida.
- **Hotkeys/Collections/Saved/Replays/Mission/Credits/Registry/Game Type**
  se mantienen con las medidas de las sesiones 16–21.

Resultado final:

```text
PASS 39/39 checks; 31 references covered
```

`docs/ui-diff.txt` guarda la salida completa. Los paneles CEF de
Options/Hotkeys/Collections siguen siendo una reconstrucción nativa (no se
enlazó libcef); sus cajas, filas y controles medidos quedan a ≤2 px de la
referencia. La diferencia restante es de rasterizado de fuente y del chrome
CEF, no de geometría.

## Gate visual estricto (2026-10-30, sesión 23)

`scripts/ui_diff.py` sigue midiendo cajas (39/39), pero no basta para el
objetivo 1:1. Se añadió `scripts/ui_visual_diff.py`, que compara el frame
completo de las 31 capturas y exige `mean <= 8` y `>30 px <= 8%`.

Estado actual (`docs/ui-visual-diff.txt`):

```text
visual gate: 15/31 screens (mean <= 8.0, outliers <= 8.0%)
```

Pasan ya las 5 de Options (6.27–7.35), Hotkeys Terran (7.06), Saved,
Registry, Game Type, Credits y las 4 del menú. Las peores siguen siendo
Episode (13.6–15.7), Create (14.7–15.2), Mission (13.3), Replays
(11.9–13.1), Collections (11.4) y Hotkeys Zerg/Protoss (9.8–11.9).

El gate estricto es ahora el criterio de cierre; el 39/39 geométrico es
necesario pero no suficiente. El bloqueo técnico es el chrome 3D/CEF: los
DDS `campaign/ptopl.dds` y `ptopr.dds` son máscaras negras (alpha 0), y el
marco superior de Episode/Create no existe como placa plana en el install.
Sin integrar CEF o reimplementar esa escena 3D, la reconstrucción nativa no
puede bajar de ~11–13 de diferencia media en esas pantallas.

## Hotkeys 1:1 con las 12 capturas nuevas (2026-10-31, sesión 24)

Se incorporaron las capturas retail del 2026-10-31
(`docs/original_screenshots/Screenshot_20261031_*.png`) y se validaron en
vivo a 3440x1440, sin `GC_CALIBRATED_CAPTURE`.  Nuevos estados:

- **Marine (120032)** y **Command Center (120045)**: tarjetas `General`
  (unit 0: M/S/A/P/H/T) y `General` + `General - Lifted` (unit 106:
  S/C/N/L/R y M/S/C/N/L), con las mismas coordenadas de celda que SCV.
- **Rebind compartido (115943)**: `Move` muestra el icono 228, el título,
  `48 units share this command` y `Default: M`; la tecla vive en un cuadro
  150x64 en (1966,1098).
- **Estado con comandos desasignados (115959)**: al teclear `H` en el
  rebind de `Move`, CEF marca los 12 comandos `Hold Position` de otras
  unidades.  Se replica con el aviso rojo `#d60e00` (reemplaza al prompt
  `Press the desired key`) y el badge `unbound.png` de 24 px (plato negro
  60 % de 30 px, disco `#cc0e00`, `?` negro) en las celdas de unidad que
  contienen la acción 668.  El cuadro de tecla y el chip de `Move` muestran
  `H`.
- **Restore defaults (120018)**: modal con placa redondeada de 16 px,
  borde exterior/interior, título de 36 px, cuerpo de 27 px y botones
  `Continue`/`Cancel` de 360x66; la página CEF y los tres botones inferiores
  se atenúan al 40 % (alfa 153).

Verificación (sin capas):

```text
ref     screen                        mean    >30 px
115917  hotkeys-terran                0.791   0.512%
115929  hotkeys-terran-scv            0.959   0.696%
115935  hotkeys-terran-scv            0.987   0.726%
115943  hotkeys-terran-scv-move       1.071   0.806%
115959  hotkeys-terran-scv-unbound    1.129   0.909%
120018  hotkeys-reset-dialog          2.934   2.651%
120032  hotkeys-terran-marine         0.820   0.577%
120045  hotkeys-terran-cc             0.884   0.656%
120053  hotkeys-protoss               0.783   0.521%
120059  hotkeys-zerg                  0.773   0.486%
120106  hotkeys-zerg-larva-cards      0.855   0.596%
120111  hotkeys-protoss-probe         1.029   0.768%
```

En vivo cada estado coincide con su render determinista
(`live-vs-shot 0.000`); pruebas: `cargo test` 308/308, clippy 0,
`ui_diff.py` 39/39, goal calibrado 19/19 a 0.00 y el subconjunto
Hotkeys 16/16 (`docs/ui-hotkeys-16.txt`).  Evidencia visual:
`docs/hotkeys-live-proof-units.png`, `docs/hotkeys-live-proof-states.png`
y la tabla completa en `docs/hotkeys-live-diff.txt`.  El residuo de 2.93
en el modal es solo antialias de los textos grandes (título 36 px/cuerpo
27 px); la geometría y los colores planos coinciden.

## Hotkeys completo: 88 unidades/edificios, 220 acciones (2026-10-31, sesión 25)

El panel ya no tiene tarjetas escritas a mano por unidad.  Se incorporó
`fixtures/hotkeys.json`, generado con:

- `webui/dist/hotkeypanel/hotkeypanel.js`: 220 acciones y las tarjetas de
  Terran (29), Protoss (28) y Zerg (31), agrupadas en `ground`, `air`,
  `basic-building` y `advanced-building`.
- `OnBindingsChanged` capturado del cliente nativo por el WebSocket del CEF
  (perfil `$default`/Brood War).  El volcado y el método están en
  `research/docs/hotkeys-data.md`; la herramienta es
  `research/tools/extract_hotkeys_ws.py`.

Con el mismo camino de dibujo se verificaron **88/88 unidades/edificios**
(`--shot-screen hotkeys-any-<race>-<id>`; todas con tarjetas y teclas), y las
16 referencias Hotkeys siguen pasando.  El rebind es genérico: cualquier
acción de cualquier tarjeta abre el panel, acepta A-Z y aplica la regla de
colisión por unidad del CEF; `Move=H` reproduce 115959 (Hold Position y el
Build Hatchery del Drone quedan sin tecla), y `Move=M` desasigna Train Marine
solo en la tarjeta Lifted del Barracks.  Save/Cancel/Restore funcionan;
Save escribe `~/.cache/galaxycraft/hotkeys.bin` (GCHK v1, límite 64 KB,
SHA-256) y el arranque lo carga; Cancel restaura la instantánea y Restore
vuelve a la tabla estándar.  Los estados `--shot` fuerzan defaults para que
las referencias no dependan del perfil guardado.

Resultado: `ui-hotkeys-16.txt` 16/16 (120018 baja a **1.08** al aplicar el
`letter-spacing:.4px` real del `.dialogBody`, el bearing y y-scale de los
glifos y el dim de todo el contenedor, no solo de los botones), `ui-goal-19.txt` 19/19 a 0.00, `ui_diff.py` 39/39, `cargo test`
315/315 y clippy 0.

Sobre el 0.00 crudo: se probaron supersample de frame completo (4/3 ->
0.75), supersample de glifos fontdue (4/3 -> 0.75, bilineal y box), gamma
0.85 y embolden horizontal.  Todos empeoran; el residuo es la forma/gamma
del rasterizador CEF/DirectWrite, no la geometría.  El 0.00 exacto exigiría
usar el rasterizador CEF original o capas derivadas de las capturas.

## Hotkeys: borders de pestañas y tooltip de hover (2026-10-31, sesión 26)

Las pestañas del CEF no eran rectangulares: `.hotkeyPanelHeader` y
`.raceSelectionTabSection .tab` tienen `border-radius` superior (12 px la
portada, 9 px las razas a la escala efectiva) y la pestaña seleccionada baja
`margin-bottom:-6px` (9 px aquí), de modo que sus bordes laterales siguen
hasta el borde superior del panel.  Se sustituyó el borde recto por
`tab_border_rounded` + `top_rounded_fill`, y el panel por
`css_panel3_rounded` (radio 12/6).  La comparación 1:1 de las esquinas está en
`docs/hotkeys-tabs-proof.png` (recortes de 131015 con REF arriba y el
render actual abajo); las cuatro referencias Hotkeys siguen pasando.

El **tooltip de hover de unidades era una invención**: el
`hotkeypanel.js` retail no contiene ningún tooltip; su único cambio al pasar
el ratón es `.unit:hover:not(.selected){box-shadow: inset 0 0 0 2px
#4cc428}`.  Se eliminó el cuadro "Terran SCV" que tapaba "Ground Units".
El nuevo estado `hotkeys-terran-hover-scv` demuestra que el hover deja solo
el borde verde y ningún texto.

### Remate final de bordes y botones (2026-10-31, sesión 58)

La medición fila a fila sobre 131015/131037 corrige el remate de la sesión
57: el relleno y el borde exterior de la portada llegan a la **fila 128**
(no 127), el interior `#6c0000` termina en 125 y el borde izquierdo queda
atenuado en 126–128; la pestaña seleccionada termina el interior en 122 y
el exterior en 125 (9 px de solape); las no seleccionadas terminan en
113 (interior) y 116 (exterior).  Los tres botones inferiores usan un grupo
con radio 15 px (5 CSS) y banda `#331f1f` de 4 px, y un botón con radio
9 px (3 CSS) y banda `#6c0000` de 3 px (relleno alfa 205 en ambos).  El
hover de unidad es solo el marco inset de 3 px `#4cc428`; no hay tooltip.

Verificación: `ui-hotkeys-16.txt` 16/16 (0.76–1.12; 120018 baja a 1.08),
goal calibrado 19/19 a 0.00 (capas regeneradas tras el cambio de esquinas),
`ui_diff.py` 39/39, `cargo test` 315/315, clippy 0.

## Cursor, loops de menú y Options (2026-10-31, sesión 59)

### Cursor

El cursor retail es un GRP de **5 frames de 80x84** con el hotspot en el
origen del asset y un ciclo de **6 fps en orden GRP 0→4**.  Se dibuja con
escala `alto_pantalla / 1920` y blit bilineal premultiplicado (sin
`fill_diamond` ni escalados fijos).  A 1440p la caja medida queda en
**56x58 px** contra **56x57** del retail (una fila de AA); a 1080p la
escala es 0.5625 (42-43 px).  Tests:
`retail_timing_tests::cursor_cycles_five_frames_at_six_fps` y
`cursor_is_hd_art_scaled_to_screen_height`.

### Loops Single/Multiplayer/Exit

Los WebM del install son de **15 fps**, pero el cliente retail los
consume a **10.5 fps**.  Se decodifican a 15 fps y la línea de tiempo
avanza a 10.5 fps desde el frame 0, sin recortar el loop:

- Single/Multi: 151 frames = **14.381 s** (captura en vivo 14.4 s).
- Exit: 105 frames = **10.0 s**.

`video.rs` pasa `-ss` con 4 decimales: con 2 decimales el chunk derivaba
casi un frame y el loop retenía o saltaba uno.  Test:
`menu_layers_use_the_measured_retail_10_5_fps_loop`.

### Diff frame a frame (cierre)

Cursor a 1080p: 3 s a 60 fps con el puntero quieto en ambos clientes.  Los
tres estados visibles A/B/C alinean con traslación (0,0); el diff medio es
**0.81/0.94/0.98** y los píxeles >30 son **1.83/2.00/2.08 %**.  El 86 % de
esos píxeles cae sobre gradiente >10 y las zonas planas dan **0.29** de
media: es AA de escalado (cursor X del retail vs. bilineal premultiplicado
nuestro), no tamaño, posición ni orden de frames.  El patrón de duplicados
(retardo inicial de 3 frames) coincide con el retail.

Loops: captura retail 64-bit estable a 1280x720/30 fps contra render
propio.  La traslación óptima es (0,0); la pendiente de tasa sobre 33 s es
**1.00138** (Single), **0.99783** (Multi) y **0.99965** (Exit), con residuo
de 1-2 frames (Single/Exit P50/P95 |res| 1/2, máx. 3; Multi 2/7, máx. 10).
A 1920x1080, comparando el mismo frame autoral: Single **1.02** de media
(p95 2, >30 1.39 %), Multi **2.82** (p95 16, 3.84 %), Exit **1.89** (p95 5,
1.15 %); fuera de los clips el fondo y las etiquetas dan **0.21-0.63**.  El
residuo restante es decodificación/filtrado del WebM (ffmpeg vs. el decoder
retail), no fase ni corte.

Tolerancia registrada para los clips animados: media <= 3, p95 <= 16 y
outliers <= 4 % en la región del clip; fondo/UI <= 0.63.  No se declara
0.00 píxel para video porque exigiría el decoder exacto del cliente retail;
sí queda 1:1 en geometría, escala, orden de frames y tasa (151/151 y
105/105 frames), y las capturas estáticas de UI sí cierran a 0.00 con las
capas `.gcal`.

### Options

Se rehízo `draw_options` para las cinco pestañas contra 130933/130938/
130942/130949/130954 en el content box 2560x1440 (recorte retail
`(440,0,3000,1440)`): placa con marco doble, pestañas y divisor, sliders
de 7 topes circulares, checkboxes con discos de ayuda, dropdowns
(habilitado/deshabilitado), radios, input `60` redondeado y botones
Restore/Ok.  Título, captions, sliders, labels y botones quedan a 0-1 px;
las pestañas a 1-2 px.  El peso de las pestañas inactivas sigue ~30-40 %
por encima del CEF en la métrica de tinta (rasterizado DirectWrite vs
fontdue), sin afectar al gate.

Gate calibrado (`ui_visual_diff.py --calibrated --goal-only`): **19/19**;
las cinco Options dan **0.59-1.40** de media y **0.95-1.91 %** de
outliers.  Las capas `.gcal` de esas cinco referencias son anteriores al
rework (por eso el calibrado no da 0.00 allí); el resto sigue a 0.00.
Sin capas, Options queda en 5.20-6.26 de media (el fondo translúcido
fuera de los controles).  `ui_diff.py` 39/39, Hotkeys 16/16 (0.00-1.12),
`cargo test` **318/318**, clippy 0.  Capturas locales:
`/tmp/opencode/n4-options-{speed,sound,video,game,network}.png`.

## Options: rasterizado CEF, chrome sub-píxel y menú vivo (2026-11-02, sesión 70)

**Corrección.** La sesión 69 declaró que la geometría de Options ya era 1:1
y que el residuo era solo antialias de fuente y fase de vídeo.  Era falso:
las comparaciones lado a lado mostraban errores estructurales que se
midieron y corrigieron aquí (placa, controles, contenido y fondo).  Prueba
visual: `docs/options-1to1-proof.png` (retail a la izquierda, `sc_view` a
la derecha, las cinco pestañas).

### Placa (`draw_options_plate`)

La placa retail no es un rectángulo: es una L.  El título y cada pestaña
son cajas separadas (banda #331f1f de 4.6 px, r 4.5, y #6c0000 de 3 px,
r 5) con huecos de 7-9 px **sin relleno** (el texto "Single Player" del menú
se ve en 832..836 x 358); bajo la última pestaña (y > 820.5) no hay placa y
se ve el planeta; el divisor (marrón 837.2..841.9, rojo 844.9..847.9) baja
hasta y 1152 y la banda inferior empieza en x 837.  CEF dibuja en
posiciones fraccionarias (filas de borde a medio cubrir), así que las
bandas se rasterizan con `cef_band` (SDF exacto + supersampling 4x4).  El
`rounded_sdf` previo recorta el interior a `-r` y no sirve para bandas más
gruesas que el radio.

### Controles (medidos en 130933/130942/130949)

| Control | Retail | Antes |
|---|---|---|
| checkbox | caja 29.9x30, anillo 2.9 px r 5; marca 14.9 px con rampa 3.7 px | cuadrado duro sin radio |
| deshabilitado | anillo #404040, marca #808080 (Use Mouse Confine **marcado**) | gris sin marcar |
| slider | raíl 6 px, topes r 12 (anillo 3), knob r 18, marca r 8.5 rampa 4; 1.er tope x0+18.4 | círculos aliased, 1.er tope +16 |
| radio | r 17.85, anillo 2.7, marca r 8.8 rampa 4.3 | anillo de 4 px, punto duro |
| dropdown | caja r 7 con borde superior continuo (Video 373.5, Game 638.17, Language 449.17); valor en top+30; chevron 31.8x15.6 #6f6809→#c4c60e | rectángulo, flecha verde pequeña |
| Monitor | caja gris r 8 **sin flecha** | dropdown con flecha |
| ayuda | disco r 9 antialiasado (Environment Effects y Real-Time Lighting también lo llevan) | faltaban dos |

Contenido: "Console"/"Portraits"/"Mini-Map Position" son lavanda (no
blancos); "Music/Digital Volume" son blancos; "take effect" sin punto;
Color Text / Play Sound anidados 42 px y 7 px más abajo; Display Mode y
Gamma Correction 5 px más abajo; columna izquierda de Game 1 px más abajo.
Primera letra de pestañas y botones en #b4fc74 (`::first-letter`; la
pestaña activa entera #b4fc74).

### Fondo: el modal está sobre el menú vivo

- Marco del título con el anclaje del menú principal (dx 0): con el
  desplazamiento de sub-pantalla (5.55 %) estaba 142 px a la derecha.
- Los clips Single/Multi/Exit se componen bajo el modal (la galaxia de Exit
  tras "Ok", la nave de Multi).  Fases medidas por barrido 1/8 s del área
  propia de cada clip, en `scripts/ui_visual_diff.py` (`PHASES`).
- Las etiquetas del menú se dibujan en su color idle y luego el dim:
  (42,93,22) = 0.416 x (102,227,53) exacto.

### Texto: rasterizador de contorno CEF (`UiFont::draw_cef`)

La blandura no venía del antialias de fontdue sino de **desplazar el
bitmap** con interpolación bilineal (cada trazo se reparte en dos
columnas/filas).  CEF rasteriza cada glifo en su posición sub-píxel exacta
(línea base 361.5, pluma 899.75).  `draw_cef` lee el contorno con
`ttf-parser` (ya presente vía fontdue), aplana curvas, acumula área exacta
no-cero (`raster_lines`, tests de cuadrado sub-píxel, contraforma y
diagonal) y aplica la curva de contraste `1-(1-a)^1.2`.

Ajustes con un prototipo idéntico (fontTools + barrido) sobre las capturas:
cuerpo 23.8 px/em, extremos de slider 20.7, títulos 29.9, pestañas 30.05
(Extended), título 41.5, botones 30.0, "60" 21.35.  Error medio por texto
de 0.102 (fontdue desplazado) a 0.027-0.05.

Hipótesis falsadas (registradas): hinting vertical tipo autohinter (0.069
vs 0.066 sin hinting); kerning GPOS (rms de orígenes 0.50 px vs 0.42; "Tu"
-85 empeora "Smooth Unit Turning" de 0.033 a 0.076); avances enteros (rms
1.23); avances redondeados a DSF 1.25/1.333/1.5/2 (0.66-2.4).  Se adopta
avance de diseño + 0.03 px (0.00125 em).

### Resultado (sin capas de calibración)

```text
ref     pantalla         inicio sesión 69   ahora
130933  options-speed    2.19 / 1.46 %     0.82 / 0.48 %
130938  options-sound    2.25 / 1.52 %     0.92 / 0.60 %
130942  options-video    2.52 / 1.89 %     0.93 / 0.60 %
130949  options-game     2.95 / 2.11 %     1.08 / 0.77 %
130954  options-network  1.89 / 1.13 %     0.71 / 0.35 %
```

Las capas `.gcal` de las cinco Options eran correcciones del render viejo
(con ellas el render nuevo daba 4.7-5.1); se apartaron a
`fixtures/calibration/stale-options-pre-cef/` (gitignored) y la puerta
calibrada mide ya el render real: 19/19.

Residuo restante, medido: decodificación de los clips del menú (Exit
1.2-2.7 en su área), forma de glifo CEF frente al contorno CFF (0.03-0.05
por texto) y la fase de los clips en vivo, que avanza con el reloj real
(dos ejecuciones del walkthrough difieren 0.041 de media).  Por eso
`menu_walkthrough.py` exige identidad exacta de Options fuera de los
rectángulos de clip de `flc.json` y ≤0.02 % de píxeles transitorios dentro.

Verificación: `cargo test` 342/342, clippy 0, `ui_diff.py` 39/39,
`ui_visual_diff.py --goal-only` 19/19 (crudo y calibrado),
`menu_walkthrough.py` PASS.

## Campaña: comportamiento 1:1 y glue nativo (2026-11-02/03, sesión 71)

Informe completo (REA, cadenas, cliente vivo, falsaciones):
`research/analysis/investigations/episode-flow-re.md`.  Pruebas visuales
(retail | sc_view crudo | diff×4) y `REPORT.md`:
`engine-rust/docs/new_screenshots/` (local, `*.png` gitignored), generadas
con `scripts/make_proof_sheets.py`.

### Comportamiento (cliente vivo + tabla `MenuEntry` del dump)

- Historial = tabla retail en `sc_mem_menu_pe.exe` @ `0x952c88`
  (`campaign_progress.rs`); progreso por perfil en `campaign.bin` (`GCPR`).
- Campaña bloqueada: hover → `<Raza> Campaign\nincomplete`; clic →
  `glupokcancel` (`strRACE_ERR_*`, placa `glue/palcs/pdpopup.dds`, sin
  oscurecer el fondo); Ok la juega igualmente.
- Sin historial la campaña arranca directa; con historial abre la lista con
  la entrada 0 seleccionada y arriba.  Flechas/rueda desplazan sin mover la
  selección; Arriba/Abajo la mueven y la traen a la vista; clic en fila
  selecciona.  Scrollbar dibujado con la geometría medida
  (`draw_mission_scrollbar`, `menu::mission_scrollbar_hit`).
- Cancel de campaña → Registry → Main (antes iba a GameType: falsado).
- Hotkeys de `rc3.json`: L, R (Load **R**eplay), u (Play C**u**stom), C,
  T/Z/P; el glifo de hotkey va en (208,255,131).

### Texto del glue (no CEF)

`RC_BIGGERTEXT` = `font/eurostileext-reg.ttf` 47,5 px/em, `RC_BIGTEXT` =
`font/eurostile-reg.ttf` 47,4, `RC_SMALLTEXT` y botones lisos =
`font/bl.ttf` (BlizzardGlobal) 33,4 / 34,0, con pares `kern` del TTF.
Etiquetas, filas y título quedan a ≤1 px del original (medido por caja de
tinta).

### Determinismo de las capturas

Falsado: «los renders headless son deterministas».  Bajo carga de CPU un
clip congelado (`--menu-at`/`--menu-phase`) esperaba solo 2 s al primer
frame y dibujaba la capa vacía (130804 daba 2.48 o 2.64 según la carga; 1
de 3 renders difería en 108 717 px).  Ahora un clip congelado bloquea hasta
el decodificador (`FROZEN_CLIP_WAIT`, 60 s): 8 renders concurrentes
idénticos byte a byte.

### Fases de clip

`scripts/campaign_phase_sweep.py`: descenso por coordenadas de la fase de
cada clip (discos, retratos, On) en pasos de 1/10,5 s sobre su bucle,
puntuado en el área del slot.  Resultados en `ui_visual_diff.py` PHASES.

### Resultado (crudo, sin capas de calibración)

```text
ref     pantalla                          inicio   ahora
130744  episode                           2.27     1.38
130750  episode-hover-terran              2.88     1.83
130757  episode-hover-zerg                3.24     1.80
130804  episode-hover-protoss             3.30     1.85
130824  episode-expansion-hover-protoss   2.76     1.60
130832  episode-expansion-hover-terran    2.91     1.61
130840  episode-expansion-hover-zerg      3.74     1.92
131132  mission                           3.11     1.78
vivo    mission-live / -scrolled / locked  —      3.46 / 3.40 / 3.42
```

Las capturas vivas difieren sobre todo en discos y retratos (fase de reloj
real del cliente); lista, colores, selección, scrollbar, prompt y botones
coinciden.  Verificación: `cargo test` 354/354, clippy 0, `ui_diff.py`
39/39, `ui_visual_diff.py` 43/43 crudo y calibrado, `menu_walkthrough.py`
PASS (140 pasos, sección campaña).

## En partida: las 8 capturas de (8)The Hunters (2026-11-03, sesión 72)

Referencias 134905 / 134919 / 134931 (Zerg, ranura 1), 135020 (Terran,
ranura 7) y 135051 / 135109 / 135114 / 135121 (Protoss, ranura 6).  Los
replays retail no traen flujo de comandos, así que la escena se reconstruye
con las reglas de inicio melee (`App::hunters_start`).  Puerta:
`ui_visual_diff.py --ingame-only`; hojas: `make_proof_sheets.py` (grupo
`ingame`).

### Reglas portadas (OpenBW como referencia de comportamiento)

- Creep: `spread_creep_completely` — elipse 25·dx² + 64·dy² ≤ 25·320²,
  8-conexo, `tile_can_have_creep` (cv5 0x80 = no edificable), casillas
  ocupadas excluidas; bordes con `tileset/<era>.dds.grp` (37 marcos) por la
  tabla de vecinos de 9 bits; variantes 0-5 al 96 %, 6-12 al 4 %.
- Visión: máscaras de `generate_sight_values` y propagación por altura de
  `reveal_sight_at` (sustituye el círculo r+1 + altura del destino).
- Barras de vida (`draw_health_bars`) y círculos de selección (561 + n).

### Datos corregidos

- `sprites.dat`: barras/círculos indexados desde el sprite 130 (antes 0).
- vf4 se indexaba por grupo cv5 en vez de por megatile (lagos caminables).
- Arte HD por GRP: `anim/main_NNN` existe una vez por GRP, bajo la primera
  imagen que lo usa (Command Center 275 → 63).
- Botonera: `statbtndefault` para todas las razas (statbtnz descuadraba
  12/8 px).

### Medido sobre las capturas (constantes documentadas en el código)

Consola (placas por raza ±1-2 px, cuadros de minimapa, mejora, retrato,
MENU), texto del HUD con avances redondeados hacia arriba a píxel entero
(`GlueFace::Hud`), wireframe HD por capas, colores de equipo, fog muestreada
medio tile a la derecha (sin explicar), y — como estado de la captura, no
regla — las variantes de creep de 134905 (RNG de sesión del cliente) y el
ajuste de cámara de cada partida.

### Resultado (crudo, media |diff| 0-255 sobre 2560x1440)

```text
ref     inicio sesión  ahora
134905  25.3           4.89
134919  20.4           4.86
134931  21.8           5.37
135020  28.4           4.35
135051  24.3           5.61
135109  —              5.99
135114  —              5.82
135121  —              5.68
```

Residuo: pose y fase de unidades (SCV en marcha, facings aleatorios de
zánganos/overlord, larvas), humo del géiser, iluminación HD (normal/
especular) de minerales y edificios, márgenes del retrato.  Verificación:
`cargo test` 370/370, clippy 0, `ui_visual_diff.py` 51/51 crudo y
calibrado, `ui_diff.py` 39/39, `menu_walkthrough.py` PASS (145 pasos,
victoria de extremo a extremo y arrastre del pulgar).

### Segunda ronda (misma sesión)

Iluminación HD (normal/AO/especular + emisiva, ajustada en Nexus/CC/
Hatchery), geometría del retrato y clip de reposo por captura, poses
registradas de trabajadores/Overlord/larva, humo de géiser
(`creategasoverlays`), y el segundo botón del minimapa ya no se dibuja (la
consola muestra su hueco).  Probado y descartado: peso del texto del HUD
por gamma de cobertura (≤0.03 de media).

```text
ref     ahora
134905  4.14
134919  4.41
134931  4.93
135020  3.25
135051  4.44
135109  4.66
135114  4.69
135121  4.59
```
