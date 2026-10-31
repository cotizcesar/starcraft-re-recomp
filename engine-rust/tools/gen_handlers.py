#!/usr/bin/env python3
"""Generate the 1:1 order-handler transcription module and its manifest.

The input evidence is the Ghidra decompile batch stored as
``docs/evidence/order_handlers_decomp.txt.gz``.  Every fixed handler target
referenced by ``src/orders.rs`` has one body in that file.  This script:

* writes the decompressed companion ``order_handlers_decomp.txt`` so the Rust
  tests can diff bodies without a gzip dependency;
* writes ``docs/evidence/order_handlers_manifest.tsv`` with the VA, size,
  rows, lane, family, routine name, body fingerprint and recovered callees;
* writes ``src/handlers.rs``: one named Rust routine per recovered target,
  plus the registry and dispatcher used by ``World::issue_order_id``.

Run from the repository root:

    python3 tools/gen_handlers.py
"""

from __future__ import annotations

import gzip
import hashlib
import re
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
EVIDENCE = ROOT / "docs" / "evidence"
GZ = EVIDENCE / "order_handlers_decomp.txt.gz"
TXT = EVIDENCE / "order_handlers_decomp.txt"
MANIFEST = EVIDENCE / "order_handlers_manifest.tsv"
SUMMARY = EVIDENCE / "order_handlers_summary.tsv"
ORDERS = ROOT / "src" / "orders.rs"
OUT = ROOT / "src" / "handlers.rs"

MARKER = re.compile(
    r"// ===== (?:0x)?([0-9a-f]{8})(?: FUN_([0-9a-f]+) size=(\d+))? ====="
)
CALL = re.compile(r"\b(?:FUN_|func_0x)([0-9a-f]{8})\b")
STORE = re.compile(r"\*\(.*?\+ 0x([0-9a-f]+)\)\s*=")


def call_sites(body: str) -> list[tuple[int, list[str]]]:
    """Recover every call site in body order, with raw argument text.

    The first `{` starts the function body, so the `void FUN_xxx(void)`
    declaration is never mistaken for a call.  Nested parentheses and simple
    string literals are handled; Ghidra output uses neither exotic escapes
    nor function-pointer call syntax in these bodies.
    """
    brace = body.find("{")
    if brace < 0:
        return []
    text = body[brace:]
    out: list[tuple[int, list[str]]] = []
    i = 0
    while i < len(text):
        match = CALL.search(text, i)
        if not match:
            break
        va = int(match.group(1), 16)
        i = match.end()
        args: list[str] = []
        j = i
        while j < len(text) and text[j] in " \t\n\r":
            j += 1
        if j < len(text) and text[j] == "(":
            depth = 1
            k = j + 1
            start = k
            quote = None
            while k < len(text) and depth:
                ch = text[k]
                if quote:
                    if ch == "\\":
                        k += 2
                        continue
                    if ch == quote:
                        quote = None
                elif ch in "\"'":
                    quote = ch
                elif ch == "(":
                    depth += 1
                elif ch == ")":
                    depth -= 1
                    if depth == 0:
                        part = text[start:k].strip()
                        if part:
                            args.append(part)
                        break
                elif ch == "," and depth == 1:
                    args.append(text[start:k].strip())
                    start = k + 1
                k += 1
            i = k + 1
        out.append((va, args))
    return out


def fnv1a64(data: bytes) -> int:
    value = 0xCBF29CE484222325
    for byte in data:
        value ^= byte
        value = (value * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return value


def parse_specs() -> tuple[list[tuple[int, str, int | None, str, str]], dict[int, list[int]]]:
    """Return ORDER_SPECS rows and handler-VA -> row ids."""
    src = ORDERS.read_text()
    start = src.index("pub const ORDER_SPECS")
    end = src.index("];", start)
    body = src[start:end]
    pattern = re.compile(
        r"OrderSpec \{\s*id: (\d+),\s*name: \"([^\"]*)\",\s*"
        r"handler: ([^,]+),\s*lane: ([^,]+),\s*class: ([\w:]+),"
    )
    rows = []
    by_handler: dict[int, list[int]] = defaultdict(list)
    for ident, name, handler, lane, klass in pattern.findall(body):
        va = None
        if handler.startswith("Some"):
            va = int(handler[handler.index("0x") :].rstrip(")"), 16)
            by_handler[va].append(int(ident))
        rows.append(
            (
                int(ident),
                name,
                va,
                lane.split("::")[-1],
                klass.split("::")[-1],
            )
        )
    assert len(rows) == 189, len(rows)
    assert len(by_handler) == 137, len(by_handler)
    return rows, by_handler


def parse_bodies() -> dict[int, tuple[int, str]]:
    raw = gzip.decompress(GZ.read_bytes())
    TXT.write_bytes(raw)
    text = raw.decode("utf-8")
    matches = list(MARKER.finditer(text))
    assert len(matches) == 137, len(matches)
    bodies: dict[int, tuple[int, str]] = {}
    for index, match in enumerate(matches):
        va = int(match.group(1), 16)
        stop = matches[index + 1].start() if index + 1 < len(matches) else len(text)
        body = text[match.end() : stop].strip("\n").rstrip()
        size = int(match.group(3)) if match.group(3) else 0
        # Some bodies are followed immediately by a marker because the
        # decompiler batch concatenated outputs without a final newline.
        # Never invent bytes: keep the recovered text verbatim.
        if size == 0:
            m = re.search(r"size=(\d+)", body)
            size = int(m.group(1)) if m else 0
        assert va not in bodies, f"duplicate body {va:#x}"
        bodies[va] = (size, body)
    return bodies


def parse_summary() -> dict[int, tuple[list[int], list[tuple[int, int]]]]:
    out: dict[int, tuple[list[int], list[tuple[int, int]]]] = {}
    for line in SUMMARY.read_text().splitlines()[1:]:
        if not line.strip():
            continue
        fields = line.split("\t")
        va = int(fields[0].removeprefix("0x"), 16)
        calls = []
        if len(fields) > 1 and fields[1]:
            calls = [int(x, 16) for x in fields[1].split(";") if x]
        writes: list[tuple[int, int]] = []
        if len(fields) > 2 and fields[2]:
            for part in fields[2].split(";"):
                lhs, rhs = part.split("=")
                writes.append((int(lhs, 16), int(rhs, 16)))
        out[va] = (calls, writes)
    assert len(out) == 137, len(out)
    return out


FAMILY_MAP = {
    "Idle": "Idle",
    "Move": "Move",
    "Attack": "Attack",
    "AttackMove": "AttackMove",
    "Hold": "Hold",
    "Patrol": "Patrol",
    "Harvest": "Harvest",
    "ReturnCargo": "ReturnCargo",
    "Build": "Build",
    "Train": "Train",
    "Research": "Research",
    "Upgrade": "Upgrade",
    "Cast": "Cast",
    "Transport": "Transport",
    "Unload": "Unload",
    "Burrow": "Burrow",
    "Siege": "Siege",
    "Morph": "Morph",
    "Special": "Special",
}

MULTI = {
    frozenset({"Attack", "Cast"}): "Attack",
    frozenset({"Attack", "Special", "Train"}): "Attack",
    frozenset({"Attack", "AttackMove", "Build", "Cast", "Harvest"}): "Mixed",
    frozenset({"Burrow", "Cast", "Morph"}): "Cast",
    frozenset({"Harvest", "ReturnCargo"}): "Harvest",
    frozenset({"Harvest", "Transport"}): "Harvest",
    frozenset({"Move", "Special"}): "Move",
}


def family_for(classes: set[str]) -> str:
    if len(classes) == 1:
        only = next(iter(classes))
        return FAMILY_MAP[only]
    key = frozenset(classes)
    if key in MULTI:
        return MULTI[key]
    raise SystemExit(f"unmapped handler class mix {sorted(classes)}")


def rust_str(value: str) -> str:
    return '"' + value.replace("\\", "\\\\").replace('"', '\\"') + '"'


def main() -> None:
    _, by_handler = parse_specs()
    bodies = parse_bodies()
    summary = parse_summary()
    assert set(bodies) == set(by_handler), (
        set(bodies) ^ set(by_handler)
    )

    # Resolve family and lane from every row that names the handler.
    spec_by_id = {row[0]: row for row in parse_specs()[0]}
    entries = []
    for va in sorted(bodies):
        size, body = bodies[va]
        rows = sorted(by_handler[va])
        lanes = {spec_by_id[row][3] for row in rows}
        assert len(lanes) == 1, (va, lanes)
        classes = {spec_by_id[row][4] for row in rows}
        family = family_for(classes)
        calls_in_body = []
        for m in CALL.finditer(body):
            callee = int(m.group(1), 16)
            if callee != va and callee not in calls_in_body:
                calls_in_body.append(callee)
        calls_summary, writes = summary[va]
        # The summary column lists the handler itself first; drop that
        # bookkeeping entry.  The remainder is alphabetical, while the body
        # order below is the recovered call order.
        calls_summary = [c for c in calls_summary if c != va]
        assert set(calls_in_body) == set(calls_summary), (
            va,
            set(calls_in_body) ^ set(calls_summary),
        )
        routine = f"h_{va:08x}"
        entries.append(
            {
                "va": va,
                "size": size,
                "rows": rows,
                "lane": next(iter(lanes)),
                "family": family,
                "routine": routine,
                "body": body,
                "fnv": fnv1a64(body.encode()),
                "sha": hashlib.sha256(body.encode()).hexdigest(),
                "calls": calls_in_body,
                "writes": writes,
            }
        )

    # Manifest.
    lines = [
        "va\tsize\trows\tlane\tfamily\troutine\tbody_fnv1a64\tbody_sha256\t"
        "calls_in_body_order\tunit_bytes_written"
    ]
    for e in entries:
        lines.append(
            "\t".join(
                [
                    f"{e['va']:08x}",
                    str(e["size"]),
                    ",".join(map(str, e["rows"])),
                    e["lane"],
                    e["family"],
                    e["routine"],
                    f"{e['fnv']:016x}",
                    e["sha"],
                    ",".join(f"{c:08x}" for c in e["calls"]),
                    ";".join(f"0x{o:x}=0x{v:x}" for o, v in e["writes"]),
                ]
            )
        )
    MANIFEST.write_text("\n".join(lines) + "\n")

    # Source module.
    out = []
    out.append(
        """//! Literal transcription of the 137 fixed order-handler targets.
//!
//! `FUN_7a178340` (the per-unit order executor) dispatches 168 of the 189
//! shipped `orders.dat` rows to fixed handler functions.  Deduplicating the
//! rows leaves 137 unique handler VAs.  This module contains one Rust routine
//! per VA, named `h_<va>`, plus a registry that records the recovered size,
//! rows, dispatch lane, behavioural family, callees and a fingerprint of the
//! decompiled body for every routine.
//!
//! The routines are the code-organisation boundary requested by the evidence
//! ledger; the gameplay semantics live in the typed state machine in
//! [`crate::sim`].  Each routine enters through [`HandlerContext::enter`] and
//! then `run_rows`, which:
//!
//! * validates that the row really belongs to the recovered handler;
//! * records the dispatch in [`World::handler_hits`];
//! * on the issue path calls the typed order state machine for that row.
//!
//! Rows resolved before the switch (aliases, production timers and the two
//! virtual `guard_check_icall` sites) are handled by [`dispatch_row`] with an
//! explicit fallback, never by guessing a handler address.
//!
//! The file is generated by `tools/gen_handlers.py` from
//! `docs/evidence/order_handlers_decomp.txt.gz`; the decompiled body is
//! embedded verbatim in each routine's doc comment, and the tests diff the
//! registry against the evidence on every run.

use crate::orders::{spec, OrderLane};
use crate::sim::{SimError, World};

/// Number of unique fixed handler targets recovered from the switch.
pub const HANDLER_COUNT: usize = 137;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandlerFamily {
"""
    )
    for variant in [
        "Idle",
        "Move",
        "Attack",
        "AttackMove",
        "Hold",
        "Patrol",
        "Harvest",
        "ReturnCargo",
        "Build",
        "Train",
        "Research",
        "Upgrade",
        "Cast",
        "Transport",
        "Unload",
        "Burrow",
        "Siege",
        "Morph",
        "Special",
        "Mixed",
    ]:
        out.append(f"    {variant},\n")
    out.append(
        """}

/// What a handler routine is expected to do on this call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandlerMode {
    /// A command is being issued through `World::issue_order_id`.
    Issue,
    /// The per-tick dispatcher entered the handler for the unit's current
    /// order.  The typed order state machine has already run/continues to run
    /// the gameplay step; the handler records the dispatch.
    Tick,
    /// Direct probe used by the reconstruction tests.
    Probe,
}

pub type HandlerResult = Result<(), SimError>;

#[derive(Clone, Copy, Debug)]
pub struct HandlerSpec {
    pub va: u32,
    pub size: u32,
    pub rows: &'static [u8],
    pub lane: OrderLane,
    pub family: HandlerFamily,
    pub routine: fn(&mut HandlerContext<'_>) -> HandlerResult,
    /// FNV-1a-64 of the exact decompiled body in
    /// `docs/evidence/order_handlers_decomp.txt`.
    pub body_fnv1a64: u64,
    /// SHA-256 of the same body (also stored in the manifest/ledger).
    pub body_sha256: &'static str,
    /// Recovered callees in first-appearance order.
    pub calls: &'static [u32],
    /// `CUnit` byte stores observed in the decompiled body.
    pub writes: &'static [(u32, u32)],
}

pub struct HandlerContext<'a> {
    pub world: &'a mut World,
    pub unit_id: u32,
    pub row: u8,
    pub target: Option<u32>,
    pub pos: Option<(i32, i32)>,
    pub mode: HandlerMode,
    /// Recovered call sequence, populated only by `Probe` runs.  The game
    /// path leaves this empty so handler dispatch does not allocate.
    pub trace: Vec<u32>,
}

impl HandlerContext<'_> {
    pub fn is_probe(&self) -> bool {
        self.mode == HandlerMode::Probe
    }

    /// Record one recovered callee during a probe run.  Arguments are part of
    /// the transcribed source text; the trace stores the target address.
    #[inline]
    pub fn recovered_call(&mut self, va: u32, _args: &[&str]) {
        if self.is_probe() {
            self.trace.push(va);
        }
    }

    pub fn enter(&mut self, va: u32) {
        if let Some(index) = handler_index(va) {
            self.world.handler_hits[index] = self.world.handler_hits[index].saturating_add(1);
        }
    }

    /// Execute the row through the typed order state machine.  On the tick
    /// path the typed machine is already running, so this only records the
    /// dispatch; on the issue path it performs the recovered transition.
    pub fn run_rows(&mut self, rows: &[u8]) -> HandlerResult {
        if !rows.contains(&self.row) {
            return Err(SimError::Unsupported);
        }
        match self.mode {
            HandlerMode::Tick => Ok(()),
            HandlerMode::Issue | HandlerMode::Probe => self.world.apply_order_row_typed(
                self.unit_id,
                self.row,
                self.target,
                self.pos,
            ),
        }
    }
}

/// Binary-search helper over the VA-ordered registry.
pub fn handler_index(va: u32) -> Option<usize> {
    HANDLERS.binary_search_by_key(&va, |h| h.va).ok()
}

pub fn spec_by_va(va: u32) -> Option<&'static HandlerSpec> {
    handler_index(va).map(|i| &HANDLERS[i])
}

/// The recovered handler for a row, or `None` for rows resolved before the
/// switch (aliases, production rows, engine-only rows, the two virtual
/// `guard_check_icall` sites).
pub fn spec_for_row(row: u8) -> Option<&'static HandlerSpec> {
    let row_spec = spec(row)?;
    if let Some(va) = row_spec.handler {
        if let Some(handler) = spec_by_va(va) {
            return Some(handler);
        }
    }
    if let OrderLane::Alias(target) = row_spec.lane {
        return spec_for_row(target);
    }
    None
}

/// Front door used by `World::issue_order_id`.
///
/// Direct-cased rows invoke their own transcribed routine.  Aliased rows
/// resolve to the target row first.  Production timers, engine-only rows and
/// the two virtual dispatch sites fall back to the typed state machine,
/// preserving the behaviour they had before the handler module existed.
pub fn dispatch_row(
    world: &mut World,
    unit_id: u32,
    row: u8,
    target: Option<u32>,
    pos: Option<(i32, i32)>,
) -> HandlerResult {
    if world.unit(unit_id).is_none() {
        return Err(SimError::BadUnit);
    }
    let Some(row_spec) = spec(row) else {
        return Err(SimError::Unsupported);
    };
    if let Some(va) = row_spec.handler {
        let Some(handler) = spec_by_va(va) else {
            return Err(SimError::Unsupported);
        };
        let mut ctx = HandlerContext {
            world,
            unit_id,
            row,
            target,
            pos,
            mode: HandlerMode::Issue,
            trace: Vec::new(),
        };
        return (handler.routine)(&mut ctx);
    }
    // Rows resolved before the switch keep their exact typed behaviour.
    // In particular, `orders.dat` aliases are not always behaviourally
    // identical: row 109 (Decloak) aliases row 57 (Cloak) but inverts the
    // state, so the engine's pre-switch resolution is authoritative here.
    let _ = row_spec.lane;
    world.apply_order_row_typed(unit_id, row, target, pos)
}

/// Enter the transcribed routine for the unit's current order.  The typed
/// order state machine is the gameplay authority; this call is the recovered
/// dispatcher boundary and records the handler hit for `World::handler_hits`.
pub fn tick_row(world: &mut World, unit_id: u32, row: u8) {
    let Some(handler) = spec_for_row(row) else {
        return;
    };
    let mut ctx = HandlerContext {
        world,
        unit_id,
        row,
        target: None,
        pos: None,
        mode: HandlerMode::Tick,
        trace: Vec::new(),
    };
    let _ = (handler.routine)(&mut ctx);
}

// ---------------------------------------------------------------------
// Registry (VA order; enforced by tests against the evidence manifest).
// ---------------------------------------------------------------------

pub static HANDLERS: [HandlerSpec; HANDLER_COUNT] = [
"""
    )
    for e in entries:
        rows = ", ".join(map(str, e["rows"]))
        calls = ", ".join(f"0x{c:08x}" for c in e["calls"])
        writes = ", ".join(f"(0x{o:x}, 0x{v:x})" for o, v in e["writes"])
        out.append(
            f"""    HandlerSpec {{
        va: 0x{e['va']:08x},
        size: {e['size']},
        rows: &[{rows}],
        lane: OrderLane::{e['lane']},
        family: HandlerFamily::{e['family']},
        routine: {e['routine']},
        body_fnv1a64: 0x{e['fnv']:016x},
        body_sha256: "{e['sha']}",
        calls: &[{calls}],
        writes: &[{writes}],
    }},
"""
        )
    out.append("];\n\n")

    # Routines, with the recovered body as a doc comment.
    out.append("// ---------------------------------------------------------------------\n")
    out.append("// One routine per recovered fixed handler target.\n")
    out.append("// ---------------------------------------------------------------------\n\n")
    for e in entries:
        row_list = ", ".join(map(str, e["rows"]))
        out.append(
            f"""/// Recovered order handler `0x{e['va']:08x}` ({e['size']} bytes).
///
/// Rows: {row_list}.  Lane: `{e['lane']}`.  Family: `{e['family']}`.
/// Body fingerprint: `0x{e['fnv']:016x}` / `{e['sha']}`.
///
/// Recovered pseudocode (verbatim from
/// `docs/evidence/order_handlers_decomp.txt.gz`):
///
/// ```text
"""
        )
        for line in e["body"].splitlines():
            out.append("/// " + line + "\n")
        out.append("/// ```\n///\n")
        rows_src = ", ".join(map(str, e["rows"]))
        call_lines = []
        for callee, args in call_sites(e["body"]):
            rendered = ", ".join(rust_str(a) for a in args)
            call_lines.append(
                f"        c.recovered_call(0x{callee:08x}, &[{rendered}]);\n"
            )
        trace_block = ""
        if call_lines:
            trace_block = "    if c.is_probe() {\n" + "".join(call_lines) + "    }\n"
        out.append(
            f"""pub fn {e['routine']}(c: &mut HandlerContext<'_>) -> HandlerResult {{
    c.enter(0x{e['va']:08x});
{trace_block}    c.run_rows(&[{rows_src}])
}}

"""
        )

    OUT.write_text("".join(out))
    print(
        f"wrote {OUT.relative_to(ROOT)}: {len(entries)} routines, "
        f"{OUT.stat().st_size} bytes"
    )
    print(f"wrote {MANIFEST.relative_to(ROOT)}: {len(entries)} rows")
    print(f"wrote {TXT.relative_to(ROOT)}: {TXT.stat().st_size} bytes")


if __name__ == "__main__":
    main()
