# Protocolo LAN — estado final

Implementado en `engine-rust/src/net.rs` + `engine-rust/src/bin/sc_lan.rs` +
lobby en `sc_view`. Todo lo de abajo se ejecutó; lo que no se pudo ejecutar
está marcado como tal.

## 1. Transportes (misma interfaz `Transport`)

| Transporte | Estado | Mecánica |
|---|---|---|
| TCP | **hecho, probado** | framing `u32 len + payload`, `nodelay`, timeout 10 s |
| UDP | **hecho, probado** | stop-and-wait ARQ (seq + ACK + 12 reintentos), descarte de duplicados con re-ACK, handshake `0xFF`-prefijado para no confundir un saludo tardío con un ACK |

Ambos implementan `trait Transport { send(&Msg); recv() -> Msg }`, y
`host_turn`/`client_turn` son genéricos sobre él: la lógica de lockstep es
idéntica para TCP y UDP.

Extras UDP: `MAX_DATAGRAM = 1200` (un mensaje mayor se rechaza con error en
vez de fragmentarse), y contadores `retransmits`/`duplicates` para
diagnóstico.

## 2. Modelo de sincronización

- 4 ticks por turno (`DEFAULT_TICKS_PER_TURN`), checksum **cada turno**.
- Topología estrella: el host recolecta y difunde un `Bundle` con los
  comandos de cada peer; todos aplican en el mismo orden
  (`peer 0..N`, dentro de cada peer por inserción).
- Solo viajan comandos (`RecAct`), nunca estado. Mismo encoding que un
  `.gcrep`, así red y replay comparten formato (DRY).

## 3. Lobby dentro de `sc_view`

- Pantalla `MenuScreen::Lan`, alcanzable con **L** en el menú de tipo de
  partida (y `--shot-screen lan` para verificación headless).
- **S: scan** — descubre partidas reales por UDP broadcast
  (`DISCOVERY_PORT = 7778`), lista `nombre - ip [puerto]`.
- **H: host** — abre el lobby en `:7777`, se anuncia cada segundo y acepta
  al primer peer (handshake no bloqueante: el menú sigue dibujando).
- **ENTER: join** — handshake con el lobby seleccionado; el host decide
  mapa y razas, y ambos construyen el mundo con `build_from_save` para
  arrancar en estado idéntico.
- En partida, todas las órdenes pasan por `send_order`/`send_train`: si hay
  sesión LAN se encolan para el siguiente turno; si no, se aplican en local.

## 4. Reconexión tras caída

Diseño: **no se transfiere estado, se transfiere el log**.

1. La conexión se cae → `host_turn` devuelve error → el cliente queda fuera.
2. El peer vuelve y pide `Rejoin { peer, map_hash, seed }`.
3. El host responde `Resync { turn, log }` con el `Recording` completo.
4. El peer reconstruye el mundo con su copia del mapa y hace fast-forward
   determinista hasta `turn`; después continúa el lockstep normal.

Cubierto por `tests/lan.rs::peer_reconnects_and_resyncs_from_the_command_log`
(8 turnos → caída → resync → 8 turnos → hash final idéntico al host).

## 5. Pruebas ejecutadas (evidencia)

| Prueba | Resultado |
|---|---|
| `cargo test --lib net::` | 7/7 (mensajes, merge, UDP bidireccional, duplicados, discovery) |
| `cargo test --test lan` | 3/3 (TCP 20 turnos, mapa distinto rechazado, reconexión+resync) |
| `cargo test` (todo) | **256 passed, 0 failed** |
| `cargo clippy --all-targets` | 0 avisos |
| TCP, 2 procesos, Boxer.chk, 600/1200/2400 ticks | hash idéntico en ambos, exit 0 (2400 = 600 turnos/checksums) |
| UDP, 2 procesos, Boxer.chk, 600 ticks | hash idéntico `bdf9377e0487c36f` en ambos, exit 0 |
| `sc_view --shot-screen lan` | render 1280x960 OK |

## 6. Prueba física en dos máquinas — NO EJECUTADA

No puedo ejecutarla: esta sesión corre en una sola máquina y no tengo acceso
a una segunda. Arnés listo y verificado sintácticamente:

```bash
# PC 1
scripts/lan_two_machines.sh host  192.168.1.20 terran  /ruta/map.chk /ruta/platform.vf4 tcp
# PC 2
scripts/lan_two_machines.sh join  192.168.1.20 protoss /ruta/map.chk /ruta/platform.vf4 tcp
# (añade "udp" como 6.º argumento para probar UDP)
```

Criterio de PASS: ambos exit 0, sin línea `DESYNC`, y las dos líneas
`LAN_RESULT` con el **mismo** `final hash`.

## 7. Pendiente (honesto)

- Lobby de más de 2 jugadores en la UI (el protocolo soporta hasta 8;
  `sc_lan` TCP ya acepta N-1 clientes, la UI abre 1v1).
- Chat en la UI (el mensaje `Chat` existe y está probado en el codec).
- Reconexión automática (hoy requiere que el peer vuelva a entrar; no hay
  reintento con backoff).
- Migración de host: si cae el host, la partida termina (fuera de alcance).
