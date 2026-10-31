# Plataforma y runtime

## Plataforma objetivo

Windows PE GUI 6.00, Win32 x86 **y** Win64 x86-64. Ejecución en Bazzite vía
Lutris/Battle.net (Proton). Dependencias: `D3DX9_42.dll`, `D3DCompiler_42.dll`,
`ClientSdk.dll` (Battle.net SDK), CEF solo en lanzador.

## Ejecutables

| Ruta | Bits | Tamaño | Papel |
|---|---|---:|---|
| `x86/StarCraft.exe` | 32 | 12 820 096 | juego clásico; objetivo RE |
| `x86_64/StarCraft.exe` | 64 | 15 734 912 | variante 64 bits |
| `StarCraft Launcher.exe` | 32 | 5 004 152 | lanzador CEF/login |
| `x86/cef/SceneCefBrowser.exe` | 32 | 2 560 640 | render UI lanzador |
| `x86*/ClientSdk.dll` | 32/64 | ~12/17 MB | SDK online |
| `x86*/BlizzardError.exe` | 32/64 | 893 384 | reportero |

## Datos

CASC (`Data/data/data.000…005`, 48 `.idx`, 47 `Data/indices`), `Data/config`,
`Maps/` (323 `.scm`/`.scx` MPQ con CHK), `locales/enUS/`.

## Implicaciones

Estático sobre imagen de memoria descifrada (PE en disco cifrado); datos vía
`src/retail.rs`; equivalencia contra binario 32 bits con Proton.
Detalle histórico: `docs/archive/platform-and-runtime.md`.
