---
title: Windows migration planning
date: 2026-11-05
tags: [session]
type: session
source: opencode
area: platform
log: []
open: 4
---

## Goal
Work out what to copy to a USB drive to move the project to Windows without losing work. Check whether PowerShell alone is enough and whether moving to Windows helps finish the decomp.

## Progress
- FACT: there are uncommitted changes on `desperate-alliance` in `sim.rs`, `trigger.rs`, `bin/sc_view.rs` and `tests/campaign_episode1.rs`.
- FACT: `multiplayer-lan-flow` has 2 unpushed commits (`ed7cd30`, `8bf58c4`). It is checked out in the worktree `../starcraft-re-recomp-mp`.
- FACT: data that exists only on this PC (gitignored) includes `engine-rust/assets` + `fixtures` (105 MB, needed by the tests), `src/menu_masks/`, `camp_art.png`, the evidence and screenshot folders under `engine-rust/docs/`, the ignored files in `research/analysis`, `reports`, `docs/screenshots` and `docs/rea-evidence-c.md`, `research/build` (Ghidra projects, 451 MB), `research/logs` (193 MB), `research/work` (memory dumps, 4.5 GB, optional) and `.opencode/opencode.json`. `engine-rust/target` (14 GB) is not needed.
- FACT: outside the repo, `~/.config/opencode` (18 MB) and `~/.local/share/opencode` (951 MB, optional) should be copied. `SC_INSTALL` is not set in the current shell.
- Proposed a `tar` command that skips `target` and `__pycache__`, written to a USB drive formatted as exFAT (about 6 GB, or about 1.5 GB without `research/work`).
- FACT: three tools read `/proc/<pid>/mem` and are Linux-only: `research/tools/capture_memory.py`, `research/tools/heap_capture.py` and `engine-rust/tools/verify_runtime_tables.py`.
- Corrected an earlier recommendation: WSL2 cannot see the memory of a native Windows StarCraft process, so the memory tools have to be ported to the Windows API whether or not WSL is used.
- INFERENCE: on Windows, the Rust engine, most Python scripts, Ghidra (`analyzeHeadless.bat`), Obsidian and OpenCode should work from PowerShell. Git Bash should cover the `.sh` scripts.
- INFERENCE: most open items in [[PROGRESS]] are engine work that does not depend on the OS. Windows could help with three of them: the CEF UI audio blocker ([[PROJECT-STATUS]] #23), recording retail missions with sound, and the two-machine LAN test if the Linux PC is kept.

## Decisions
- Recommended keeping Linux and adding Windows as a second machine rather than replacing Linux. The user has not decided yet.
- Recommended creating a new SSH key on Windows instead of copying the private key.

## Pending
- [ ] User to confirm: WIP commit on `desperate-alliance`, push both branches, and create a `git bundle --all` on the USB drive.
- [ ] Port the memory capture tools to `OpenProcess(PROCESS_VM_READ)` + `ReadProcessMemory`, read-only and with a test.
- [ ] Set up the Windows environment: Rust 1.99.1, Ghidra 12.1.4, JDK 21, `core.autocrlf false`, REA paths in the OpenCode config, and `SC_INSTALL`.
- [ ] Rebase or recapture the dump addresses for the native Windows process.
