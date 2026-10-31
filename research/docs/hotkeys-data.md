# Hotkeys: datos completos (1.23.10.13515)

## Por qué hacía falta extraerlos

`webui/dist/hotkeypanel/hotkeypanel.js` contiene las 220 acciones
(`id`, `x`, `y`, `icon`) y las 88 unidades/edificios con sus tarjetas, pero
**no las teclas**: el cliente nativo las envía al CEF como
`OnBindingsChanged`.  Reconstruir el panel con datos inventados o solo con
las capturas de SCV/Marine/CC/Probe/Larva dejaba fuera al resto de unidades.

## Cómo se extrajeron (solo lectura)

1. El juego retail (64 bits) se ejecutó bajo `umu-run`/Wine y se abrió
   `Hotkeys`.  El CEF carga la página desde un servidor local:
   `http://127.0.0.1:<port>/gamedata/webui/dist/HotkeyPanel/HotkeyPanel.html?locale=enUS&port=<port>&guid=<guid>`.
   La URL viva queda en
   `drive_c/users/steamuser/AppData/Local/Temp/blizzard_browser_cache/data_*`.
2. El panel abre un WebSocket en
   `ws://127.0.0.1:<port>/panel/HotkeyPanel/<guid>` (la ruta sale del
   `<route path="panel/HotkeyPanel">` del HTML).
3. Al conectar, el nativo envía:
   - `OnActionsDiscovered`: 230 textos de acción.
   - `OnBindingsChanged`: 690 entradas por perfil, 230 acciones únicas,
     `$default` = Brood War y `$device` = Custom.  Cada acción aparece 3
     veces con el mismo valor (deduplicado).
   - `OnProfilesDiscovered` y `ColorTable`.
4. La herramienta reproducible es
   `research/tools/extract_hotkeys_ws.py`; el volcado crudo está en
   `research/analysis/hotkeys-bindings-retail-1.23.10.13515.json` y los
   datos del JS en `research/analysis/hotkeys-actions-retail-1.23.10.13515.json`.

## Fixture del cliente

`engine-rust/fixtures/hotkeys.json` (105 KB) mezcla ambas fuentes:

- `actions`: 220 acciones usadas por alguna tarjeta, con `label` (texto
  localizado del nativo), `icon`, `x`, `y`, `default_text` y
  `default_code` del perfil `$default`.
- `units`: `terran` 29, `protoss` 28, `zerg` 31; `kind` =
  `ground`/`air`/`basic-building`/`advanced-building`; `cards` con las
  mismas etiquetas (`general`, `basicStructure`, `advancedStructure`,
  `generalLifted`, `generalTankMode`, `generalSiegeMode`,
  `generalUncloaked`, `generalCloaked`, `generalBurrowed`).
- Los iconos se resuelven por `cmdicons.N.png`; Probe 678/679 usan
  272/273, como el JS original.

Las teclas compartidas cuadran con las capturas: Move 48 unidades, Stop 52,
Attack 35, Patrol 38, Hold Position 38.  El perfil por defecto tiene 230
acciones con tecla; las 10 acciones sin tarjeta visible no se incluyen.

## Verificación

- `cargo test` incluye 8 tests del fixture: cobertura de todas las
  acciones, 88 unidades únicas, teclas por defecto completas, conteos
  compartidos, colisiones CEF (`Move=H` desasigna Hold Position y Build
  Hatchery del Drone; `Move=M` colisiona con Train Marine en Barracks
  Lifted) y round-trip/checksum del fichero de perfil.
- Auditoría headless: 88/88 unidades y edificios renderizan sus tarjetas
  (`--shot-screen hotkeys-any-<race>-<id>`, región de tarjetas con >8 000
  píxeles brillantes).
- Capturas: `docs/ui-hotkeys-16.txt` (16/16) y `docs/hotkeys-live-diff.txt`.
