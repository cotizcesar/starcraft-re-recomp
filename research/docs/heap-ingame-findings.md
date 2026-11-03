# In-game heap — verified findings (2026-10-28)

Capture: `work/heap_ingame_20261028_1237/` (483 RW regions, 897 MB).
Process: `x86_64\StarCraft.exe -launch -uid s1`, PID 80639, mission
**Terran 8 "The Big Push"** (Episode I) in progress.
Method: `tools/heap_capture.py` (pread of `/proc/<pid>/mem`, no debugger,
no writes to the process).

## 1. The map IS loaded in the heap (FACT)

| Text | Region | Offset |
|---|---|---|
| `Eliminate the Confederate forces.` | `0xad22000` (126 MB) | `0x432fbf1` |
| `Duke must survive.` | `0xad22000` | +0x2e |
| `THE BIG PUSH` | `0xa942000` | — |

The briefing objectives live in the module's `.data`, and the map's STR
block was parsed into the heap. Before (image-only dumps) this gave 0 hits: **the
RW re-dump was the right experiment**.

## 2. RULED OUT: raw TRIG-2400 and DAT columns (FACT)

0 valid candidates across 483 regions. The positives from the lax scoring were
BSS zeros and garbage (verified by hand: conditions and actions all 0).

## 3. MAIN FINDING: pointer tables built at runtime

Three tables of pointers into `.text` that are **ZERO in the on-disk image**
and **populated in memory**:

| Table (module +) | Entries | Disk | Memory |
|---|---|---|---|
| `0x11b9b78` | 97 | `0x0` | `0x6ffffb343eb0` |
| `0x118f970` | 84 (57 deltas < 0x400) | `0x0` | `0x6ffffb343eb0` |
| `0x11afef0` | 63 | `0x0` | `0x6ffffb7e9180` |

The first two start with the **same** pointer (`0x6ffffb343eb0`),
consistent with a shared default handler.

**Why this matters:** the static sweep of 13 jump tables and of
all references in `.rdata`/`.data` could not find them — in the
file the table does not exist, only the reserved space. This closes the
question of why the dispatcher "does not show up" statically.

## 4. CHK section dispatch table in 64-bit (FACT, new)

`C6` at `f=0xae1940`, 64-bit layout `name(4) | ptr64 | flags(4)`, 30
entries. Recovered handlers (absolute VA, base `0x6ffffade0000`):

```
ERA  0x6ffffb138520   OWNR 0x6ffffb138440   SIDE 0x6ffffb138360
STR  0x6ffffb1387f0   SPRP 0x6ffffb139660   FORC 0x6ffffb139690
VCOD 0x6ffffb1396d0   TRIG 0x6ffffb1388d0   CRGB 0x6ffffb138600
MBRF 0x6ffffb138c60   MTXM 0x6ffffb139410   THG2 0x6ffffb1386a0
UNIT 0x6ffffb138d50   MASK 0x6ffffb139590   UNIS 0x6ffffb1399d0
UPGS 0x6ffffb139cc0   TECS 0x6ffffb13a200   PUNI 0x6ffffb13a5b0
UPGR 0x6ffffb13a810   PTEC 0x6ffffb13aae0
(+ optional UNIx/UPGx/TECx/PUPx/PTEx)
```

## 5. FALSIFIED: the section handlers are not in runtime tables

Sweep of the 20 handlers across the 483 regions: **1 hit** (TRIG itself,
in its static C6 table). Conclusion: section dispatch is the static
`.rdata` C6 table, not a runtime table.

## 6. Differential experiment with pause (FACT, 2026-10-28)

Goal: isolate what is written when issuing an order, with the simulation
frozen.

| Step | Capture | Notes |
|---|---|---|
| A | `work/heap_pauseA` | paused (Esc) |
| A2 | `work/heap_pauseA2` | paused, no action → **noise floor** |
| B | `work/heap_pauseB` | paused after 1 move order |

Measured results (`tools/heap_diff.py`):

| Comparison | Bytes changed | New pointers into `.text` |
|---|---:|---:|
| A → A2 (noise) | 11 462 468 | **13** |
| A → B (1 order) | 16 019 944 | **69** |

**Reading:** the differential works (5× over noise with a single order),
but the heap remains very dynamic even when paused (11 MB): there is active
render/streaming buffering. The 69 new pointers are spread across regions
`0x59exxxx`/`0x5aexxxx` (pairs 0x100000 apart with **identical
values**, consistent with double buffering, not with unit objects).

**Verified negative:** the three runtime tables of §3 **do not change** when
issuing an order (0 bytes in T97/T84/T63/T32). They are initialized at load
(class vtables), they are not the order dispatch.

**Conclusion:** the method is validated but the experiment needs less
noise (isolate the process from rendering, or instrument with Frida instead of
heap diffing) to locate the order handler. Current cost/benefit
low: the reconstruction does not depend on it (all 189 orders are already
implemented and tested in `engine-rust`).

Scripts: `tools/heap_capture.py`, `tools/heap_diff.py`.
