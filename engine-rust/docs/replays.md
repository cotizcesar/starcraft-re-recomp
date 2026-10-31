# Repeticiones (`gcrep` / `gcsave`): cómo grabar, verificar y reproducir partidas

Una repetición de GalaxyCraft es **el mapa + el registro de comandos** de la
partida. No es un vídeo ni una copia del estado: al abrirla se reconstruye el
mundo desde el `.chk` embebido y se reaplican los comandos tick a tick. Como la
simulación es determinista (LCG con semilla fija y `World::issue` como única
puerta de entrada de órdenes), el resultado es idéntico al de la partida
original, y eso se comprueba con un hash de estado.

---

## 1. Grabar desde el juego

| Acción | Tecla | Qué escribe |
|---|---|---|
| Guardar partida | `F5` | `save-NNN.gcsave` en `<dir>/saves/` |
| Guardar repetición | `F6` | `replay-NNN.gcrep` en `<dir>/replays/` |

`<dir>` es, por orden:

1. `$GALAXYCRAFT_SAVES`, si está definido,
2. `$XDG_CACHE_HOME/galaxycraft`, si está definido,
3. `~/.cache/galaxycraft`.

Ejemplo real en esta máquina:

```bash
$ ls ~/.cache/galaxycraft/replays ~/.cache/galaxycraft/saves
```

Para llevarte una repetición a otro equipo basta copiar el `.gcrep`: lleva el
`.chk` dentro, así que no depende de que el mapa siga estando instalado.

```bash
cp ~/.cache/galaxycraft/replays/replay-001.gcrep /tmp/mi-partida.gcrep
```

El mensaje en pantalla al guardar confirma la ruta y el tick:

```
Replay saved /home/<usuario>/.cache/galaxycraft/replays/replay-001.gcrep
```

## 2. Reproducir

### Desde el menú

`Multiplayer → Replays` (o `Single Player → Load Replay`) lista los `.gcrep`
encontrados; `↑`/`↓` selecciona, `Enter` reproduce y `Supr` borra.

Durante la reproducción:

| Tecla | Acción |
|---|---|
| `Espacio` | pausa / reanuda |
| `+` / `-` | velocidad arriba / abajo |
| `Esc` | detener y volver al menú |

### Desde la línea de comandos

```bash
cargo run --release --bin sc_view -- \
  --install "/ruta/a/StarCraft" \
  --replay /tmp/mi-partida.gcrep
```

`--replay` acepta un `.gcrep` **o** un `.gcsave` (en ambos casos se abre como
repetición, sin IA). Si el fichero no es un guardado válido el cliente sale con
código 1 y un mensaje claro, en vez de abrir el menú.

Para una captura sin ventana (CI, comparación de pantallas):

```bash
cargo run --release --bin sc_view -- --replay /tmp/mi-partida.gcrep \
  --width 3440 --height 1440 --shot /tmp/replay.bmp --shot-screen replay
```

## 3. Crear, inspeccionar y verificar sin jugar

`sc_replaytool` graba una partida IA-vs-IA determinista, imprime lo que contiene
un fichero y comprueba que dos reproducciones del mismo log coinciden.

```bash
# 1) grabar una demo (25 s de juego, mapa incluido)
cargo run --release --bin sc_replaytool -- \
  make --chk assets/maps/Boxer.chk --ticks 600 --out /tmp/demo.gcrep

# 2) ver qué contiene
cargo run --release --bin sc_replaytool -- info /tmp/demo.gcrep

# 3) verificar determinismo (dos pasadas, mismo hash)
cargo run --release --bin sc_replaytool -- verify /tmp/demo.gcrep
```

Salida comprobada en esta máquina:

```
$ ./target/release/sc_replaytool make --chk assets/maps/Boxer.chk --ticks 600 --out /tmp/opencode/demo.gcrep
wrote /tmp/opencode/demo.gcrep  map "Boxer"  tick 600  21 commands  final hash f1127320cf799060

$ ./target/release/sc_replaytool verify /tmp/opencode/demo.gcrep
map Boxer                    tick 600
commands 21
run #1   f1127320cf799060
run #2   f1127320cf799060
OK: the command log replays identically
```

`run #1` puede compararse además con el hash que imprimió `make`: si coincide,
la partida grabada y la reproducida son el mismo estado.

## 4. Formato (para copiarlo o reimplementarlo)

Todo entero es little-endian. Dos contenedores: `gcsave` (envoltorio) y `gcrep`
(registro puro).

### `.gcrep` — `GCS1`, versión 2

```
u32  magic  0x47435331 ("GCS1")
u16  version 2
u64  seed                 semilla del RNG del mundo
u8[32] map_hash           SHA-256 del CHK (0 si se creó a mano)
u32  cmd_count
cmd* comando:
     u64 tick             tick en el que se aplica (antes de step)
     u8  kind             0 = Issue, 1 = Train, 2 = Morph, 3 = MorphBuilding
     ... payload según kind
u8[32] sha256(payload)    checksum de todo lo anterior
```

Payload de cada comando:

| kind | payload |
|---|---|
| 0 `Issue` | `u32 unit` + orden (tag `u8` + campos) |
| 1 `Train` | `u32 building` + `u16 unit_type` |
| 2 `Morph` | `u32 larva` + `u16 unit_type` |
| 3 `MorphBuilding` | `u32 source` + `u16 unit_type` |

Órdenes (`Order`, tag = orden de declaración): `0 Idle`, `1 Stop`,
`2 Move{pos}`, `3 AttackMove{pos}`, `4 AttackUnit{target}`, `5 HoldPosition`,
`6 Patrol{a,b,forward}`, `7 Gather{resource}`, `8 ReturnCargo{depot}`,
`9 Build{type,pos}`, `10 Repair{target}`, `11 Train{type}`, `12 Research{tech}`,
`13 Upgrade{id,level}`, `14 Cast{spell,target}`, `15 EnterTransport{unit}`,
`16 Unload`, `17 Burrow`, `18 Unburrow`, `19 Siege`, `20 Unsiege`, `21 Lift`,
`22 Land{pos}`, `23 Die`.

### `.gcsave` — `GCSF`, versión 1

```
u32  magic  0x47435356 ("GCSF")
u16  version 1
str  map_name             u32 len + bytes (máx. 1024)
u32  chk_len
u8[] map_chk              CHK embebido (máx. 8 MiB)
u32  race_count
u8[] races                0 Zerg, 1 Terran, 2 Protoss, 3 Neutral
u32  local_player
u64  tick                 tick del guardado
u32  rec_len
u8[] rec                  un `.gcrep` completo
u8[32] sha256(payload)
```

Límites que el lector aplica antes de reservar memoria: `chk_len ≤ 8 MiB`,
`cmd_count ≤ 10 000 000`, `race_count ≤ 12`. Un fichero que no cumpla el magic,
la versión, el checksum o el tamaño se rechaza con `None` (nunca entra en
pánico): ver `tests/replay.rs::corrupt_file_is_rejected`.

## 5. Restaurar: cómo se reaplica

`replay_to(world, rec, upto_tick)` (en `src/replay.rs`) es la única
implementación de la reproducción, compartida por guardado, repetición y LAN:

```text
mientras world.tick < upto_tick:
    aplicar los comandos estampados en world.tick (en orden)
    world.step()
aplicar los comandos estampados exactamente en upto_tick (sin step)
```

Por eso un guardado cargado y una repetición vista usan exactamente el mismo
camino que la partida en vivo, y `state_hash(&world)` debe coincidir con el de
la sesión original en el mismo tick (`tests/replay.rs`:

- `savefile_roundtrip_with_embedded_map`
- `boxer_with_ai_save_reload_replays_identically`).

## 6. Problemas típicos

| Síntoma | Causa | Solución |
|---|---|---|
| `cannot replay X: not a GalaxyCraft save/replay` | fichero `.rep` retail o corrupto | usa `sc_replay` para inspeccionar `.rep` de StarCraft; GalaxyCraft solo lee `gcrep`/`gcsave` |
| la reproducción se detiene antes | el `.gcrep` se grabó a mitad de partida | es normal: `tick` es el último tick grabado |
| el hash final no coincide | se editó el `.gcrep` | recalcula el checksum grabando de nuevo; `sc_replaytool verify` lo detecta |
