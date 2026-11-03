# REA ↔ OpenCode integration — real verification (without Hermes)

Date: 2026-10-28. OpenCode chain only. Hermes out of service by
project decision; mentions in `research/` remain as
history.

## 1. Current state

| Test | Command | Result 2026-10-28 |
|---|---|---|
| OpenCode | `opencode --version` | v2.2.0 |
| MCP list | `opencode mcp list` | 4/4 `connected`: `comfyui`, `godot`, `gsd`, **`rea`** |
| `rea` config | `~/.config/opencode/opencode.json` → `mcp.rea` | `command: npx -y rea-agents@6.8.0 mcp`, `enabled: true`, **with** `environment` block |
| Environment | `GHIDRA_INSTALL_DIR`, `JAVA_HOME`, `REA_GHIDRA_STARTUP_TIMEOUT_MS=900000` | present in the `rea` entry |
| Ghidra provider (CLI, with env) | `capabilities` | `status: available`, Ghidra 12.1.4, JDK 21.0.13, `analyzeHeadless` and decompiler OK |
| Ghidra provider (CLI, without env) | `capabilities` | `not_configured` (confirms the env is what enables it) |

The gap documented in the earlier version of `research/docs/rea-opencode-integration.md`
(`rea` entry without `environment`) **is closed**: the current `opencode.json`
already includes the block.

## 2. Verified E2E (Phase 3 — PASS 2026-10-28)

`open_binary` + `binary_overview` + `binary_session` + `close_binary` from
OpenCode via MCP on the copy `/tmp/opencode/test_launcher.exe` (original
intact, sha256 `ab51787f…1963a`): **PASS**.

- `open_binary`: PE x86 OK.
- `binary_overview`: 20 862 procedures, 13 326 strings, 7 segments,
  `image_base 0x400000` (matches the historical record).
- `binary_session`: binding `ghidra 12.1.4` (`auto-single-candidate`,
  `analyzer_preset=ghidra-default`), groups available.
- `close_binary`: OK. Full log in `logs/integration-tests.log`.

## 3. Notes

- Ghidra 12.2 also installed, but REA requires the 12.1.x pin; use 12.1.4.
- `analyzeHeadless` only by absolute path; it is not on PATH.
- Do not run game binaries outside Lutris/Proton without authorization.

## 4. Official documentation

- Installation: https://github.com/morluto/rea/blob/main/docs/installation.md
- Project: https://github.com/morluto/rea
- Guides: https://morluto.github.io/rea/
