# Identificación del motor — evidencias

Estado: **motor propietario Blizzard (StarCraft 1.23 “classic”)** — hipótesis
principal con evidencia convergente; no se descarta que el ejecutable de 64 bits
sea una variante del mismo engine recompilada.

## Evidencia observada

| # | Observación | Comando / fuente | Interpretación |
|---|---|---|---|
| E1 | `x86/StarCraft.exe` es `PE32 i386`, 12 820 096 B, 7 secciones | `file`, `objdump -h` | Binario nativo Win32 x86, sin runtime administrado |
| E2 | `x86_64/StarCraft.exe` es `PE32+ x86-64`, 15 734 912 B, 8 secciones (con `.pdata`) | `file`, `objdump -h` | Segunda variante nativa de 64 bits |
| E3 | `.text` con **entropía 8.000** en ambos binarios; `.rdata` en texto plano | cálculo propio sobre offsets de sección | El código está **cifrado/comprimido en disco**; los datos no |
| E4 | Cadenas en claro: `scripts\iscript.bin`, `scripts\aiscript.bin`, `glue\battle.net\`, `font\BLIZZARD-REGULAR.ttf`, `us.actual.battle.net` | extracción de strings | Nombres de recursos y del propio motor clásico (iscript/aiscript son formatos de StarCraft) |
| E5 | `Data/data/data.000…005` (~5,4 GB) + `*.idx` + `Data/indices` + `Data/config` | `find`, `du`, `.build.info` | Almacenamiento **CASC** de Blizzard, no MPQ en disco |
| E6 | `Data/config/44/...` contiene `# Patch Configuration` con `patch-entry = download <hash> <size> …` | lectura del archivo | Metadatos de CDN/parcheo de Blizzard |
| E7 | `.build.info`: `Version = 1.23.10.13515`, `Product = …`, CDN `level3.blizzard.com` | lectura | Build exacto identificado |
| E8 | `Maps/*.scm` (163) y `Maps/*.scx` (160) | inventario | Mapas en formato MPQ clásico con `staredit\scenario.chk` |
| E9 | `StarCraft Launcher.exe` + `x86*/cef/` + `SceneCefBrowser.exe` | inventario | La **UI de lanzamiento** es Chromium Embedded Framework; el juego no |
| E10 | Sin rastros de `UnityEngine`, `il2cpp`, `UnrealEngine`, `GodotEngine`, `mscorlib`/`BSJB` en los ejecutables del juego | búsqueda de marcadores | Descarta Unity (Mono/IL2CPP), Unreal, Godot y .NET para el juego |

## Hipótesis y contraste

- **H1 (principal): motor propietario de StarCraft, C/C++ nativo x86 + x86-64, recursos vía CASC.**
  Evidencia: E1–E9. Explica los formatos (iscript/aiscript/CHK), el lanzador CEF y la
  ausencia total de marcadores de motores comerciales.
- **H2: build “Remastered-adjacent” con doble binario (32 y 64 bits) y código protegido.**
  Evidencia a favor: dos ejecutables, ambos con `.text` de entropía máxima. A favor del
  mismo engine: las cadenas de recursos son idénticas en ambos.
- **H3 (descartada): Unity/Unreal/Godot.** Sin marcadores; además los formatos de datos
  son los de StarCraft clásico.
- **H4 (parcial, no del juego): Electron/CEF en el lanzador.** Solo `SceneCefBrowser.exe`
  y `*.pak` de CEF; es la capa de UI/menú, no el motor del juego.

## Consecuencia técnica crítica

El código está cifrado en reposo (E3). Por tanto:

1. Desensamblar `on-disk` con Ghidra produce basura (entropía 8.0 no es código).
2. El camino válido es una **instantánea de solo lectura de la imagen en memoria**
   del proceso en ejecución (`/proc/<pid>/mem` vía Proton), o localizar la rutina de
   descifrado. El trabajo previo en `~/Documents/galaxycraft` ya hizo esto: el proceso
   de 32 bits se mapea en `0x79fa0000` y se obtuvo una imagen de 17 620 992 B con
   `.text` descifrado y analizable.
3. No se ha intentado eludir ninguna protección: solo se lee memoria del propio
   proceso legítimo del usuario, en su máquina, para análisis.

## Siguiente experimento que discrimina

Con la imagen volcada, `mcp__rea__binary_overview` sobre el dump debe devolver
funciones con nombres de unwind/PDB y no basura. Si el dump también sale con entropía
8.0, la copia se tomó antes del descifrado (o desde una región equivocada) y hay que
repetir la captura tras la inicialización del motor.
