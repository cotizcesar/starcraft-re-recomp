# scripts/ — harnesses on top of the engine

Run every script from the repository root. Scripts that render frames need a
release `sc_view` build (`cargo build --release` in `engine-rust/`) and
`SC_INSTALL` pointing at your own StarCraft: Remastered install. Reference
captures (`engine-rust/docs/original_screenshots/`) and all PNG output are
local-only and never committed.

| Script | Purpose | Needs |
|---|---|---|
| `progress.py` | Generate (`--write`) or verify (`--check`) the summary table in `PROGRESS.md` | — |
| `test_progress.py` | Unit tests for `progress.py` (`python3 -m unittest scripts/test_progress.py`) | — |
| `lan_two_machines.sh` | LAN acceptance on two PCs: compare the printed `LAN_RESULT` tick and hash | two machines, same map |
| `render_screens.sh` | Headless render of every client screen to `/tmp/shots/` | `SC_INSTALL` |
| `ui_diff.py` | Per-reference control checks (dialog borders, rings, art panels) | references |
| `ui_visual_diff.py` | Full-frame visual gate over the 2560x1440 content box | references |
| `menu_time_diff.py` | Deterministic check of the four main-menu references at fixed clip phases | references |
| `menu_walkthrough.py` | Scripted menu walkthrough: transitions, UI sounds, state and frames | `SC_INSTALL` |
| `campaign_phase_sweep.py` | Fit campaign clip phases per reference (coordinate descent) | references |
| `make_proof_sheets.py` | Proof sheets (retail \| sc_view \| diff) and `engine-rust/docs/proof-sheets/REPORT.md` | references |
| `make_calibration.py` | Build the optional per-reference `.gcal` correction layers | references |
| `compare_shot.py` / `compare_all.py` | Region-grid difference of one render, or all 44 references | references |
| `extract_menu_masks.py` | Measure text coverage masks into `engine-rust/src/menu_masks/` | references |
| `gen_button_sets.py` | Generate `engine-rust/fixtures/button_sets.in` from your own menu-time memory dump | dump |
| `cef_host_probe.py` | Serve the shipped CEF options panel with a synthetic WebSocket host | extracted panel |

Committed gate outputs live in `engine-rust/docs/gates/`.

Related tooling lives elsewhere: crate tools (oracle, table extractors,
handler generator) are in `engine-rust/tools/`, and reverse-engineering tools
(Ghidra scripts, memory capture, inventory) are in `research/tools/`.
