# Gráficos — estado real (2026-10-28)

Cliente: `engine-rust/src/bin/sc_view.rs` + `src/render.rs`, `src/dds.rs`,
`src/ui_art.rs`, `src/video.rs`. Todo el arte sale de la instalación del
usuario (`--install`); el repo no contiene arte retail.

## 1. Soporte de resolución

| Requisito | Estado | Prueba |
|---|---|---|
| 16:9 hasta 4K | **hecho** | renders 1280x720 / 1920x1080 / 2560x1440 / 3840x2160, exit 0 |
| Ultrawide expande (no bandas) | **hecho** | render 3440x1440, exit 0 |

Modelo: **la escala se deriva de la altura** (`scale = height / 480`), el HUD
4:3 se centra y el **mundo ocupa todo el ancho**. Así 16:9 y 21:9 muestran
más mundo a la misma proporción de sprite, en lugar de letterbox.

El mundo se dibuja en un framebuffer lógico (640x360 a 1x) y se escala con
`blit_scaled` al tamaño de ventana; el HUD se dibuja después en espacio de
pantalla. El ratón se convierte con `to_logical` antes de resolver mundo.

## 2. Bugs corregidos

### 2.1 Huecos negros en el terreno (corregido)

`draw_tile` pintaba solo el megatile del MTXM. CV5 guarda además un
**megatile de fondo por grupo** (`+10`), y los megatiles por tile son en gran
parte transparentes (acantilados, bordes, espacio). Sin esa capa se veía
negro a través.

- Antes: 2 de 698 tiles distintos de `Boxer.chk` no pintaban nada y decenas
  cubrían solo 30–90%.
- Después: **0 vacíos, 0 parciales**.
- Regresión cubierta por `tests/terrain_layers.rs`
  (`every_shipped_map_tile_is_fully_opaque`).

### 2.2 Sprites diminutos frente al HUD (corregido)

El mundo se dibujaba 1:1 mientras el HUD se escalaba: a 720p los sprites
salían a la mitad de tamaño relativo. Ahora ambos comparten `scale`.

### 2.3 Clics desalineados al escalar (corregido)

`to_logical` no estaba cableado: con escala ≠ 1 se seleccionaba la unidad
equivocada. Ahora los dos puntos de conversión ratón→mundo pasan por él.

## 3. Menú principal Remastered

Composición (imagen final: `menu Remastered`, 1920x1080):

| Capa | Origen | Tratamiento |
|---|---|---|
| Fondo espacial | gradiente + campo de estrellas determinista | el original es una **escena 3D** del motor, no una textura |
| Flota + crucero lejano | `glue/mainmenu/single.webm` | frame a 1,5 s, **mezcla aditiva** |
| Crucero cercano | `glue/mainmenu/multi.webm` | idem |
| Portal de Exit | `glue/mainmenu/exit.webm` | idem |
| Marco metálico | `glue/mainmenu/titleframe_overlay.dds` | DDS con alfa, arriba a la derecha |
| Entradas | texto verde (Single/Multi/Options/Collections/Hotkeys/Credits/Intro/Exit) | posiciones del layout de referencia |
| Perfil | perfil local + `#1532` | arriba a la derecha |
| Versión | `1.23.10.13515 \| enUS \| 64-bit` | abajo a la derecha |

Hallazgo clave: esos WebM **no son botones de texto**, son las capas
animadas del menú (van sobre negro, `yuv420p` sin alfa) y el motor original
las compone en aditivo. Eso es lo que se implementó (`blit_additive`).

## 4. Animación y HUD a ancho completo

- Las tres capas del menú se decodifican **en secuencia** (6 frames a 6 fps,
  un solo proceso ffmpeg por capa, `decode_frames`) y se ciclan con
  `MenuLayers::pick`. Los frames con destello de explosión a pantalla
  completa se descartan (`drop_flash`, luminancia media > 96) porque en
  aditivo lavaban el menú.
  Prueba: `tests/menu_layers.rs::menu_layer_yields_distinct_frames`.
- El HUD ya anclaba los platos de consola a los bordes (`draw_plates`:
  izquierda a x=0, derecha a `w - ancho`, centro centrado), así que en
  21:9 ocupa todo el ancho; lo que se centra es el contenido interactivo.
- Corregido un duplicado: el shot `gamemenu` dibujaba el panel procedural
  encima del arte retail.

## 5. Límites conocidos (honestos)

- Sin parallax: las capas se animan pero no se desplazan con el ratón como
  en el cliente real.
- Los frames del menú se muestrean a 6 fps (no 30) para no decodificar
  cientos de imágenes al arrancar.
- **Falta el planeta** de la esquina inferior izquierda: no aparece como
  textura en el CASC que hayamos mapeado (forma parte de la escena 3D).
- El panel del marco sale vacío: en retail muestra el rango/retrato.
- El HUD es arte 4:3 (640x480) centrado; no se estira a 21:9 como en SC:R,
  que recompone las consolas para llenar el ancho.
- Sin filtrado bilineal: escalado nearest, igual que el original pixel-art.

## 6. Cómo verlo

```bash
# Menú (1920x1080)
sc_view --install "/ruta/StarCraft" --width 1920 --height 1080 \
        --shot /tmp/menu.bmp --shot-screen menu
# Partida (4K)
sc_view --install "/ruta/StarCraft" --width 3840 --height 2160 \
        --shot /tmp/game.bmp --shot-screen objectives
# Interactivo
sc_view --install "/ruta/StarCraft"
```
