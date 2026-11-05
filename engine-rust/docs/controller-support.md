# Controller (gamepad) support — plan

Status: **PLAN, not implemented** (written 2026-11-05). Nothing in this
document is ticked in [`PROGRESS.md`](../../PROGRESS.md) yet; when the work
is approved it becomes new unchecked items there (see §8).

## 1. Goal and non-goals

**Goal.** Let a player use `sc_view` end to end — menus, briefings,
skirmish, campaign, replays, LAN lobby — with a standard gamepad
(Xbox / PlayStation / Switch Pro / Steam Deck layout), on Linux, Windows
and macOS, without a keyboard or mouse plugged in.

**Non-goals (YAGNI).**

- No new game rules, no aim-assist that changes what a command does, no
  "console mode" HUD redesign. The retail console layout stays as it is.
- No rumble, gyro, touchpad, light bar or per-controller glyph art in the
  first version (only listed as later options, §7 phase 4).
- No input remapping UI in the first version; the mapping is a static
  table plus an optional profile file (§5.4).

## 2. What exists today (FACT, read from the tree on 2026-11-05)

| Fact | Where |
|---|---|
| Windowing and input come from `minifb` 0.29 (`x11`, `dlopen`). `minifb` has **no gamepad API**. | `engine-rust/Cargo.toml` |
| No gamepad crate (`gilrs`, `sdl2`, …) is a dependency. | `engine-rust/Cargo.toml` |
| Input is read directly from `&Window` in 7 handlers: `handle_intro`, `handle_menu`, `handle_briefing`, `handle_replay`, `handle_pause`, `handle_result`, `handle_input` (in-game). | `src/bin/sc_view.rs:6381, 8109, 9111, 9168, 9266, 9321, 10636` |
| 29 direct `window.is_key_down / get_keys_pressed / get_mouse* / get_scroll_wheel` calls. | `src/bin/sc_view.rs` |
| The menu already has an input snapshot, `MenuInput { keys, mouse, mouse_down, wheel }`, filled either from the window or from `--menu-script`. `handle_menu_input(&MenuInput)` contains the logic. This is the pattern to generalise. | `src/bin/sc_view.rs:985, 8109-8126` |
| In-game: arrow keys scroll the camera 14 px/tick, the wheel scrolls 48 px/notch; LMB drag = box select, Shift = add, RMB = smart command, Ctrl+1..9 = control groups, hotkeys A/P/S/H/B/F/L/Tab/Space/Home/F10/Esc/±. | `src/bin/sc_view.rs:10636-10945` |
| Hotkeys per race/unit come from the shipped hotkey panel data. | `src/hotkey_data.rs` |
| The software cursor is drawn by `draw_cursor` from the retail cursor art. | `src/bin/sc_view.rs:1343, 5776` |
| All state mutations go through `World::issue` / `train` / `morph*`, recorded in the command log (replays, saves, LAN). | `AGENTS.md` §2, `src/sim.rs:1435-2121` |

**About the original game (INFERENCE).** StarCraft: Remastered
1.23.10.13515 is believed to ship no gamepad support; no `xinput*.dll` /
`dinput*.dll` appears in `research/manifests/inventory.csv`.
That is weak evidence (those DLLs are system DLLs and would not be in the
install). Falsifier: list the imports of the retail executable with REA
(`open_binary` → import table) and search for `XInputGetState` /
`DirectInput8Create` / `SDL_GameController*`. Either way, controller
support is a **client feature with no retail reference**, so it has no
visual gate to match; its tests are behavioural (§6).

## 3. Design principles applied

1. **The simulation does not know a gamepad exists.** A controller only
   produces the same high-level intents the keyboard and mouse already
   produce, and those intents call the existing public API
   (`issue`, `train`, `replay_to`, …). No new mutation path → replays,
   save/load and LAN lockstep are untouched (determinism invariant).
2. **One input path (DRY).** Keyboard, mouse, gamepad and scripted input
   all fill **one** per-tick snapshot; every handler reads only that
   snapshot. No `if gamepad { … } else { … }` copies of handler logic.
3. **Headless-testable.** The snapshot can be built from a script, as
   `--menu-script` already does, so every gamepad behaviour gets a test
   without hardware.
4. **Optional dependency.** Gamepad support sits behind a cargo feature
   (`gamepad`, on by default for release packages). Without it the build
   is byte-for-byte the current client.

## 4. Library choice (recommendation)

| Option | Pros | Cons |
|---|---|---|
| **`gilrs`** (recommended) | Pure Rust; evdev on Linux (no SDL), XInput / Windows.Gaming.Input on Windows, IOKit on macOS; ships the SDL `gamecontrollerdb` mappings, so most pads get a standard Xbox-style layout; polling API fits our fixed-tick loop. | Linux needs read access to `/dev/input/event*` (normal desktop users have it via `uaccess`; Flatpak needs `--device=input` or `--device=all`). |
| `sdl2` (already in the local cargo cache) | Very mature controller DB, rumble, gyro. | Links a C library; would sit next to `minifb` only for input; heavier packaging (Flatpak/macOS bundles). |
| Raw evdev / XInput per OS | No dependency. | Three back-ends to write and maintain; violates KISS. |

Decision to record in `research/docs/decision-log.md` before coding:
"add `gilrs` behind feature `gamepad`", with the alternatives above.

## 5. Architecture

### 5.1 One input snapshot

Generalise `MenuInput` into `InputFrame` (new module
`src/bin/sc_view/input.rs` or `src/input.rs` if tests need it from the
library):

```rust
struct InputFrame {
    keys_pressed: Vec<Key>,     // edge-triggered, as get_keys_pressed(No)
    keys_down: KeySet,          // level, as is_key_down
    pointer: Option<(i32, i32)>,// window px, real or virtual cursor
    lmb: bool, rmb: bool,       // level
    wheel: i32,                 // notches this tick
    pan: (i32, i32),            // camera pan request in px this tick
    source: InputSource,        // KeyboardMouse | Gamepad | Script
}
```

- `InputFrame::from_window(&Window)` reproduces today's reads exactly.
- `GamepadState::apply(&mut InputFrame, &Context)` merges controller input
  into the same frame (it synthesises `Key`s, button levels, pointer and
  `pan`).
- `InputFrame::from_script(...)` replaces the current `--menu-script` filler.
- Each `handle_*(&mut self, window: &Window)` becomes
  `handle_*(&mut self, input: &InputFrame)`. Migration is mechanical and
  done one handler per commit, with the existing headless shots and
  walkthrough as the regression net.

### 5.2 Virtual cursor

The game is mouse-driven (box select, smart right-click, minimap, command
card). The controller drives a **virtual cursor** that is the same pointer
the mouse moves:

- Left stick moves the cursor with a radial dead zone (default 0.15) and
  an acceleration curve (`speed = base * |v|^2`, base ≈ 900 px/s at the
  1920×1080 reference, scaled with `ui_scale`).
- When the mouse moves, the pointer follows the mouse again (last input
  wins); `source` switches accordingly so the cursor never jumps.
- The cursor is drawn by the existing `draw_cursor`; no new art.
- Pushing the cursor against the playfield edge pans the camera (same as
  edge-scroll), so one stick can play the whole map.

### 5.3 Default mapping (Xbox names; PlayStation in brackets)

**Menus / glue screens / briefings / pause / score**

| Control | Action | Existing equivalent |
|---|---|---|
| D-pad / left stick (flick) | Move focus between the screen's controls | Up/Down keys (already handled in menus) |
| A [✕] | Activate focused control | Enter / Space |
| B [○] | Back / Cancel | Escape |
| Left stick (hold) | Free virtual cursor for any control | mouse |
| LB / RB | Previous / next list page | wheel |
| Start | Confirm default button | Enter |

Focus navigation needs a **focus order** per screen. The menu code
already knows its control rectangles (`MenuHit`, `PROMPT_BUTTONS`, the
shipped dialog layouts); the plan is to derive the order from those
rects (top-to-bottom, left-to-right) instead of hand-writing lists, and
to draw the focus as the retail hover state (the screens already render
hover from the pointer, so focusing = placing the virtual cursor on the
control's centre — no new highlight art).

**In game**

| Control | Action | Existing equivalent |
|---|---|---|
| Left stick | Virtual cursor (edge pans) | mouse move |
| Right stick | Pan camera (14 px/tick at full deflection, analog) | arrow keys |
| A [✕] | Select / start box-drag (hold) | LMB |
| X [□] | Smart command at cursor | RMB |
| B [○] | Cancel targeting / deselect step | Escape |
| Y [△] | Open the **command radial** (§5.5) | command card |
| LB (hold) | Modifier: A/X act as Shift (add to selection / queue) | Shift |
| RB (hold) + D-pad | Control groups: tap = recall 1–4, hold RB+LB = assign | 1–9, Ctrl+1–9 |
| LT | Jump to last alert / base | Space / Home |
| RT | Cycle idle workers / next selection | Tab |
| D-pad (no modifier) | Quick hotkeys: Attack / Stop / Hold / Patrol | A / S / H / P |
| L3 (click) | Centre camera on selection | — (same as double-tap group) |
| R3 (click) | Toggle minimap focus: right stick moves the minimap cursor, A jumps | minimap click |
| Start | Pause menu | F10 |
| Back/View [Share] | Objectives / stats | F10 sub-menus |

**Replays**: A = play/pause, LB/RB = speed, D-pad left/right = seek,
B = exit — mapped onto the keys `handle_replay` already reads.

### 5.4 Configuration

- Stick dead zone, cursor speed, inverted Y and "swap A/B" (Nintendo
  layout) stored in the existing profile (`src/profile.rs`), which already
  has magic + version + checksum. Bump the version, keep old files
  loadable, add a corrupt-file test (AGENTS.md §3).
- `SC_GAMEPAD=0` disables the back-end at runtime; `SC_GAMEPAD_DEBUG=1`
  logs events (same style as `SC_VIEW_DEBUG`).

### 5.5 Command card on a controller

The 3×3 command card is the hardest part to reach with a cursor.
`Y` opens a radial (or a 3×3 overlay aligned with the real card) whose
nine slots are the **current** `command_slots()`; the right stick or
D-pad picks a slot and releasing `Y` calls the existing
`activate_command(index)`. This reuses the button sets
(`src/button_sets.rs`) and the hotkey data (`src/hotkey_data.rs`); the
overlay draws the retail button icons already loaded for the card.
Fallback with no overlay at all: point the virtual cursor at the card
and press A (works from phase 1).

### 5.6 Hot-plug and multiple pads

- Hot-plug handled by `gilrs` events; connect/disconnect shows the
  existing on-screen text line (no new UI). On disconnect during a match,
  the game opens the pause menu (single-player only; in LAN it does not
  pause the lockstep, it only shows the message).
- First active pad is player 1. Local split-screen is out of scope.

## 6. Tests (each ships with the code that needs it)

All tests are headless; none need a physical controller.

1. `InputFrame::from_window` equivalence: the existing menu-script and
   walkthrough produce byte-identical gate outputs after the refactor.
2. Dead zone / curve unit tests (pure functions).
3. Mapping tests: a scripted gamepad sequence (new
   `--input-script` format extending `--menu-script` with pad events)
   walks Main Menu → Single Player → skirmish start, and the screen
   sequence equals the keyboard walk.
4. In-game: scripted pad input selects an SCV, issues a move and a build;
   the resulting **command log is identical** to the same actions done by
   scripted mouse input (determinism proof; extends `tests/replay.rs`).
5. Command radial: slot index → `activate_command` for each race's
   default card.
6. Profile: new fields round-trip, old-version file still loads, corrupt
   file is rejected.
7. Feature off: `cargo build --no-default-features` (or without
   `gamepad`) compiles and passes the suite.

Manual acceptance (cannot be automated, recorded in the session log with
the pad model and OS): Xbox Series pad on Linux and Windows, DualSense on
Linux, Steam Deck built-in controls, hot-plug mid-match.

## 7. Phases

| Phase | Content | Done when |
|---|---|---|
| 0 | Decision-log entry; REA import check of the retail exe (§2 falsifier); add `PROGRESS.md` items. | decision recorded, items unchecked in PROGRESS |
| 1 | `InputFrame` refactor of the 7 handlers, **no behaviour change**. | tests 1 + all existing gates byte-identical |
| 2 | `gilrs` back-end behind `gamepad`; virtual cursor; in-game mapping; camera pan; replays. | tests 2, 4, 7 |
| 3 | Menu focus navigation; pause/score/briefing; command radial; profile settings. | tests 3, 5, 6 + headless shots of the focused menu and the radial |
| 4 (optional) | Rumble on attack alerts, controller glyphs in tooltips, remap UI, Flatpak `--device=input` permission. | each only with a test that requires it |

Rough size: phase 1 ≈ 1 session (mechanical), phase 2 ≈ 2 sessions,
phase 3 ≈ 2–3 sessions; phase 4 open.

## 8. Risks

| Risk | Mitigation |
|---|---|
| Refactor of 18 k-line `sc_view.rs` breaks a screen. | Phase 1 is behaviour-neutral and gated by the existing visual diffs (`ui_visual_diff.py`, `menu_time_diff.py`, walkthrough). |
| Gamepad input path diverges from keyboard path (two truths). | Gamepad only synthesises `InputFrame` fields; test 4 compares command logs. |
| Linux permissions / Flatpak sandbox hide `/dev/input`. | Document `uaccess`; add `--device=input` to `engine-rust/packaging/` manifest in phase 4; the client runs normally without a pad. |
| Precision RTS play is slow with a stick. | Radial command card, control groups on the D-pad, cursor acceleration, edge pan; the keyboard/mouse path keeps working at the same time. |
| LAN: pad input timing differs. | Input is turned into commands before the network layer, which only carries `RecAct`; nothing changes in `src/net.rs`. |

## 9. Checklist items to add to PROGRESS.md (when approved)

Under *Playable client (`sc_view`)*:

- [ ] Unified `InputFrame` for keyboard, mouse, script and gamepad
- [ ] Gamepad in game: virtual cursor, camera pan, select/command, control groups
- [ ] Gamepad in menus, briefings, pause, score and replays (focus navigation)
- [ ] Gamepad command card (radial) and profile settings (dead zone, speed, swap A/B)
