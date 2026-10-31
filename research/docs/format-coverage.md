# Cobertura de formatos

| Formato | Estado | Lector | Validación |
|---|---|---|---|
| CASC (BLTE N/Z, ENCODING, ROOT, MPQ v1 protegido, DCL) | leído | `src/retail.rs` | 323/323 mapas + hashes DAT/GRP/WAV |
| DAT (`units/weapons/flingy/orders/tech/upgrades/sprites/images`) | parseado | `src/data.rs`, `src/tables.rs` | 73/73 tablas vs juego vivo; tests vs `oracle.py` |
| `stat_txt.tbl` (1 539) | parseado | `src/tables.rs` | tests |
| iscript 0x00..0x44 | VM completa | `src/iscript.rs` | todos los ciclos Walking, release Marine |
| aiscript/bwscript | intérprete | `src/aiscript.rs` | 293+156 entradas, run `TMCu` |
| CHK (DIM/ERA/MTXM/UNIT/MRGN/STR/OWNR/SIDE/TRIG) | parseado | `src/map.rs`, `src/trigger.rs` | 391 mapas simulan; TRIG 1..59 / cond 0..23 |
| Tileset CV5/VX4EX/VR4/WPE + `Units.pal` | render | `src/render.rs` | tile real + minimapa en tests |
| GRP sprites | decodificado | `src/render.rs` | Marine + 80 sprites |
| DDS/DXT + `.dds.grp` | decodificado | `src/dds.rs` | superficies retail |
| WAV efectos / Ogg música / voces | mezcla | `src/audio.rs`, `src/sfx.rs`, `src/voice.rs` | 9 WAV + 23 Ogg + 1 122 voces |
| `sfxdata.tbl/.dat` | parseado | `src/sfx.rs` | tests |
| CEF `.pak`, locale `.htm` | no objetivo | — | lanzador, fuera del motor |
| PE juego `.text` | bloqueado hasta dump | REA+Ghidra | entropía ~8.0 en disco |

Extraídos: ver `extracted/` y `~/Documents/galaxycraft/assets/`.
Manifiesto extraído→origen: pendiente de formalizar (Fase 6).
