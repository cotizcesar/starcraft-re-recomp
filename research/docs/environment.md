# Working environment — GalaxyCraft / StarCraft 1.23.10.13515

Phase 0 (discovery) record. Read-only commands; raw detail in
`logs/environment-check.log` (generated 2026-10-28T12:25:55Z).

## System

| Field | Value |
|---|---|
| Distribution | Bazzite 44.20261026.0 (Kinoite, Fedora 44 base, immutable image) |
| Kernel | Linux 7.2.11-ogc4.1.fc44.x86_64 |
| Architecture | x86_64 |
| CPU | AMD Ryzen 7 9800X3D (8 cores / 16 threads) |
| RAM | 30 GiB (20 GiB available at check time) |
| Swap | 15 GiB |
| Disk (home volume) | 929 G, 559 G free (40 % used) |
| GPU | NVIDIA GeForce RTX 5070 Ti 16 GB (driver 615.78.08) + AMD Radeon 610M iGPU |

## Tool runtime

| Tool | Version / path |
|---|---|
| git | 2.56.0 |
| Node.js | v26.12.1 (linuxbrew) |
| npm | 11.20.0 |
| Python | 3.14.8 (`file`, `objdump`, `xxd`, `sha256sum` present) |
| Rust / Cargo | 1.99.1 |
| gcc | 16.2.1 |
| Ghidra | 12.1.4 and 12.2_PUBLIC in `~/.local/share/ghidra/` + JDK Temurin 21.0.13+9 |
| OpenCode | v2.2.0 (`~/.opencode/bin/opencode`). Official analysis chain |
| Hermes Agent | **not used** (decision 2026-10-28: out of the work chain) |
| `rea` CLI | not on PATH (used via MCP `rea-agents@6.8.0`) |
| Containers | `podman`, `distrobox` available; `docker` absent |
| Wine | no `wine` binary on PATH (the game runs via Lutris/Proton) |

Bazzite notes: everything lives in the home directory, without `sudo`. The immutable
image stays intact.

## Project paths

- Original install (intact, read-only):
  `$SC_INSTALL` (`.../Program Files (x86)/StarCraft`, ~5.5 GB,
  build 1.23.10.13515 in `.build.info`).
- RE workspace (canonical, do not duplicate):
  `research/` (`original/ work/ analysis/ extracted/
  reconstructed/ tests/ build/ docs/ logs/ manifests/ tools/`).
  Canonical inventory: `research/manifests/inventory.csv`
  (724 files, 5 879 150 935 B with SHA-256).
- Rust reconstruction (this repo): `engine-rust/`
  (`src/`, `tests/`, `tools/`, `fixtures/`, convenience `assets/`).
  `original/` is never copied or modified; `src/retail.rs` reads the
  user's install with `--install` and materializes a deletable derived cache
  in `$XDG_CACHE_HOME/galaxycraft/`.

## Differences from the previous record (2026-10-28T06:50)

- Bazzite went from `44.20261012.0` / kernel `7.2.9` to `44.20261026.0` /
  kernel `7.2.11`. Tools and runtime versions unchanged.
- Decision 2026-10-28: Hermes is out of the work chain. Official
  chain: OpenCode + MCP `rea` (see `docs/rea-opencode-integration.md`).
