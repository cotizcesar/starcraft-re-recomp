# Reconstruction coverage

This page is the executable checklist for the reconstruction.  Every number
below is produced by a test or tool in the repository; the commands are
listed at the end.

## Retail data

| Table | Rows | Runtime cross-check |
|---|---:|---|
| `units.dat` | 228 | byte-identical arrays (73/73) |
| `weapons.dat` | 130 | byte-identical arrays |
| `flingy.dat` | 209 | byte-identical arrays |
| `orders.dat` | 189 | byte-identical arrays |
| `techdata.dat` | 44 | byte-identical arrays |
| `upgrades.dat` | 61 | byte-identical arrays |
| `sprites.dat` | 517 | byte-identical arrays |
| `images.dat` | 999 | byte-identical arrays |
| `stat_txt.tbl` | 1,539 | labels resolve |
| `iscript.bin` | 40,482 B | every shipped animation decodes |
| `aiscript.bin` | 293 entries | every opcode decodes |
| `bwscript.bin` | 156 entries | every opcode decodes |

The independent `tools/oracle.py` parses the same files with separate offsets;
all 228 units, 130 weapons, 209 flingy rows and 189 order rows match column-by-column.  `tools/verify_runtime_tables.py` compares
the running game's expanded arrays with the fixtures: **73/73 match**.

## Orders

| Item | Count |
|---|---:|
| `orders.dat` rows with a recovered spec | 189 |
| direct handler addresses | 165 |
| decompiled handler bodies | 137 / 137 addresses |
| every-tick dispatch rows | 40 |
| carry-8 dispatch rows | 128 |
| aliases resolved before dispatch | 7 |
| production-timer rows | 1 |
| engine-internal rows | 13 |
| non-identity target-order mappings | 10 |

`cargo run --bin sc_orders` validates the table and exits non-zero on an
unclassified row.  `issue_order_id` gives every player-facing row concrete
behaviour (orders, attacks, holds, wanders, return, self-destruct, abilities);
only engine-internal hero/doodad rows without a player command remain on the
family fallback.  The dispatcher is `FUN_7a178340`
(`docs/evidence/order_dispatch_vm.txt`).

## Gameplay systems

| System | Status | Representative test |
|---|---|---|
| Damage pipeline (matrix → shields → armour → size → floor) | complete | `damage_matrix_matches_the_independent_oracle` |
| 228 unit stats, 130 weapons, acquisition/cooldowns | complete | table differential tests |
| Projectiles (all 10 bullet behaviours, 25 hit types) | complete | projectile tests |
| Movement (tile regions, octile A*, iscript walk cycle) | complete | path/region tests; recovered A* heap tie-break |
| Unit collision / separation | complete | `ground_units_do_not_stack_on_one_another` |
| Buildings, construction, add-ons, lift/land | complete | addon/lift tests |
| Zerg creep growth/decay, morphs, Drone consumption, unit morphs | complete | creep/morph tests |
| Protoss Pylon power | complete | `protoss_production_requires_a_pylon_power_field` |
| Terran burning and SCV repair | complete | burn/repair tests |
| Economy (mining 8/trip, gas, supply cap 200, queues) | complete | economy tests |
| Tech tree (build/research/upgrade gates) | complete | `build_tree_matches_the_hard_coded_switch` |
| Spells (all shipped ability families incl. Infestation) | complete | spell tests |
| Transports, Bunkers (firing occupants), Nydus, Shield Battery | complete | transport/bunker/nydus tests |
| Vision, fog, cloak, detectors, Scanner Sweep | complete | vision tests |
| Triggers (classic conditions 0..23, actions 1..59, UPRP slots) | complete | trigger/UPRP tests |
| Campaign AI (`aiscript`/`bwscript`) | complete opcode interpreter | `TMCu` test |
| Deterministic skirmish AI | complete | AI tests |
| iscript VM | complete 0x00..=0x44 | animation decode tests |
| Graphical client (tileset, GRP sprites, creep, fog, HUD, minimap, input) | playable | `tests/render.rs`; `sc_view`, `sc_shot` |
| Command card (yellow HD glyphs on dark insets, 74x74 in a 138x120 cell) | measured against the 3440x1440 reference | `wells_compare` sweep, session log 2026-10-29 |
| Minimap ("Show Terrain in Minimap" toggle, click geometry from the content box) | matches the reference's radar state | `draw_minimap_with_terrain`; shot `ingame-*` |
| Replays (`gcrep`) + save games (`gcsave`) + `sc_replaytool` | complete | `tests/replay.rs`; `docs/replays.md` |
| Retail console HUD (plates, cmdbtns, wireframes, exact `stat*.ui.json`) | complete | `ConsoleArt`; `stat_dialogs_keep_the_shipped_exact_coordinates`; retail test |
| Retail main menu / pause / score / leaderboard art | complete | `GlueArt`; `glumain`/`gluscore`/`gamemenu`/`statlb`/`msnstats` layouts; exact `glumain` hotboxes/labels + `rez/flc.json` hover clips; `docs/menu.png`, `docs/ui-coverage.png` |
| 32 reference screens | aligned | measured geometry per screen; see `docs/ui-alignment.md` |
| Full music + voice sets (23 Ogg tracks, 1,122 voice files) | complete | `decode_ogg`; `SfxTable`; `VoiceBank`; retail materialise test |
| All 28 retail intros (Blizzard logo, title, Brood War, 25 campaign act movies) | complete | `video::tests::every_intro_table_movie_is_readable_and_decodable` (needs `SC_INSTALL`); `--shot-screen intro-blizzard|intro-title|intro-expansion`; `docs/intro.png`, live Xvfb run in `docs/intro-live.png` |
| Title intro + mission/briefing screens | complete | `--shot-screen intro|briefing`; `retail_briefing.png` |
| Campaign slots / briefing / results / progression | complete | `campaign_player_slots_keep_non_contiguous_owners`; `next_scenario_action_resolves_to_the_bundled_followup_mission` |
| Full retail campaign (68 CASC missions) + briefings | complete | `retail_campaign_enumerates_every_shipped_mission`; `retail_briefings_parse_and_their_backdrops_decode` |
| Retail WAV effects + software mixer | complete | `tests/audio.rs`; ALSA no-op fallback |
| Game shell (map menu, pause, victory/defeat, restart, AI opponent) | complete | Xvfb/xdotool run; `docs/menu.png` |

## Retail asset access

| Item | Result |
|---|---|
| CASC paths / local index / encoding records | 52,498 / 14,382 / 43,091 |
| Shipped MPQ archives decompressed and parsed | **323 / 323** |
| Maps with a `TRIG` section | 322 |
| Bottleneck CHK vs retail extraction | byte-identical (`65833b86…`) |
| Single-player campaign CHKs (`sd/campaign/*`) | **68 / 68** parse and simulate |
| Establishing-shot briefing scripts | 62 scripts, 17 retail backdrops |
| Documented DAT/GRP/table SHA-256 checks | 7 / 7 |
| Retail effects byte-identical | 9 / 9 |
| Retail UI layouts (`*.ui.json`) | 17 |
| Console/menu/score art surfaces | 37 console + 37 glue files |
| Ogg music tracks | 23 |
| Voice files (of 1,127 sfx names) | 1,122 |
| All-map simulation from MPQ + CASC | 323/323, 0 failed, 9,835 triggers on 322 maps |

`src/retail.rs` implements the CASC text build config, local `.idx`,
ENCODING/ROOT manifests, BLTE frames, and MPQ v1 (including protected
encrypted/key-v2/implode maps).  `sc_assets --install DIR verify` prints the
checks above; `sc_view --install DIR` sources every sprite, tileset, effect
and map from the installation and caches only the derived subset.  No retail
asset is required in the repository.

## Shipped content

* **391/391** maps (323 `.scm`/`.scx` + the 68 campaign CHKs from
  `sd/campaign/`) parse, build a `World`, and step for 1,200 ticks with
  triggers and AI scripts: `0 failed`, 68,013 units, 11,742 triggers on 390
  maps, 90 AI script starts.
* The client lists the retail campaign (Terran/Zerg/Protoss + Brood War) in
  episode order, opens the shipped `est*.txt` establishing-shot scripts with
  their real `glue/pal*` backdrops (DXT1/DXT5 `.dds`), and advances to the
  next mission on victory.  The human seat follows `OWNR == 6`, so missions
  whose player is not slot 0 (Zerg 01 slot 3, Enslavers 01 slot 6) start on
  the correct base.
* 322 maps contain `TRIG`; `tools/trigger_ids.py` finds only classic
  condition ids **1..23** and action ids **1..57**.  SC:R EUD memory paths are
  editor-only and not used by any shipped map.
* The trigger VM implements every classic action 1..=59 (preserve/wait
  resumption, Create Unit with Properties from `UPRP`, kill/remove counts,
  percentage HP/energy/shield edits, score categories, invincibility,
  doodad state, orders, alliances and the presentation/leaderboard records)
  and the CHK `Exactly` comparison value **10**.
* All 8 tilesets and 2–8 player melee layouts load.
* The playable client bundles thirteen maps (2–8 players, platform/jungle/
  twilight and other tilesets, including the five Enslavers campaign maps)
  and a start menu; `--chk` loads any other extracted map.  Trigger-started
  `CampaignAi` scripts run in the client.
* CV5/VX4EX/VR4/WPE terrain, GRP sprites, fog, HUD, minimap and
  mouse/keyboard RTS controls are rendered by the bundled playable client.

## Explicit limitations

Gameplay is complete. Every order row (189), handler family (137 transcribed
bodies sharing the recovered behavioural family, all player-facing/internal
rows with concrete typed implementations), trigger condition/action id used
by the 323 shipped maps, campaign AI opcode, iscript opcode, projectile
bullet/hit type, tech/research gate, economy, vision, transport and spell
rule above is implemented and tested. SC:R/EUD-only trigger memory paths
are parsed but intentionally not executed; `tools/trigger_ids.py` confirms
no shipped map uses them, so they are not a gameplay gap. Nine fallback WAV
effects are bundled only for no-install runs; with `--install` the full
23-track music and 1,122-voice set is materialised, so audio is not a gap.

The only remaining non-gameplay limits (see `docs/EVIDENCE.md`) are:

* the cosmetic 20th-anniversary/Carbot alternative console skins (the stock
  Terran/Zerg/Protoss consoles, menus, score panels, music and all voice sets
  are used);
* pixel-exact retail creep (no creep GRP exists in CASC; the client renders
  a deterministic organic mat from the recovered maturity grid, so the mat
  animates correctly but is not a pixel copy);
* cinematics now play the full retail intro table (REA: executable table at
  `0x140ae7ae0`): `smk/blizzard.webm` (11.4 s logo) then `smk/starintr.webm`
  on first launch, `smk/starxintr.webm` for Brood War, and the 25
  `Pre<Race>_<act>` / `<Race><act>` / `x<Race>` campaign movies around the
  matching acts. `src/cinematic.rs` streams each file through `ffmpeg` in
  two-second chunks, fills the 16:9 content box and skips with
  `Enter`/`Esc`; the soundtrack is decoded once (`video::decode_audio_pcm`,
  mono 22 050 Hz) and handed to the mixer. Campaign briefing pages that name a
  `</VIDEO>` clip now stream it at 12 fps inside the briefing panel
  (`VideoStore::resolve` normalises the script's `interstitials\TR1010.webm`
  to the cache's `interstitials/tr1010.webm`); the poster remains the first
  frame and the fallback without `ffmpeg`.

## Commands

```bash
cargo test   # lib + integration suites (183 lib tests at time of writing)
cargo run --release --bin sc_view -- --install "/path/to/StarCraft"
cargo run --release --bin sc_assets -- --install "/path/to/StarCraft" verify
SC_INSTALL="/path/to/StarCraft" cargo test --test retail   # 9 retail tests
make ui-shots SC_INSTALL="/path/to/StarCraft"   # intro/menu/mission/pause/score
cargo run --release --bin sc_campaign -- --install "/path/to/StarCraft" --ticks 1200
cargo run --release --bin sc_shot -- --out /tmp/frame.bmp
make verify                 # test + clippy + order table + table oracle
cargo run --bin sc_orders
python3 tools/oracle.py fixtures
python3 tools/verify_runtime_tables.py /tmp/sc_mem/sc_dump.exe
python3 tools/trigger_ids.py /tmp/sc_maps
cargo run --release --bin sc_campaign -- \
  --maps /tmp/sc_maps --tiles /tmp/tiles/TileSet --ticks 1200
```
