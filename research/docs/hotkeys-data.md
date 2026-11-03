# Hotkeys: complete data (1.23.10.13515)

## Why they had to be extracted

`webui/dist/hotkeypanel/hotkeypanel.js` contains the 220 actions
(`id`, `x`, `y`, `icon`) and the 88 units/buildings with their cards, but
**not the keys**: the native client sends them to CEF as
`OnBindingsChanged`.  Rebuilding the panel with invented data or only with
the SCV/Marine/CC/Probe/Larva captures left out the rest of the units.

## How they were extracted (read-only)

1. The retail game (64-bit) was run under `umu-run`/Wine and
   `Hotkeys` was opened.  CEF loads the page from a local server:
   `http://127.0.0.1:<port>/gamedata/webui/dist/HotkeyPanel/HotkeyPanel.html?locale=enUS&port=<port>&guid=<guid>`.
   The live URL is left in
   `drive_c/users/steamuser/AppData/Local/Temp/blizzard_browser_cache/data_*`.
2. The panel opens a WebSocket at
   `ws://127.0.0.1:<port>/panel/HotkeyPanel/<guid>` (the path comes from the
   HTML's `<route path="panel/HotkeyPanel">`).
3. On connect, the native side sends:
   - `OnActionsDiscovered`: 230 action texts.
   - `OnBindingsChanged`: 690 entries per profile, 230 unique actions,
     `$default` = Brood War and `$device` = Custom.  Each action appears 3
     times with the same value (deduplicated).
   - `OnProfilesDiscovered` and `ColorTable`.
4. The reproducible tool is
   `research/tools/extract_hotkeys_ws.py`; the raw dump is in
   `research/analysis/hotkeys-bindings-retail-1.23.10.13515.json` and the
   JS data in `research/analysis/hotkeys-actions-retail-1.23.10.13515.json`.

## Client fixture

`engine-rust/fixtures/hotkeys.json` (105 KB) merges both sources:

- `actions`: 220 actions used by some card, with `label` (localized
  text from the native side), `icon`, `x`, `y`, `default_text` and
  `default_code` from the `$default` profile.
- `units`: `terran` 29, `protoss` 28, `zerg` 31; `kind` =
  `ground`/`air`/`basic-building`/`advanced-building`; `cards` with the
  same labels (`general`, `basicStructure`, `advancedStructure`,
  `generalLifted`, `generalTankMode`, `generalSiegeMode`,
  `generalUncloaked`, `generalCloaked`, `generalBurrowed`).
- Icons are resolved via `cmdicons.N.png`; Probe 678/679 use
  272/273, like the original JS.

The shared keys match the captures: Move 48 units, Stop 52,
Attack 35, Patrol 38, Hold Position 38.  The default profile has 230
actions with a key; the 10 actions without a visible card are not included.

## Verification

- `cargo test` includes 8 fixture tests: coverage of all
  actions, 88 unique units, complete default keys, shared
  counts, CEF collisions (`Move=H` unbinds Hold Position and Build
  Hatchery on the Drone; `Move=M` collides with Train Marine on Barracks
  Lifted) and profile file round-trip/checksum.
- Headless audit: 88/88 units and buildings render their cards
  (`--shot-screen hotkeys-any-<race>-<id>`, card region with >8 000
  bright pixels).
- Captures: `docs/gates/ui-hotkeys-16.txt` (16/16) and `docs/gates/hotkeys-live-diff.txt`.
