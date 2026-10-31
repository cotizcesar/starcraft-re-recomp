# Plataforma y runtime

## Plataforma objetivo

- Windows PE (subsystem GUI 6.00), Win32 x86 **y** Win64 x86-64.
- Ejecución real en Bazzite Linux a través de Lutris / Battle.net (Wine/Proton).
- Resolución de dependencias: `D3DX9_42.dll` + `D3DCompiler_42.dll` (DirectX 9),
  `ClientSdk.dll` (Battle.net SDK), CEF en el lanzador.

## Mapa de ejecutables

| Ruta | Bits | Tamaño | Papel |
|---|---:|---:|---|
| `x86/StarCraft.exe` | 32 | 12 820 096 | Juego clásico; el objetivo de reconstrucción |
| `x86_64/StarCraft.exe` | 64 | 15 734 912 | Variante de 64 bits del mismo cliente |
| `StarCraft Launcher.exe` | 32 | 5 004 152 | Lanzador (CEF/UI, login Battle.net) |
| `x86/cef/SceneCefBrowser.exe` | 32 | 2 560 640 | Proceso de render de la UI |
| `x86*/ClientSdk.dll` | 32/64 | 12,4 / 16,8 MB | SDK online de Blizzard |
| `x86*/BlizzardError.exe` | 32/64 | 893 384 | Reportero de errores |
| `x86*/ClientSdkFirewallHelper.exe`, `ClientSdkMDNSHost.exe` | 32/64 | ~600–840 KB | Auxiliares de red del SDK |

## Almacenamiento de datos

- `Data/data/data.000 … data.005` (~1 GB por archivo), `Data/data/*.idx` (48 índices),
  `Data/indices` (47 archivos): contenedor **CASC** (formato moderno de Blizzard).
- `Data/config`, `Data/ecache`, `Data/s1`: configuración, caché de CDN y estado local.
- `Maps/`: `.scm`/`.scx` clásicos — contenedores MPQ con `staredit\scenario.chk`.
- `locales/enUS/`: 132 archivos de localización.

## Implicaciones para la reconstrucción

1. El código de juego se analiza sobre una imagen de memoria descifrada, no sobre el PE.
2. Los datos (tablas `units.dat`, `weapons.dat`, `flingy.dat`, `orders.dat`, `iscript.bin`,
   `aiscript.bin`, …) se extraen del CASC con las herramientas ya existentes
   (`~/Documents/galaxycraft/tools/casc_extract.cpp`), preservando originales.
3. La equivalencia funcional se mide contra el binario de 32 bits ejecutado con Proton,
   que es el que el trabajo previo ya instrumentó.
