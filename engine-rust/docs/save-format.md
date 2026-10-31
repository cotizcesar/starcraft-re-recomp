# Save/Load + Replays — diseño (P0, comparte infra con LAN)

Decisión: **command-log + fast-forward**, no snapshot de `World`.
Motivo: `World` contiene `Arc<GameData>`, cachés (`index`, `obstacles`),
scratch y RNG; el sim es determinista (LCG) y `World::issue` es el único
punto de entrada de órdenes (jugador en `sc_view.rs`, IA en `ai.rs`).
Guardar = estado inicial + comandos; cargar = reconstruir y reejecutar.

## Formato `.gcsave` / `.gcrep` (binario, magic + versión)

- `GCS1` magic u32 + version u16.
- seed u64, map_hash (sha256 CHK) 32 B, setup (razas, slots, tick base).
- cmds: `[(tick u64, unit u32, order Order)]`, `Order` con discriminante u8
  + campos manuales (sin serde: estable entre versiones).
- checksum final (detección de corrupción/desync).

## Restaurar

`World::from_scenario` + aplicar cmds por tick hasta T. Medido:
5 000 ticks en ~12 s wall (release) → partida 20 min (~28 800 ticks)
carga en ~70 s. Aceptable con aviso; el snapshot total queda descartado
por invasivo (cachés/`Arc`/scratch).

## Reutilización

- Replays: el mismo log (+ cabecera de resultado). Reproductor = mismo
  fast-forward con pausa/velocidad.
- LAN: solo comandos humanos por turno + checksum periódico de estado
  (`sc_state_hash` existe en `sc-c`; portar idea a Rust). Ver
  `research/docs/lan-protocol.md`.
- Hook: `World::issue` con grabador opcional (`Option<Recorder>`, coste
  cero si ausente). La IA también pasa por `issue`: sus comandos se
  graban igual (deterministas, sin daño).

## Aceptación P0

Guardar a mitad de misión, salir, cargar, continuar idéntico (hash de
estado igual al de la sesión original en el mismo tick); replay que
reproduce el mismo resultado; perfil/campaña persistidos en disco.
