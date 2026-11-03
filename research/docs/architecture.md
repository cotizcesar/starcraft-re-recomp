# Architecture map — StarCraft 1.23.10.13515 (Phase 5)

Date: 2026-10-28. Synthesis of 4 parallel tracks
(`analysis/investigations/entry-loop.md`, `orders-iscript.md`,
`diff32-64.md`, `triggers-data.md`). Everything verified against the reports;
here it is only integrated, not re-analyzed. 32-bit addresses = dump VA
(base `0x400000`); 64-bit = absolute VA (base `0x6ffffade0000`).

Validated rebase formula (A-H10, 4 routines): `VA_dump32 = VA_rt − 0x79fa0000 + 0x400000`.

## 1. Startup (high confidence)

- `entry` (`0x4a73a0`, 5 B) = `AddressOfEntryPoint` → `jmp` to MSVC `mainCRTStartup`
  (`___security_init_cookie`, `__initterm`, `__cexit`). MSVC toolchain
  confirmed; the game does not start at `entry`.
- Game gateway: **`0x70e810`** (90 B, 1 single caller = the CRT, SEH
  prologue, loads `Starcraft.ini` via `0x70ac50`, publishes `argv` at `0x117c834`).
- Confirmed init: `0xc61f70` (window + GL context, `pm_gles::Context::Init`
  = the remaster's own GL layer), `0x8aee30` (Mouse/Video/Network/Sfx settings
  orchestrator, 317 calls, 0 E8 callers → invoked via
  pointer/thread), GL loaders (`0xc60b80`/`0xc5fe60`), bzip2, FMOD,
  CASC streaming, WMV cinematics, DX error decoder (`0x4f3828`).
- Ubiquitous infra (background noise for xrefs): `0x93ec70` (11 166 callers),
  `0x538810` (5 731), `0xc046b0` 361-case dispatcher (2 941, to be
  identified: assert/log/getString?).

## 2. Loop and messages (updated: WndProc CONFIRMED)

- **`0x945050` = main WindowProc** (3 759 B, 57 IAT refs / 25 slots,
  `switch(msg)` 67 cases, `msg<0x100`, `msg==0x400`, `wParam&0xffff`,
  0 xrefs = registered callback). Verdict after report A.
- OO forwarding: `0x9468e0` (103 B, ×20 from the WndProc) resolves the object via
  IAT `0xd305a8` and jumps to vtable+4 with (msg,wParam,lParam) — a
  message→object thunk. Confirms dispatch through objects, not a monolithic switch.
- **`0x969f0d` reclassified**: 0 IAT (pure internal logic) → simulation/worker
  step, NOT a message pump. Adjacent: `0xa80e70`.
- External `GetMessage` pump not located (low priority: the
  reimplemented client runs its own loop). IAT confirmed at `0xd301xx`.

## 3. Orders + iscript (system present, identification pending)

- FACT: both dumps contain the system (`arr\*.dat`, `scripts\*.bin`,
  RTTI `COrder`/`CUnit`). The **row names are not** in the binary
  (13/13 absent): locate them via anchors (`STR_*/SND_*`, `rescue_*`,
  `nukeWarning`, counters) — anchor table per family in report B §1.3.
- Family ranking (1 Cast … 10 Patrol; details and size-exact pools in B
  §1.5/§2). `TARGET_ORDER` signature: **falsified in the image** — only the
  identity table exists (`.data 0xb2a570` / `0xdd5190`); the remap lives in the heap or the
  old reference does not transfer. Hunt for the dispatcher by switch structure.
- **Nuke CONFIRMED** (`FUN_005d3823`, 440 B, anchor `SND_NUKE_LAUNCH`):
  `switch(CUnit+0x4e)` cases 0–6, transitions `+0x4e=3/5`. Single caller:
  `FUN_005ca229` (+0x41, UNCONDITIONAL_CALL; 0 xrefs to it → via a table).
  Callees = error glue. B-exp.1 (cast size-531): no `+0x4e` stores →
  size-transfer hypothesis weakened.
- Experiments ready: 8 with candidate addresses and pseudocode signatures
  (stores `CUnit+0x4e/0x54/0x84/0x85/0x55` + number of callees).

## 4. Data and triggers (engine tables: yes; loaded map: no)

- CHK parser tables in `.rdata` (identical): long registry (~20
  sections, C6) + short dispatch tied to `League_%d.cnd` (8 sections,
  C1; asymmetry `UNIT` yes / `TRIG,MRGN` no → melee/ladder path).
- Resident SFX table byte-identical (C2); `(id,fn,fn,id)` tuples (C4);
  100 % pointer tables (C5, reference for "static").
- Strong negatives: 0 intact DAT columns, 0 TRIG-2400, 0 `melee.trg`,
  0 STR, 0 briefing/objectives, UNIT/MRGN clusters = false positives
  (zeros / counter table). `.data` ~90 % zeroed BSS.
- **Verdict D: the in-game dump does not contain the loaded map+triggers** —
  the dump covers the module image; the parsed map lives in uncaptured
  heap. Decisive experiment: re-dump RW regions from `/proc/pid/maps`
  and repeat the signatures (requires the game running + authorization).

## 5. 32 vs 64 bits (H2 confirmed, high confidence)

- Same manifest `1.23.10.13515` + hash `2e031d5be4`, exact RTTI mirror
  (415 lambdas, 24 chat, 50 glue), shared engine vocabulary,
  global size shift ×1.16–1.83 with twin tails.
- Recommendation C: **32-bit analysis base**, 64-bit anchors via
  RTTI pairs; 64-bit only if the goal requires it.

## 6. Dependency graph (confirmed vs inferred)

Confirmed: CRT → `0x70e810` → config (`Starcraft.ini`); settings init →
{window/GL, FMOD audio, streaming, cinematics}; map loader → MPQ
(`LoadMapPass2`) → CHK parser (C6/C1) → [uncaptured heap].
Inferred: `0x969f0d` → {input, sim, render}; 189-row dispatcher →
137 handlers; static tables → session (C4 to be verified).

## 7. Confirmation plan (ordered by signal/noise ratio)

1. Decompile `0x70e810` + `0x70ac50` (`main` signature, global `0x117c834`).
2. Resolve the IAT (`0xd30000–0xd31000`) and label `0xc61f70`/`0x969f0d`
   (look for `PeekMessage/Sleep/timeGetTime`).
3. Generic cast size-531 (4 in 32-bit): 25 callees + store `+0x4e=1`.
4. Xrefs to `nukeWarning`/`SND_NUKE_*` → handlers for rows 124–128.
5. Search for `TARGET_ORDER` (189 B, 10 divergences) → dispatcher.
6. Falsification of Patrol size-69 (6 callees `…4dab0,57630…`).
7. C1/C6: resolve the CHK/TRIG parser handlers in the CSVs.
8. Heap: re-dump RW regions in-game and repeat the TRIG/STR/UNIT signatures.

Details, candidate addresses and open questions: reports in
`analysis/investigations/` (A: P1–P7, B: §4–§5, C: §4, D: §8).
