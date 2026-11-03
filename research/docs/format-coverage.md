# Format coverage

| Format | Status | Reader | Validation |
|---|---|---|---|
| CASC (BLTE N/Z, ENCODING, ROOT, protected MPQ v1, DCL) | read | `src/retail.rs` | 323/323 maps + DAT/GRP/WAV hashes |
| DAT (`units/weapons/flingy/orders/tech/upgrades/sprites/images`) | parsed | `src/data.rs`, `src/tables.rs` | 73/73 tables vs live game; tests vs `oracle.py` |
| `stat_txt.tbl` (1 539) | parsed | `src/tables.rs` | tests |
| iscript 0x00..0x44 | full VM | `src/iscript.rs` | all Walking cycles, Marine release |
| aiscript/bwscript | interpreter | `src/aiscript.rs` | 293+156 entries, `TMCu` run |
| CHK (DIM/ERA/MTXM/UNIT/MRGN/STR/OWNR/SIDE/TRIG) | parsed | `src/map.rs`, `src/trigger.rs` | 391 maps simulate; TRIG 1..59 / cond 0..23 |
| Tileset CV5/VX4EX/VR4/WPE + `Units.pal` | render | `src/render.rs` | real tile + minimap in tests |
| GRP sprites | decoded | `src/render.rs` | Marine + 80 sprites |
| DDS/DXT + `.dds.grp` | decoded | `src/dds.rs` | retail surfaces |
| WAV effects / Ogg music / voices | mix | `src/audio.rs`, `src/sfx.rs`, `src/voice.rs` | 9 WAV + 23 Ogg + 1 122 voices |
| `sfxdata.tbl/.dat` | parsed | `src/sfx.rs` | tests |
| CEF `.pak`, locale `.htm` | not a target | — | launcher, outside the engine |
| Game PE `.text` | blocked until dump | REA+Ghidra | entropy ~8.0 on disk |

Extracted: see `extracted/` and `engine-rust/assets/`.
Extracted→source manifest: pending formalization (Phase 6).
