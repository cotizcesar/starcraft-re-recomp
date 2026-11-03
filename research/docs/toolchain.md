# Toolchain (without Hermes)

Verified 2026-10-28 on Bazzite 44.20261026.0. Chain: OpenCode + REA MCP.

| Block | Tool | Status |
|---|---|---|
| Orchestration | OpenCode v2.2.0 | active; REA available as MCP in this session |
| Native analysis | REA `rea-agents@6.8.0` + Ghidra 12.1.4 + JDK Temurin 21.0.13+9 | E2E PASS 2026-10-28 (20 862 proc / 13 326 strings, Launcher PE) |
| Binary | `file`, `objdump`, `sha256sum`, `xxd` | present |
| CASC/MPQ data | `src/retail.rs` (native reader in `engine-rust/`), `tools/casc_extract.cpp`, `tools/mpq_extract.cpp` | working |
| Scripts/analysis | Python 3.14, `tools/oracle.py`, `tools/trigger_ids.py` | present; `pefile`/`capstone`/`lief` absent |
| Build/run | Rust 1.99.1, gcc 16, `podman`/`distrobox` | present |
| Game execution | Lutris/Proton (no `wine` on PATH) | isolated environments only, with authorization |

Note: `opencode mcp list` run via shell from `research/`
returns "No MCP servers configured" (varies by working directory);
in this OpenCode session the `tools.rea.*` tools are registered,
the server starts and Ghidra analyzes (E2E PASS). The state that counts is the
session's, documented in `docs/rea-opencode-integration.md` and `logs/rea-readiness.log`.

Nothing requires `sudo`; everything lives in home. Ghidra 12.2 also installed,
but REA pins 12.1.x: use 12.1.4. `analyzeHeadless` only by absolute path.

## Usage rules

- Installs in `~/.local` or a user venv; no `sudo`.
- Do not run game binaries outside Lutris/Proton without authorization.
- Do not touch the original install; work writes to this repo
  (`docs/`, `logs/`, `analysis/`, `build/`) and to the deletable derived cache.
- `tool-inventory.md` (E2E "pending", superseded) was deleted (git history
  keeps it); this file is the canonical one.
