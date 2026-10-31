# Heap en partida — hallazgos verificados (2026-10-28)

Captura: `work/heap_ingame_20261028_1237/` (483 regiones RW, 897 MB).
Proceso: `x86_64\StarCraft.exe -launch -uid s1`, PID 80639, misión
**Terran 8 "The Big Push"** (Episodio I) en curso.
Método: `tools/heap_capture.py` (pread de `/proc/<pid>/mem`, sin depurador,
sin escritura en el proceso).

## 1. El mapa SÍ está cargado en heap (HECHO)

| Texto | Región | Offset |
|---|---|---|
| `Eliminate the Confederate forces.` | `0xad22000` (126 MB) | `0x432fbf1` |
| `Duke must survive.` | `0xad22000` | +0x2e |
| `THE BIG PUSH` | `0xa942000` | — |

Los objetivos del briefing viven en `.data` del módulo, y el bloque STR del
mapa se parseó a heap. Antes (dumps solo-imagen) esto daba 0 hits: **el
re-volcado de RW era el experimento correcto**.

## 2. DESCARTA: TRIG-2400 crudo y columnas DAT (HECHO)

0 candidatos válidos en 483 regiones. Los positivos del scoring laxo eran
ceros de BSS y basura (verificado a mano: condiciones y acciones todas 0).

## 3. HALLAZGO PRINCIPAL: tablas de punteros construidas en runtime

Tres tablas de punteros a `.text` que están **a CERO en la imagen en disco**
y **pobladas en memoria**:

| Tabla (módulo +) | Entradas | Disco | Memoria |
|---|---|---|---|
| `0x11b9b78` | 97 | `0x0` | `0x6ffffb343eb0` |
| `0x118f970` | 84 (57 deltas < 0x400) | `0x0` | `0x6ffffb343eb0` |
| `0x11afef0` | 63 | `0x0` | `0x6ffffb7e9180` |

Las dos primeras empiezan por el **mismo** puntero (`0x6ffffb343eb0`),
compatible con un handler por defecto compartido.

**Por qué esto importa:** el barrido estático de 13 tablas de salto y de
todas las referencias en `.rdata`/`.data` no podía encontrarlas — en el
fichero no existe la tabla, solo el espacio reservado. Esto cierra la
pregunta de por qué el despachador "no aparece" en estático.

## 4. Tabla de despacho de secciones CHK en 64 bits (HECHO, nueva)

`C6` en `f=0xae1940`, layout 64b `nombre(4) | ptr64 | flags(4)`, 30
entradas. Handlers recuperados (VA absoluta, base `0x6ffffade0000`):

```
ERA  0x6ffffb138520   OWNR 0x6ffffb138440   SIDE 0x6ffffb138360
STR  0x6ffffb1387f0   SPRP 0x6ffffb139660   FORC 0x6ffffb139690
VCOD 0x6ffffb1396d0   TRIG 0x6ffffb1388d0   CRGB 0x6ffffb138600
MBRF 0x6ffffb138c60   MTXM 0x6ffffb139410   THG2 0x6ffffb1386a0
UNIT 0x6ffffb138d50   MASK 0x6ffffb139590   UNIS 0x6ffffb1399d0
UPGS 0x6ffffb139cc0   TECS 0x6ffffb13a200   PUNI 0x6ffffb13a5b0
UPGR 0x6ffffb13a810   PTEC 0x6ffffb13aae0
(+ UNIx/UPGx/TECx/PUPx/PTEx opcionales)
```

## 5. FALSADO: los handlers de sección no están en tablas runtime

Barrido de los 20 handlers en las 483 regiones: **1 hit** (el propio TRIG,
en su tabla C6 estática). Conclusión: el despacho de secciones es la tabla
C6 de `.rdata` (estática), no una tabla runtime.

## 6. Experimento diferencial con pausa (HECHO, 2026-10-28)

Objetivo: aislar qué se escribe al emitir una orden, con la simulación
congelada.

| Paso | Captura | Notas |
|---|---|---|
| A | `work/heap_pauseA` | pausa (Esc) |
| A2 | `work/heap_pauseA2` | pausa, sin acción → **suelo de ruido** |
| B | `work/heap_pauseB` | pausa tras 1 orden de movimiento |

Resultados medidos (`tools/heap_diff.py`):

| Comparación | Bytes cambiados | Punteros nuevos a `.text` |
|---|---:|---:|
| A → A2 (ruido) | 11 462 468 | **13** |
| A → B (1 orden) | 16 019 944 | **69** |

**Lectura:** el diferencial funciona (5× sobre el ruido con una sola orden),
pero el heap sigue muy dinámico incluso en pausa (11 MB): hay buffering de
render/streaming activo. Los 69 punteros nuevos se reparten por regiones
`0x59exxxx`/`0x5aexxxx` (pares a 0x100000 de distancia con **valores
idénticos**, compatible con doble buffer, no con objetos de unidad).

**Negativo verificado:** las tres tablas runtime de §3 **no cambian** al
emitir una orden (0 bytes en T97/T84/T63/T32). Se inicializan al cargar
(vtables de clase), no son el despacho de órdenes.

**Conclusión:** el método está validado pero el experimento necesita menos
ruido (aislar el proceso del render, o instrumentar con Frida en vez de
diff de heap) para localizar el handler de orden. Coste/beneficio actual
bajo: la reconstrucción no depende de ello (las 189 órdenes ya están
implementadas y testadas en `engine-rust`).

Scripts: `tools/heap_capture.py`, `tools/heap_diff.py`.

