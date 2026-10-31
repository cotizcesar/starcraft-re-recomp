# AGENTS.md — engine-rust (galaxycraft)

Rust crate: deterministic StarCraft 1.23 gameplay engine + playable `sc_view` client. Retail bytes are never vendored; they are read from the player's install at runtime.

## Commands (run from `engine-rust/`)

- `cargo test` — default suite, works with no install (retail tests skip without `SC_INSTALL`).
- `cargo test --test <name> <test_fn>` — focused run, e.g. `cargo test --test replay boxer_with_ai`.
- `cargo clippy --all-targets -- -D warnings` — must be 0 warnings before done.
- `cargo fmt` — run before every change.
- `make verify` — `test + clippy + orders + oracle + audit` (no install needed except `audit` wants `SC_MAP_DIR`).
- `python3 tools/oracle.py fixtures` — table differential oracle; `cargo run --quiet --bin sc_orders` — 189-row order table check.
- Retail (needs install): `SC_INSTALL=/path/to/StarCraft cargo test --test retail`, `cargo run --release --bin sc_assets -- --install "$SC_INSTALL" verify`, `cargo run --release --bin sc_view -- --install "$SC_INSTALL"`.

## Bins (autodiscovered; `Cargo.toml` lists only 3)

`sc_view` (client), `sc_match`, `sc_campaign`, `sc_assets`, `sc_sim`, `sc_replay`, `sc_lan`, `sc_shot`, `sc_audit`, `sc_orders`, `sc_handlers`, `sc_regions`, `sc_list`, `sc_animsheet`.

## Retail / headless gotchas

- Never write to the install; never commit derived files (`/assets/`, `/.galaxycraft-cache/` are gitignored — regenerate with `sc_assets … extract --out`).
- Install resolution: `--install DIR` flag, else `SC_INSTALL`; derived cache defaults to `$XDG_CACHE_HOME/galaxycraft/retail-<build>` (`--cache DIR` / `GALAXYCRAFT_CACHE` overrides).
- Headless: set `SC_NO_AUDIO=1`; UI regression shots via `sc_view --shot /tmp/x.bmp --shot-screen <screen>` (e.g. `menu`, `briefing`, `pause`, `saved`, `replays`, `score`; see the `match shot_screen` block in `src/bin/sc_view.rs` for the full list). Menu clips are 10 s loops: add `--menu-at SECONDS` for a deterministic phase and `--menu-phase single=2.75,multi=1.875,exit=3.0` for per-layer offsets; `python3 ../scripts/menu_time_diff.py --install "$SC_INSTALL"` checks the four reference captures at their measured phases.
- Tests use relative paths (`assets/maps/Boxer.chk`, `assets/TileSet/platform.vf4`, `fixtures/`); do not depend on CWD outside the crate root.

## Sim invariants (do not break)

- Determinism is load-bearing: LCG sim, `World::issue` is the single order entry point (AI in `src/ai.rs` also goes through it). Audited direct mutators (`train`, `morph`, `morph_unit`, `morph_building`) must record into `Recorder` as `RecAct` or save/replay/LAN desync.
- Save/replay = command log + fast-forward (`replay_to`), never a `World` snapshot — see `docs/save-format.md`. `World` holds `Arc<GameData>`/caches/scratch that must not be serialized.
- New `World` fields need a reader (test) and a writer (`save`/`state_hash`); new file formats need magic (`GCS1`) + version + trailing checksum, explicit size cap before allocating, and a corrupt-file rejection test (`Recording::load → None` pattern in `tests/replay.rs`).
- LAN (`src/net.rs`) is command-only lockstep + periodic `state_hash`; `tests/lan.rs` proves it over real TCP loopback (sync, map-mismatch refusal, drop+resync from log).

## Rules for changes

- One behavior = one implementation (`replay_to` reuse, not a copy); no Battle.net/online, no map editor (see `features.md` non-goals); each new file must serve a planned task.
- Every fix/feature ships a test that fails without it (unit in `src/` or integration in `tests/`); `expect` only in tests, no `unwrap` on user-data load paths (return `Option`/`Result`).
- Done = `cargo test` green (incl. new tests) + clippy 0 + `cargo build --bin sc_view` (+ headless shot if UI touched).

## Reverse engineering

Rule lookup lives here, not in this crate: root `AGENTS.md` §5 (REA operating
instructions), `research/docs/README.md` (doc index), `research/docs/toolchain.md`
(env pins). Never touch the retail install; work on `/tmp` copies.
