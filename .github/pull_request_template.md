## What and why

<!-- One behaviour per PR. Link the issue if there is one. -->

## Evidence

<!-- Paste the commands you ran and their output (test results, gate numbers).
     Do NOT attach retail screenshots, game files or decompiled code. -->

## Checklist

- [ ] `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` (0 warnings)
- [ ] `cargo test` passes (0 failed), including a new test that fails without this change
- [ ] Determinism kept: no new state mutation outside `World::issue` or an audited mutator
- [ ] Docs updated and a dated entry appended to `research/docs/session-log.md`
- [ ] `PROGRESS.md` item ticked or added (if applicable) and `python3 scripts/progress.py --write` run
- [ ] `CHANGELOG.md` updated for user-visible changes
- [ ] No Blizzard assets or retail-derived data added to the repository
