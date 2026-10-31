# Inventario del juego (Fase Dos)

`GAME_PATH`: `/home/cotizcesar/Games/battlenet/drive_c/Program Files (x86)/StarCraft`
(~5.5 GB, build 1.23.10.13515). Solo lectura; verificado 2026-10-28
(`.build.info` sha256 coincide con el manifiesto).

## Manifiestos

- `original-manifest/inventory.csv`: 724 archivos (cabecera + 724 filas),
  con ruta relativa, tamaño, extensión, tipo `file`, sha256, clasificación,
  observaciones. Copia de `analysis/file-inventory.csv`.
- `original-manifest/sha256.txt`: 724 hashes.
- Total: 5 879 150 935 B.

## Composición por extensión/clase

| Extensión/clase | Archivos | Notas |
|---|---|---|
| `.scm` / `.scx` | 163 / 160 | mapas MPQ clásicos (`staredit\scenario.chk`) |
| `.htm` (locale CEF) | 128 | UI del lanzador, no datos del juego |
| `.pak` | 118 | recursos CEF del lanzador |
| `.idx` / `.index` | 48 / 47 | índices CASC (clasificados `desconocido`, formato conocido) |
| `.dll` / `.exe` | 20 / 11 | nativos Win32 (juego + lanzador + CEF + SDK) |
| `.bin` | 4 | datos del motor |
| `data.000…005` | 6 | ~5.4 GB, contenedor CASC |
| resto | ~20 | `.db`, `.info`, `.json`, `.js`, `.chm`, `.dat`, sin extensión |

## Herramientas de identificación

`find` + `file` + `sha256sum` (+ `objdump` para PE). Sin descompresión
indiscriminada; CASC/MPQ se leen con `src/retail.rs` y extractores C++.

## No reconocidos

`analysis/discovery/unrecognized-files.csv`: 239 filas con
`clasificacion_provisional == desconocido`. Composición: 128 `.htm` +
48 `.idx` + 47 `.index` + resto menor. Los `.htm` son locale CEF (formato
conocido, contenido secundario); los `.idx`/`.index` son índices CASC
(formato conocido por estructura del contenedor). Quedan como "no prioritarios",
no como formatos misteriosos: ver `docs/unresolved-questions.md`.
