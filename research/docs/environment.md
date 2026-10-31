# Entorno de trabajo — GalaxyCraft / StarCraft 1.23.10.13515

Registro de Fase 0 (descubrimiento). Comandos de solo lectura; detalle crudo en
`logs/environment-check.log` (generado 2026-10-28T12:25:55Z).

## Sistema

| Campo | Valor |
|---|---|
| Distribución | Bazzite 44.20261026.0 (Kinoite, base Fedora 44, imagen inmutable) |
| Kernel | Linux 7.2.11-ogc4.1.fc44.x86_64 |
| Arquitectura | x86_64 |
| CPU | AMD Ryzen 7 9800X3D (8 núcleos / 16 hilos) |
| RAM | 30 GiB (20 GiB disponibles en la comprobación) |
| Swap | 15 GiB |
| Disco `/var/home` | 929 G, 559 G libres (40 % usado) |
| GPU | NVIDIA GeForce RTX 5070 Ti 16 GB (driver 615.78.08) + AMD Radeon 610M iGPU |

## Runtime de herramientas

| Herramienta | Versión / ruta |
|---|---|
| git | 2.56.0 |
| Node.js | v26.12.1 (linuxbrew) |
| npm | 11.20.0 |
| Python | 3.14.8 (`file`, `objdump`, `xxd`, `sha256sum` presentes) |
| Rust / Cargo | 1.99.1 |
| gcc | 16.2.1 |
| Ghidra | 12.1.4 y 12.2_PUBLIC en `~/.local/share/ghidra/` + JDK Temurin 21.0.13+9 |
| OpenCode | v2.2.0 (`~/.opencode/bin/opencode`). Cadena oficial de análisis |
| Hermes Agent | **no se usa** (decisión 2026-10-28: fuera de la cadena de trabajo) |
| CLI `rea` | no existe en PATH (se usa vía MCP `rea-agents@6.8.0`) |
| Contenedores | `podman`, `distrobox` disponibles; `docker` ausente |
| Wine | sin binario `wine` en PATH (el juego se ejecuta vía Lutris/Proton) |

Notas Bazzite: todo vive en el directorio personal, sin `sudo`. La imagen
inmutable queda intacta.

## Rutas del proyecto

- Instalación original (intacta, solo lectura):
  `~/Games/battlenet/drive_c/Program Files (x86)/StarCraft` (~5.5 GB,
  build 1.23.10.13515 en `.build.info`).
- Espacio de trabajo RE (canónico, no duplicar):
  `~/ReverseEngineering/` (`original/ work/ analysis/ extracted/
  reconstructed/ tests/ build/ docs/ logs/ manifests/ tools/`).
  Inventario canónico: `~/ReverseEngineering/analysis/file-inventory.csv`
  (724 archivos, 5 879 150 935 B con SHA-256).
- Reconstrucción Rust (este repo): `~/Documents/galaxycraft`
  (`src/`, `tests/`, `tools/`, `fixtures/`, `assets/` de conveniencia).
  `original/` nunca se copia ni se modifica; `src/retail.rs` lee la
  instalación del usuario con `--install` y materializa un caché derivado
  borrable en `$XDG_CACHE_HOME/galaxycraft/`.

## Diferencias con el registro previo (2026-10-28T06:50)

- Bazzite pasó de `44.20261012.0` / kernel `7.2.9` a `44.20261026.0` /
  kernel `7.2.11`. Herramientas y versiones de runtime sin cambios.
- Decisión 2026-10-28: Hermes queda fuera de la cadena de trabajo. Cadena
  oficial: OpenCode + MCP `rea` (ver `docs/rea-opencode-integration.md`).
