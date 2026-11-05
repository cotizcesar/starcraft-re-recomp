# AGENTS.md — project rules for starcraft-re-recomp

There is one engine: `engine-rust/` (crate `galaxycraft`, playable client
`sc_view`). Crate commands, bins, retail/headless gotchas and sim invariants
live in `engine-rust/AGENTS.md` — read it before touching Rust. Do not quote
test counts anywhere; run `cargo test`.

The C engine was removed on 2026-10-28 (history: `research/re-c.bundle`,
provenance: `research/docs/c-engine-provenance.md`). Do not resurrect a second
engine without a justified entry in `research/docs/decision-log.md`. Its oracle
role is covered by the Boxer+AI identical-replay test, `engine-rust/tools/oracle.py`
and the arrays checked against the live game.

## 1. Where things live

- `engine-rust/` — the engine; `docs/gates/` = committed gate outputs; `packaging/` = desktop/Flatpak/macOS metadata.
- `scripts/` — LAN harnesses, headless renders, visual gates, `progress.py`, `package_*.sh` (release builds go through `stage_clean_tree.sh` so no retail file can ship).
- `research/` — RE evidence, manifests, Ghidra Java scripts (`research/tools/`). Doc index: `research/docs/README.md`; live state: `research/docs/PROJECT-STATUS.md`; map: `research/docs/architecture.md`.
- `obsidian/` — working-notes vault (rules in `obsidian/OBSIDIAN.md`, loaded via `opencode.json`). Start a task with `obsidian/context/where-to-look.md` and `python3 scripts/vault_check.py --open --area <area>`; skills `vault-session`, `vault-research`, `vault-review` in `.opencode/skills/`. Curated repo docs stay the source of truth.
- Local-only, gitignored, never commit: `research/{analysis,build,work,logs}/`, `engine-rust/assets/`, `engine-rust/fixtures/*` data, `engine-rust/docs/evidence/`, screenshots and any retail-derived file; `.opencode/opencode.json` (personal overrides).
- The retail install (`$SC_INSTALL`) is **read-only**. Never hard-code a personal absolute path; use `SC_INSTALL`, `XDG_CACHE_HOME`, `HOME` or a CLI flag.
- `research/docs/session-log.md` is **append-only** (falsifications stay recorded with their date).

## 2. Toolchain and CI (what actually gates a merge)

- Rust pinned to **1.99.1** (`engine-rust/rust-toolchain.toml`, edition 2024). CI uses `--locked`: update `Cargo.lock` with any dependency change.
- CI has **no fixtures**: it runs `fmt --check`, `clippy --all-targets -D warnings`, `build --bins`, `test --no-run`, and only the fixture-free tests (`cargo test --locked --test menus --test menu_layers --test hd_tileset --test portraits --test retail_replay` + the `PLATFORM_TESTS` lib filters in `.github/workflows/ci.yml`), on Linux, Windows and macOS. The full suite must be run locally with fixtures (`engine-rust/fixtures/README.md`).
- Python job: `python -m compileall scripts research/tools engine-rust/tools`, `bash -n scripts/*.sh`, `python -m unittest scripts/test_progress.py scripts/test_vault_check.py`, `python scripts/progress.py --check`, `python scripts/vault_check.py`, plus XML/plist parse of `engine-rust/packaging/`.
- Headless runs: `SC_NO_AUDIO=1`.

## 3. Principles (RE-specific)

- **Evidence before assumption.** FACT (observed) vs INFERENCE (hypothesis + falsifier). Inferred names are never presented as confirmed. Mark this in the vault too.
- **One implementation per behaviour** (`sc_view`'s replay player reuses `apply_act`; it does not copy it). One fact, one document: link instead of restating.
- **Scope:** LAN lockstep is in scope and exists (`src/net.rs`, `sc_lan`, `tests/lan.rs`); only the physical two-PC test is open (`scripts/lan_two_machines.sh`). Battle.net/online and a map editor are **out of scope**. Every new file must answer a planned task.
- **Determinism is an invariant:** every state mutation enters through `World::issue` or the audited mutators and is recorded in the command log, or save/load, replays and LAN break (`tests/replay.rs` proves it). `World` knows nothing about sockets; UI touches state only via the public API.

## 4. Rust rules

- `cargo fmt` before every change; clippy must report 0 warnings.
- Every new or fixed behaviour ships a test that fails without the fix. Tests use crate-relative paths; retail-dependent tests are gated on `SC_INSTALL` and skip cleanly when unset.
- Own file formats: magic + version + SHA-256 checksum, size cap before allocating (`n > 10_000_000 → None`), and a corrupt-file test. No `unwrap` on user-data load paths; `expect` only in tests.
- No speculative `World` fields: each needs a reader (test) and a writer (save/`state_hash`).

## 5. Reverse engineering

- Read-only on the original and dumps (`pread` of `/proc/<pid>/mem`, no debugger, no writes to the process). Work on copies in `/tmp`.
- REA is reached as `tools.rea.*` (Code Mode). Its MCP entry lives in the **user-global** `~/.config/opencode/opencode.json` → `mcp.rea` (not the repo `opencode.json`) and needs an `environment` block (`GHIDRA_INSTALL_DIR`, `JAVA_HOME` → JDK 21, `REA_GHIDRA_STARTUP_TIMEOUT_MS`). Ghidra **12.1.4** (12.2 is installed but REA requires 12.1.x). Details: `research/docs/rea-opencode-integration.md`, `research/docs/toolchain.md`.
- Flow: `open_binary` → bounded queries (`list_procedures`, `search_strings`, `read_bytes`) → `binary_session` for deep work → `close_binary`. Ghidra only appears as a candidate after `open_binary`; `raw-binary` requires `architecture` + `entry_point`.
- `binary_overview` on images >10 MB gives a deterministic `provider_timeout` — do not retry. Use `analyzeHeadless` by absolute path (JDK 21, in the background) with small purpose-named scripts in `research/tools/*.java`. Failures: `research/docs/tool-errors.md`.
- Ghidra annotations only touch Ghidra's ephemeral DB. Pseudocode ≠ original source. Addresses from old mappings are invalid in new dumps (explicit rebase or nothing).
- Every finding: file + address + confidence + the experiment that would confirm or refute it. Publish falsifications too.

## 6. Documentation

- **English** for every committed doc, comment, commit message and CLI message; literal in-game strings quoted as-is.
- `PROGRESS.md` is the single public record of done/missing. When a task closes, tick (`[x]`) or add (`[ ]`) its item **in the same commit** with an evidence pointer (test, gate output, dated session-log entry). Never type percentages: run `python3 scripts/progress.py --write` (CI `--check` fails on a stale table). Other docs link to it.
- Update `research/docs/PROJECT-STATUS.md` when each phase closes; add a line to `CHANGELOG.md` for notable changes. Investigation reports in `research/analysis/investigations/` (local) are not re-edited after synthesis.
- `engine-rust/README.md` = product, `PROGRESS.md` = feature list, this file = process. Do not duplicate between them.

## 7. Definition of Done (paste the proof)

From `engine-rust/` with local fixtures:

- `cargo test` → 0 failed, including the new tests.
- `cargo clippy --all-targets -- -D warnings` → 0 warnings; `cargo fmt --check` clean.
- `cargo build --bin sc_view` OK, plus headless shots if UI changed (`--shot /tmp/x.bmp --shot-screen saved|replays|pause|…`).
- `python3 scripts/progress.py --check` → OK (with `PROGRESS.md` updated if a checklist item moved).
- Docs touched + a dated `research/docs/session-log.md` entry. No "works on my machine" without the pasted command.
- **Always commit the vault too:** every commit includes the new or changed notes under `obsidian/` (a `sessions/` note whose `log` names the new session-log entry, ticks on closed items, decisions, context) together with the code/docs change. Never leave vault notes untracked. `python3 scripts/vault_check.py --write && python3 scripts/vault_check.py` → `OK` (CI fails otherwise). Workspace/cache files are gitignored; do not force-add them.
