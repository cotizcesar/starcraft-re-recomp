# Changelog

Notable changes to this project. The detailed, dated engineering record is
[research/docs/session-log.md](research/docs/session-log.md). Completion
status is in [PROGRESS.md](PROGRESS.md).

## 2026-11-03 — MIT-only license

### Changed
- The project is now licensed under MIT only. `LICENSE-APACHE` is removed and
  `LICENSE-MIT` is renamed `LICENSE`. The vendored PKWARE DCL decoder keeps its
  own license.
- `main` is protected: changes land through pull requests with green CI.

## 2026-11-03 — Repository cleanup and community release

### Added
- `PROGRESS.md`: community-facing checklist of what exists and what is
  missing, with per-area and weighted overall percentages.
- `scripts/progress.py` (with `scripts/test_progress.py`) generates the
  `PROGRESS.md` summary table from the checklists. `--check` runs in CI.
- Progress directive in `AGENTS.md` §6: every closed phase updates
  `PROGRESS.md` in the same commit, and percentages are never typed by hand.
- GitHub Actions CI (fmt, clippy `-D warnings`, build, test compilation,
  fixture-free tests, Python checks, progress check), issue and PR templates,
  Dependabot.
- `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`, `.editorconfig`,
  `scripts/README.md`.

### Changed
- All documentation, comments and tool messages are now in English,
  including the full session log, translated faithfully with every dated
  entry kept.
- The root `README.md` is rewritten as the project overview.
- Methodology: completion percentages now come from explicit checklists. The
  old hand-weighted estimates ("≈ 85 % offline", "≈ 90 % LAN", "≈ 30 % RE")
  are replaced by the generated table in `PROGRESS.md`.
- Gate outputs moved from `engine-rust/docs/*.txt` to `engine-rust/docs/gates/`,
  and `engine-rust/docs/new_screenshots/` was renamed `engine-rust/docs/proof-sheets/`.
- Tests and scripts no longer hard-code a personal home directory. They use
  `SC_INSTALL`, `GC_RETAIL_CACHE`, `XDG_CACHE_HOME` or `HOME`.

### Removed (duplicates)
- `research/original-manifest/sha256.txt` (byte-identical to
  `research/manifests/sha256.txt`). The inventory and its README were merged
  into `research/manifests/`.
- `research/docs/archive/` (three superseded docs; git history keeps them).
- `research/docs/tool-capability-matrix.md` was merged into `rea-capabilities.md`.
- `research/SALVAGED.md` and `research/docs/salvage-sc-cpp.md` were merged
  into `research/docs/c-engine-provenance.md`.

## Earlier history

See `git log` and the session log. Highlights: the single Rust engine
(2026-10-28, after the C experiment was removed), saves and replays, LAN
lockstep over TCP/UDP, the full retail campaign, the Remastered menus (visual
gate 43/43) and in-game HUD parity work.
