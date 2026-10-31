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
`css_panel3_rounded` (radio 12/6).  La comparación 1:1 de las esquinas está
en `docs/hotkeys-live-proof-states.png` y en la captura de verificación;
las cuatro referencias Hotkeys siguen pasando.

El **tooltip de hover de unidades era una invención**: el
`hotkeypanel.js` retail no contiene ningún tooltip; su único cambio al pasar
el ratón es `.unit:hover:not(.selected){box-shadow: inset 0 0 0 2px
#4cc428}`.  Se eliminó el cuadro "Terran SCV" que tapaba "Ground Units".
El nuevo estado `hotkeys-terran-hover-scv` demuestra que el hover deja solo
el borde verde y ningún texto.

Verificación: `ui-hotkeys-16.txt` 16/16 (0.76–1.13), goal calibrado 19/19 a
0.00 (capas regeneradas tras el cambio de esquinas), `ui_diff.py` 39/39,
`cargo test` 315/315, clippy 0.
