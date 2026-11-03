# Contributing

Thanks for your interest. This project is a reconstruction driven by
evidence, so contributions are judged on the proof they bring as well as on
the code. The full contract is in [AGENTS.md](AGENTS.md). This page is the
practical summary.

## Ground rules

1. **No Blizzard material in the repository.** Never commit or attach game
   files, extracted data, sprites, audio, screenshots, memory dumps or
   decompiled code. The `.gitignore` blocks the usual paths, so don't
   force-add around it. Everything retail-derived is regenerated locally from
   your own install.
2. **The install is read-only.** Tools read `$SC_INSTALL`; nothing writes
   there.
3. **English** for code comments, docs, commit messages and CLI output.
4. **Evidence before assumption.** Mark findings as FACT or INFERENCE. An
   inference names the experiment that would falsify it.
5. **Determinism.** Any new way of mutating `World` goes through
   `World::issue` or an audited mutator that records into the command log
   (see `engine-rust/AGENTS.md`).
6. **Out of scope:** Battle.net/online, the map editor, other patches.

## Setup

```bash
git clone https://github.com/cotizcesar/starcraft-re-recomp.git
cd starcraft-re-recomp/engine-rust
cargo build
```

Without fixtures the crate builds and lints, but most tests need the
gitignored developer fixtures. Generate them once from your install, as
described in [`engine-rust/fixtures/README.md`](engine-rust/fixtures/README.md).
Tests that need the full install read `SC_INSTALL` and skip when it is unset.

## Before you open a pull request

Run, from `engine-rust/`:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings   # 0 warnings
cargo test                                  # 0 failed (with local fixtures)
cargo build --bin sc_view
```

and, from the repository root:

```bash
python3 scripts/progress.py --check
python3 -m unittest scripts/test_progress.py
```

If your change touches the UI, attach the headless shot command you ran
(`sc_view --shot … --shot-screen …`) and its numeric gate result, not the
image. Screenshots of the retail game are copyrighted.

## What a good PR contains

- One behaviour per PR, with a **test that fails without the change**.
- No `unwrap` on paths that load user data, and `expect` only in tests.
- New file formats have magic + version + SHA-256, a size cap and a
  corrupt-file test.
- Docs updated where behaviour changed, and a dated entry appended to
  `research/docs/session-log.md` (append-only, never rewrite old entries).
- **`PROGRESS.md` updated** if the change completes or adds a checklist item:
  tick or add the item with an evidence pointer, then run
  `python3 scripts/progress.py --write`.
- A `CHANGELOG.md` line for anything user-visible.

## Commit messages

Use the imperative mood with a short scope prefix when it helps, for
example `net: resync from the command log after a dropped peer` or
`docs: translate the LAN protocol to English`. Keep the subject under about
72 characters and explain the *why* in the body.

## Reporting bugs and proposing features

Use the issue templates. For a bug, include the commit, the OS, the exact
command and what you expected. For reverse-engineering findings, include the
binary (32/64-bit), the address, the confidence level and how to reproduce.
