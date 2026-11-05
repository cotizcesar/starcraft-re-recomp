# AGENTS.md — project rules for starcraft-re-recomp

> There is a single engine: `engine-rust` (full fidelity: 228 units,
> 189 orders, 137 transcribed handlers; the test count grows every phase —
> do not quote it here, run `cargo test`). The C experiment was removed on
> 2026-10-28 after everything useful was migrated (save/load, replay hashes,
> `socketpair` networking → design and tests in Rust); its history lives in
> `research/re-c.bundle` and its provenance in
> `research/docs/c-engine-provenance.md`. One engine, one truth (DRY).
> Crate rules (commands, retail/headless gotchas): `engine-rust/AGENTS.md`.

## 1. Layout (one tree, no duplicates)

```text
starcraft-re-recomp/
  README.md                      # project overview (product + process)
  PROGRESS.md                    # what we have / what is missing, with % (generated table)
  AGENTS.md                      # this file: the project contract
  CONTRIBUTING.md                # how to build, test and send changes
  CHANGELOG.md                   # notable changes per date
  engine-rust/ (+ AGENTS.md)     # THE engine (Rust, playable) + crate rules
    docs/                        # product docs; docs/gates/ = committed gate outputs
    packaging/                   # desktop/AppStream/icon, Flatpak manifest, macOS Info.plist
  scripts/                       # harnesses: LAN, headless renders, visual gates, progress.py, package_*.sh
  research/                      # reverse engineering: evidence, manifests, Ghidra tools, docs
    docs/                        # PROJECT-STATUS.md = detailed live state; session-log.md = log
    manifests/                   # inventory + SHA-256 of the reference install
    tools/                       # Ghidra scripts (Java) + memory capture
    re-c.bundle                  # git history of the removed C engine
  .github/                       # CI, issue/PR templates
  obsidian/                      # working-notes vault (OBSIDIAN.md = its rules);
                                 # curated docs above stay the source of truth
  .opencode/skills/              # OpenCode skills for the vault (sesion, investigar, ...)
  opencode.json                  # portable OpenCode project config (vault MCP, instructions)
```

Local-only (gitignored, never commit): `research/analysis/`,
`research/build/` (Ghidra projects), `research/work/` (memory dumps),
`research/logs/`, `engine-rust/assets/`, `engine-rust/fixtures/*` data,
`engine-rust/docs/evidence/`, screenshots and every other retail-derived file.
Also local: `.opencode/opencode.json` (personal OpenCode overrides such as
absolute plugin paths) and the Obsidian workspace/cache.

- The original install (`$SC_INSTALL`) is **read-only**. Nothing writes there.
- No parallel paths: the only checkout path is the repository itself. Never
  hard-code a personal absolute path (`/home/<user>/…`); use `SC_INSTALL`,
  `XDG_CACHE_HOME`, `HOME` or a CLI flag.
- `research/docs/session-log.md` is **append-only**: history is not
  rewritten (errors and falsifications stay recorded with their date).

## 2. Principles (SOLID / DRY / YAGNI / KISS applied to RE)

- **Evidence before assumption.** FACT (observed) vs INFERENCE (with a
  hypothesis and a falsifier). Inferred names are never presented as
  confirmed.
- **Real DRY:** one implementation per behaviour. Shared logic lives in one
  place (`replay_to` ↔ `apply_act` is the model: the `sc_view` replay player
  reuses `apply_act` instead of duplicating it). The C engine was removed
  rather than maintaining two implementations. One fact, one document: other
  docs link to it instead of restating it.
- **YAGNI:** no speculative systems. LAN lockstep (TCP+UDP, discovery,
  lobby, resync) is **in scope** and already exists (`src/net.rs`, `sc_lan`,
  `tests/lan.rs`); only the physical two-PC test is missing
  (`scripts/lan_two_machines.sh`). **Battle.net/online: out of scope.** No
  skins/features without a test that requires them. Every new file must
  answer a planned task.
- **KISS + SOLID in code:** small functions with a single reason to change;
  `World` knows nothing about sockets: the network only delivers commands
  (`RecAct`) and compares hashes (`state_hash`); the UI only touches internal
  state through the public API (`issue`, `train`, `replay_to`, …).
- **Determinism is an invariant:** the sim is an LCG with a fixed seed; every
  external mutation enters through `World::issue` or the audited direct
  mutators (`train`, `morph`, `morph_unit`, `morph_building`). Any new path
  that mutates state **must** be recorded in the command log or it will break
  save/load, replays and LAN. A test proves it (`tests/replay.rs`).

## 3. Rust (`engine-rust`)

- `cargo fmt` before every change; `cargo clippy --all-targets -- -D warnings`
  must report **0 warnings** (CI enforces both).
- Every new or fixed behaviour ships a **test that fails without the fix**
  (unit in `src/` or integration in `tests/`). Tests use paths relative to the
  crate's assets/fixtures; they never depend on the retail install except the
  ones gated on `SC_INSTALL`, which skip cleanly when it is unset.
- Own file formats: magic + version + SHA-256 checksum, explicit size limits
  before allocating (`n > 10_000_000 → None`), and a corrupt-file test.
- No `unwrap` on user-data load paths (return `Option` / `Result` with a
  message); `expect` only in tests.
- `World` gets no "just in case" fields: every new field needs a reader
  (tests) and a writer (save/hash) or it does not go in.

## 4. Differential validation (no second engine)

- The C engine was removed; its oracle role is covered by the Boxer+AI test
  (identical replay), `tools/oracle.py` (table differential), the 73/73
  arrays checked against the live game, and `re-c.bundle` as a historical
  reference. Do not resurrect a second engine without an explicit, justified
  decision in `research/docs/decision-log.md`.

## 5. Reverse engineering (`research/`)

- Read-only on the original and the dumps (`pread` of `/proc/<pid>/mem`, no
  debugger, no writes to the process). Working copies go to `/tmp`; the
  original is never touched.
- REA connection: `tools.rea.*` via MCP in the OpenCode session. Requires an
  `environment` block in the `mcp.rea` entry of `opencode.json`
  (`GHIDRA_INSTALL_DIR`, `JAVA_HOME` → JDK 21, `REA_GHIDRA_STARTUP_TIMEOUT_MS`);
  Ghidra pinned to **12.1.4** (12.2 is installed but REA requires 12.1.x).
  Verified details: `research/docs/rea-opencode-integration.md`.
- Per-target flow: `open_binary` → bounded queries (`list_procedures`,
  `search_strings`, `read_bytes`) before escalating → `binary_session` for
  deep work → `close_binary`. Ghidra only appears as a candidate after
  `open_binary`; `raw-binary` requires `architecture` + `entry_point`.
- Hard limit: `binary_overview` over MCP on images >10 MB gives a
  deterministic `provider_timeout` — do not repeat it. Alternative:
  `analyzeHeadless` by absolute path, with `JAVA_HOME` set to JDK 21, in the
  background, with small purpose-named scripts in `research/tools/*.java`.
  Failure record: `research/docs/tool-errors.md`.
- Annotations (`annotate_native_function`, `set_address_name`) only mutate
  Ghidra's ephemeral database, never the executable.
- Pseudocode ≠ original source. Addresses from old mappings are not valid in
  new dumps (explicit rebase or nothing).
- Every finding: file + address + confidence + the experiment that would
  confirm or refute it. Falsifications are published too.
- Doc index: `research/docs/README.md`.

## 6. Documentation

- **Language: English** for every committed document, comment, docstring,
  commit message and CLI message. Literal in-game strings are quoted as-is.
- **Progress directive.** `PROGRESS.md` is the single, community-facing
  record of what exists and what is missing, with percentages:
  1. When a phase or task closes, tick (`[x]`) or add (`[ ]`) the matching
     checklist item in `PROGRESS.md` **in the same commit**, with an
     evidence pointer (test, gate output or dated session-log entry).
  2. A box is ticked only with observable evidence; otherwise it stays open.
  3. Percentages are never typed by hand: run
     `python3 scripts/progress.py --write`. CI runs `--check` and fails on a
     stale table. New scope becomes a new unchecked item, which lowers the
     percentage honestly.
  4. Other docs (README, PROJECT-STATUS) link to `PROGRESS.md` instead of
     restating percentages.
- `research/docs/PROJECT-STATUS.md` (detailed internal state) is updated when
  each phase closes, not at the end.
- `research/docs/architecture.md` is the map; track reports live in
  `research/analysis/investigations/` (local) and are not re-edited after the
  synthesis (only referenced).
- `engine-rust/README.md` + `features.md` describe the product; this file
  describes the process. Do not duplicate content between them: link.
- Notable changes get a line in `CHANGELOG.md`.

## 7. Definition of Done (verifiable, no exceptions)

Nothing is declared done without pasting the observable proof:

- `cargo test` (from `engine-rust/`, with local fixtures) → 0 failed,
  including the new tests.
- `cargo clippy --all-targets -- -D warnings` → 0 warnings; `cargo fmt --check` clean.
- `cargo build --bin sc_view` OK + headless shots if the change touches UI
  (`--shot-screen saved|replays|pause`).
- `python3 scripts/progress.py --check` → OK, with `PROGRESS.md` updated if
  the change moves a checklist item.
- Docs touched + a dated `session-log.md` entry. No "works on my machine"
  without the pasted command.
