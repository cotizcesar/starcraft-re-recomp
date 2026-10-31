# Mapa de arquitectura — StarCraft 1.23.10.13515 (Fase 5)

Fecha: 2026-10-28. Síntesis de 4 pistas paralelas
(`analysis/investigations/entry-loop.md`, `orders-iscript.md`,
`diff32-64.md`, `triggers-data.md`). Todo verificado contra los informes;
aquí solo se integra, no se re-analiza. Direcciones 32b = VA del dump
(base `0x400000`); 64b = VA absoluta (base `0x6ffffade0000`).

Fórmula de rebase validada (A-H10, 4 rutinas): `VA_dump32 = VA_rt − 0x79fa0000 + 0x400000`.

## 1. Arranque (confianza alta)

- `entry` (`0x4a73a0`, 5 B) = `AddressOfEntryPoint` → `jmp` a `mainCRTStartup`
  MSVC (`___security_init_cookie`, `__initterm`, `__cexit`). Toolchain MSVC
  confirmada; el juego no empieza en `entry`.
- Puerta del juego: **`0x70e810`** (90 B, 1 único llamante = el CRT, prólogo
  SEH, carga `Starcraft.ini` vía `0x70ac50`, publica `argv` en `0x117c834`).
- Init confirmados: `0xc61f70` (ventana + contexto GL, `pm_gles::Context::Init`
  = capa GL propia del remaster), `0x8aee30` (orquestador settings
  Mouse/Video/Network/Sfx, 317 calls, 0 llamantes E8 → invocado por
  puntero/hilo), cargadores GL (`0xc60b80`/`0xc5fe60`), bzip2, FMOD,
  streaming CASC, cinemáticas WMV, decodificador de errores DX (`0x4f3828`).
- Infra ubicua (ruido de fondo para xrefs): `0x93ec70` (11 166 llamantes),
  `0x538810` (5 731), `0xc046b0` dispatcher de 361 casos (2 941, por
  identificar: ¿assert/log/getString?).

## 2. Bucle y mensajes (actualizado: WndProc CONFIRMADO)

- **`0x945050` = WindowProc principal** (3 759 B, 57 refs IAT / 25 slots,
  `switch(msg)` 67 casos, `msg<0x100`, `msg==0x400`, `wParam&0xffff`,
  0 xrefs = callback registrado). Veredicto posterior al informe A.
- Reenvío OO: `0x9468e0` (103 B, ×20 desde el WndProc) resuelve objeto vía
  IAT `0xd305a8` y salta a vtable+4 con (msg,wParam,lParam) — thunk
  mensaje→objeto. Confirma despacho por objetos, no switch monolítico.
- **`0x969f0d` reclasificado**: 0 IAT (lógica interna pura) → paso de
  simulación/trabajador, NO bomba de mensajes. Adyacente: `0xa80e70`.
- Bomba externa `GetMessage` sin localizar (prioridad baja: el cliente
  reimplementado pone su propio bucle). IAT confirmada en `0xd301xx`.

## 3. Órdenes + iscript (sistema presente, identificación pendiente)

- HECHO: ambos dumps contienen el sistema (`arr\*.dat`, `scripts\*.bin`,
  RTTI `COrder`/`CUnit`). Los **nombres de fila no están** en el binario
  (13/13 ausentes): localizar por anclas (`STR_*/SND_*`, `rescue_*`,
  `nukeWarning`, contadores) — tabla de anclas por familia en informe B §1.3.
- Ranking de familias (1 Cast … 10 Patrol; detalle y pools size-exactos en B
  §1.5/§2). Firma `TARGET_ORDER`: **falsada en imagen** — solo existe la
  tabla identidad (`.data 0xb2a570` / `0xdd5190`); el remap vive en heap o la
  ref. antigua no transfiere. Caza del despachador por estructura de switch.
- **Nuke CONFIRMADO** (`FUN_005d3823`, 440 B, ancla `SND_NUKE_LAUNCH`):
  `switch(CUnit+0x4e)` casos 0–6, transiciones `+0x4e=3/5`. Único llamante:
  `FUN_005ca229` (+0x41, UNCONDITIONAL_CALL; 0 xrefs hacia él → vía tabla).
  Callees = glue de errores. B-exp.1 (cast size-531): sin stores `+0x4e` →
  hipótesis de transferencia por tamaño debilitada.
- Experimentos listos: 8 con direcciones candidatas y firmas de pseudocódigo
  (stores `CUnit+0x4e/0x54/0x84/0x85/0x55` + nº de callees).

## 4. Datos y triggers (tablas del motor: sí; mapa cargado: no)

- Tablas del parser CHK en `.rdata` (idénticas): registro largo (~20
  secciones, C6) + despacho corto ligado a `League_%d.cnd` (8 secciones,
  C1; asimetría `UNIT` sí / `TRIG,MRGN` no → camino melee/ladder).
- Tabla SFX residente idéntica al byte (C2); tuplas `(id,fn,fn,id)` (C4);
  tablas 100 % punteros (C5, referencia de "estático").
- Negativos fuertes: 0 columnas DAT íntegras, 0 TRIG-2400, 0 `melee.trg`,
  0 STR, 0 briefing/objetivos, clusters UNIT/MRGN = falsos positivos
  (ceros / tabla de contadores). `.data` ~90 % BSS en cero.
- **Veredicto D: el dump en partida no contiene mapa+triggers cargados** —
  el volcado cubre la imagen del módulo; el mapa parseado vive en heap no
  capturado. Experimento decisivo: re-volcar regiones RW de `/proc/pid/maps`
  y repetir las firmas (requiere juego en marcha + autorización).

## 5. 32 vs 64 bits (H2 confirmada, confianza alta)

- Mismo manifiesto `1.23.10.13515` + hash `2e031d5be4`, espejo RTTI exacto
  (415 lambdas, 24 chat, 50 glue), vocabulario de engine compartido,
  desplazamiento global de tamaños ×1.16–1.83 con colas gemelas.
- Recomendación C: **base de análisis en 32 bits**, anclajes en 64 bits vía
  pares RTTI; 64 bits solo si el objetivo lo exige.

## 6. Grafo de dependencias (confirmadas vs inferidas)

Confirmadas: CRT → `0x70e810` → config (`Starcraft.ini`); init settings →
{ventana/GL, audio FMOD, streaming, cinemáticas}; loader mapas → MPQ
(`LoadMapPass2`) → parser CHK (C6/C1) → [heap no capturado].
Inferidas: `0x969f0d` → {input, sim, render}; despachador 189 filas →
137 handlers; tablas estáticas → sesión (C4 por verificar).

## 7. Plan de confirmación (ordenado por ratio señal/ruido)

1. Decompilar `0x70e810` + `0x70ac50` (firma `main`, global `0x117c834`).
2. Resolver IAT (`0xd30000–0xd31000`) y etiquetar `0xc61f70`/`0x969f0d`
   (busca `PeekMessage/Sleep/timeGetTime`).
3. Cast genérico size-531 (4 en 32b): 25 callees + store `+0x4e=1`.
4. Xrefs a `nukeWarning`/`SND_NUKE_*` → handlers filas 124–128.
5. Buscar `TARGET_ORDER` (189 B, 10 divergencias) → despachador.
6. Falsación Patrol size-69 (6 callees `…4dab0,57630…`).
7. C1/C6: resolver handlers del parser CHK/TRIG en los CSV.
8. Heap: re-volcar regiones RW en partida y repetir firmas TRIG/STR/UNIT.

Detalles, direcciones candidatas y preguntas abiertas: informes de
`analysis/investigations/` (A: P1–P7, B: §4–§5, C: §4, D: §8).
