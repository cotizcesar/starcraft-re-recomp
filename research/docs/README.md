# research/docs — index

Reading order for a new session. Current canonical docs. Docs under
`../analysis/investigations/` and `rea-evidence-c.md` are local-only (gitignored).

## Entry (read first)

| Doc | What it is |
|---|---|
| [`../../PROGRESS.md`](../../PROGRESS.md) | Community-facing progress with percentages (single source of truth for %) |
| `PROJECT-STATUS.md` | Live project status (goal, phase, pending items) |
| `architecture.md` | Verified map of the binary (gate, WndProc, sim, nuke) |
| `../../AGENTS.md` §5 | REA/Ghidra rules |
| `blockers.md` | Active blockers and alternative paths |

## By topic

| Topic | Doc |
|---|---|
| Toolchain + REA usage rules | `toolchain.md` |
| REA↔OpenCode integration (E2E PASS) | `rea-opencode-integration.md` |
| Observed capabilities per provider (+ tool × capability matrix) | `rea-capabilities.md` |
| Tool errors (timeouts, JDK) | `tool-errors.md` |
| Platform/executables/CASC | `runtime-and-platform.md` |
| Game inventory (724 files) | `file-inventory.md`, `../manifests/` |
| Formats and their readers | `format-coverage.md` |
| Analysis strategy | `analysis-strategy.md` |
| LAN protocol | `lan-protocol.md` |
| Visual parity vs retail | `gap-audit.md`, `parity-report.md`, `graphics-remastered.md` |
| Complete Hotkeys data (220 actions, 88 units) | `hotkeys-data.md` |
| Player profiles: retail `.spc`/`.mpc` RE + the `GCPF` store | `profile-format.md` |
| In-game heap findings | `heap-ingame-findings.md` |
| Episode RE + mission modal (1:1, 43/43) | `../analysis/investigations/episode-screen-re.md` (local-only) |
| RE of the CEF screens (Registry/Collections/Saved/Replays/Lobby, 43/43) | `../analysis/investigations/cef-screens-re.md` (local-only) |
| Dated decisions | `decision-log.md` |
| Open questions | `unresolved-questions.md` |
| Removed C engine (historical) | `c-engine-provenance.md`, `rea-evidence-c.md` (local-only), `../re-c.bundle` |
| Append-only log | `session-log.md` |
| Phase 0 environment (historical) | `environment.md` |
| Engine identification (historical) | `engine-identification.md` |
