# AGENTS.md — Reglas del proyecto starcraft-re-recomp

> Hay un solo motor: `engine-rust` (fidelidad total: 228 unidades,
> 189 órdenes, 137 handlers transcritos; el número de tests crece cada
> fase — no citarlo aquí, ver `cargo test`). El experimento en C se
> eliminó el 2026-10-28 tras migrar todo lo útil (save/load, replay-hashes,
> red-`socketpair` → diseño y tests en Rust); su historial vive en
> `research/re-c.bundle` y sus evidencias en
> `research/docs/rea-evidence-c.md`. Un motor, una verdad (DRY).
> Reglas de crate (comandos, gotchas retail/headless): `engine-rust/AGENTS.md`.

## 1. Estructura (una sola carpeta, sin duplicados)

```text
starcraft-re-recomp/
  README.md / AGENTS.md          # este archivo: contrato del proyecto
  engine-rust/ (+ AGENTS.md)     # ÚNICO motor (Rust, jugable) + sus reglas de crate
  scripts/                       # arneses: lan_two_machines.sh, render_screens.sh, compare_*.py
  research/                      # ingeniería inversa: evidencias, dumps, Ghidra, docs, planes
    docs/                        # PROJECT-STATUS.md = estado vivo; session-log.md = bitácora
    analysis/                    # inventarios, funciones, xrefs, decompilados
    tools/                       # scripts Ghidra (Java) + captura de memoria
    build/                       # proyectos Ghidra (grandes, no tocar a mano)
    work/                        # dumps de memoria (solo lectura)
    re-c.bundle                  # historial del motor C eliminado
    docs/rea-evidence-c.md       # evidencias REA del motor C
```

- El original (`~/Games/.../StarCraft`) es **solo lectura**. Nada escribe ahí.
- `~/ReverseEngineering` → `research/`. El enlace legacy
  `~/Documents/galaxycraft` se eliminó el 2026-10-29: la única ruta es
  `~/Documents/starcraft-re-recomp/`. No crear rutas paralelas nuevas.
- `session-log.md` es **append-only**: la historia no se reescribe (los
  errores y falsaciones quedan registrados con fecha).

## 2. Principios (SOLID / DRY / YAGNI / KISS aplicados a RE)

- **Evidencia antes que suposición.** HECHO (observado) vs INFERENCIA
  (con hipótesis y falsador). Nada de nombres inferidos como confirmados.
- **DRY real:** una sola implementación por comportamiento. La lógica
  compartida vive en un sitio (`replay_to` ↔ `apply_act` es el ejemplo a
  seguir: el reproductor de `sc_view` reutiliza `apply_act`, no la duplica).
  El motor C se eliminó en vez de mantener dos implementaciones.
- **YAGNI:** sin sistemas especulativos. LAN lockstep (TCP+UDP,
  descubrimiento, lobby, resync) está **dentro** del alcance y ya existe
  (`src/net.rs`, `sc_lan`, `tests/lan.rs`); falta solo la prueba física
  en 2 PCs (`scripts/lan_two_machines.sh`). **Battle.net/online: fuera
  de alcance.** Nada de skins/features sin test que los pida. Cada
  fichero nuevo debe responder a una tarea del plan.
- **KISS + SOLID en el código:** funciones pequeñas con una sola razón para
  cambiar; `World` no conoce sockets: la red solo entrega comandos
  (`RecAct`) y compara hashes (`state_hash`); la UI no toca estado
  interno salvo por la API pública (`issue`, `train`, `replay_to`...).
- **Determinismo como invariante:** el sim es LCG con semilla fija; toda
  mutación externa entra por `World::issue` o por los mutadores directos
  auditados (`train`, `morph`, `morph_unit`, `morph_building`). Cualquier vía nueva que
  mute estado **debe** registrarse en el command log o romperá
  save/load, replays y LAN. Hay test que lo demuestra (`replay.rs`).

## 3. Rust (`engine-rust`)

- `cargo fmt` antes de cada cambio; `cargo clippy --all-targets` debe dar
  **0 avisos** (el proyecto compila con `-D warnings` en CI local).
- Todo comportamiento nuevo o corregido lleva **test que falla sin el fix**
  (unit en `src/` o integración en `tests/`). Los tests usan rutas
  relativas a assets/fixtures; nunca dependen de la instalación retail
  salvo los marcados `retail` (con `SC_INSTALL`).
- Formatos de fichero propios: magic + versión + checksum SHA-256, límites
  de tamaño explícitos antes de reservar (`n > 10_000_000 → None`), y test
  de fichero corrupto.
- Sin `unwrap` en rutas de carga de datos de usuario (devolver `Option` /
  `Result` con mensaje); `expect` solo en tests.
- `World` no gana campos "por si acaso": cada campo nuevo necesita lector
  (tests) y escritor (save/hash) o no entra.

## 4. Validación diferencial (sin segundo motor)

- El motor C se eliminó; su rol de oracle lo cubren: el test Boxer+IA
  (replay idéntico), `tools/oracle.py` (diferencial de tablas), los 73/73
  arrays contra el juego vivo y `re-c.bundle` como referencia histórica.
  No resucitar un segundo motor salvo decisión explícita con justificación.

## 5. Ingeniería inversa (`research/`)

- Solo lectura sobre el original y los dumps (`pread` de `/proc/<pid>/mem`,
  sin depurador, sin escritura en el proceso). Las copias de trabajo van
  a `/tmp`, nunca se toca el original.
- Conexión REA: herramientas `tools.rea.*` vía MCP en la sesión OpenCode.
  Requiere bloque `environment` en la entrada `mcp.rea` de `opencode.json`
  (`GHIDRA_INSTALL_DIR`, `JAVA_HOME`→JDK 21, `REA_GHIDRA_STARTUP_TIMEOUT_MS`);
  pin Ghidra **12.1.4** (12.2 instalado pero REA pide 12.1.x).
  Detalle verificado: `research/docs/rea-opencode-integration.md`.
- Flujo por objetivo: `open_binary` → consultas acotadas
  (`list_procedures`, `search_strings`, `read_bytes`) antes de escalar →
  `binary_session` para trabajo profundo → `close_binary`. Ghidra solo
  aparece como candidato tras `open_binary`; `raw-binary` exige
  `architecture` + `entry_point`.
- Límite duro: `binary_overview` vía MCP en imágenes >10 MB da
  `provider_timeout` determinista — prohibido repetirlo. Alternativa:
  `analyzeHeadless` por ruta absoluta, con `JAVA_HOME` al JDK 21, en fondo,
  y scripts `research/tools/*.java` pequeños y nombrados por propósito.
  Registro del fallo: `research/docs/tool-errors.md`.
- Las anotaciones (`annotate_native_function`, `set_address_name`) mutan
  solo la base efímera de Ghidra, nunca el ejecutable.
- Pseudocódigo ≠ fuente original. Direcciones de mapeos viejos no valen en
  dumps nuevos (rebase explícito o nada).
- Cada hallazgo: archivo + dirección + confianza + experimento que lo
  confirmaría o refutaría. Las falsaciones también se publican.
- Índice de docs: `research/docs/README.md`. `research/docs/archive/` es
  histórico superado (no usar como referencia).

## 6. Documentación

- `research/docs/PROJECT-STATUS.md` se actualiza al cerrar cada
  fase, no al final.
- `research/docs/architecture.md` es el mapa; los informes de pista viven
  en `research/analysis/investigations/` y no se reeditan tras la síntesis
  (solo se referencian).
- `engine-rust/README.md` + `features.md` describen producto; este repo
  describe proceso. No duplicar contenido entre ambos: enlazar.

## 7. Definition of Done (verificable, sin excepciones)

Nada se declara terminado sin pegar la prueba observable:

- `cargo test` (desde `engine-rust/`) → 0 failed, incluyendo los tests nuevos.
- `cargo clippy --all-targets` → 0 avisos.
- `cargo build --bin sc_view` OK + shots headless si toca UI
  (`--shot-screen saved|replays|pause`).
- Docs tocadas + `session-log.md` con fecha. Sin "funciona en mi máquina"
  sin comando pegado.
