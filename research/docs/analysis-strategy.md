# Estrategia de análisis (Fase Cuatro)

Motor: código nativo (C/C++ propietario) + datos CASC/MPQ. Ruta: **nativa con
REA+Ghidra para código; lectores dedicados para datos**. Sin Unity/Unreal/
Godot/.NET/Java/web que justifiquen otros proveedores.

## Verificación por formato (representantes)

| Formato | Representante | Herramienta | Resultado |
|---|---|---|---|
| PE x86 | `StarCraft Launcher.exe` (copia `/tmp`) | REA+Ghidra 12.1.4 | PASS: 20 862 proc, 13 326 strings, base `0x400000` |
| PE x86 juego | `x86/StarCraft.exe` (en disco, cifrado) | `file`/`objdump`/strings | PE válido, `.text` entropía ~8.0 → requiere dump |
| CASC | `Data/data/data.000` + índices | `src/retail.rs` / casc_extract | tablas/tiles/GRP/WAV leídos |
| MPQ mapas | `Maps/*.scm` (323) | `src/retail.rs` / mpq_extract | 323/323 descomprimidos y parseados |
| iscript/aiscript | `scripts/*.bin` vía CASC | `src/iscript.rs`, `src/aiscript.rs` | opcodes 0x00..0x44 + AI completa |
| CHK triggers | `staredit\scenario.chk` | `src/trigger.rs`, `src/map.rs` | condiciones 0..23, acciones 1..59 |

## Alcance

- Nativo: funciones, xrefs, llamadas, símbolos, cadenas, tipos, estructuras,
  pseudocódigo (Ghidra). Punto de partida: imagen descifrada (pendiente
  recaptura; el dump 2026-10-27 ya no está en disco).
- Datos: tablas DAT, TBL, GRP, tilesets, WAV/Ogg, CHK (ya decodificados en
  `~/Documents/galaxycraft/`; reutilizar, no reinventar).
- No instalar: IDA, Hopper (ausente y no necesario), ILSpy/dotNet (sin
  objetivo), proveedores web/Java.

## Límites

- Sin dump vigente no hay análisis estático nuevo del juego (bloqueo
  documentado, no fracaso de herramientas).
- Pseudocódigo ≠ fuente original; nombres inferidos se marcan.
