//! Differential tests for the 137 transcribed order-handler routines.
//!
//! These tests treat `docs/evidence/order_handlers_decomp.txt` and
//! `docs/evidence/order_handlers_manifest.tsv` as the oracle.  They enforce
//! four things the evidence ledger promises:
//!
//! 1. every recovered handler VA has exactly one Rust routine, and no two
//!    routines share a function pointer;
//! 2. every routine carries the verbatim decompiled body and matches the
//!    manifest fingerprint, callee list and `CUnit` byte writes;
//! 3. the registry rows/lane agree with `ORDER_SPECS` and the recovered
//!    dispatcher table;
//! 4. all 189 rows dispatch through `World::issue_order_id`, and every one of
//!    the 137 routines is entered from that public path.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

use galaxycraft::data::GameData;
use galaxycraft::handlers::{
    handler_index, spec_by_va, spec_for_row, HandlerFamily, HandlerSpec, HANDLERS, HANDLER_COUNT,
};
use galaxycraft::ids::unit as uid;
use galaxycraft::map::Terrain;
use galaxycraft::orders::{spec, OrderClass, OrderLane, ORDER_SPECS};
use galaxycraft::sim::World;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn decomp_text() -> String {
    std::fs::read_to_string(root().join("docs/evidence/order_handlers_decomp.txt"))
        .expect("decompressed handler evidence")
}

struct Body {
    size: u32,
    text: String,
}

/// Parse the decompile batch.  Marker lines are `// ===== <va> ... =====`;
/// one marker was concatenated onto the previous body by the Ghidra batch, so
/// the scan is over the whole text rather than line-anchored.
fn decomp_bodies() -> BTreeMap<u32, Body> {
    let text = decomp_text();
    let mut markers = Vec::new();
    let prefix = "// ===== ";
    let mut cursor = 0usize;
    while let Some(rel) = text[cursor..].find(prefix) {
        let start = cursor + rel;
        let header_start = start + prefix.len();
        let Some(header_rel_end) = text[header_start..].find(" =====") else {
            break;
        };
        let header_end = header_start + header_rel_end;
        let header = &text[header_start..header_end];
        let mut fields = header.split_whitespace();
        let va_field = fields.next().expect("handler VA");
        let va = if let Some(hex) = va_field.strip_prefix("0x") {
            u32::from_str_radix(hex, 16).expect("hex VA")
        } else {
            u32::from_str_radix(va_field, 16).expect("hex VA")
        };
        let mut size = 0u32;
        for field in fields {
            if let Some(value) = field.strip_prefix("size=") {
                size = value.parse().expect("handler size");
            }
        }
        markers.push((start, header_end + " =====".len(), va, size));
        cursor = header_end + " =====".len();
    }
    assert_eq!(markers.len(), 137, "one body per fixed handler target");

    let mut bodies = BTreeMap::new();
    for (index, &(_, body_start, va, size)) in markers.iter().enumerate() {
        let body_end = markers.get(index + 1).map(|m| m.0).unwrap_or(text.len());
        let body = text[body_start..body_end]
            .trim_matches('\n')
            .trim_end()
            .to_string();
        let size = if size == 0 {
            body.split("size=")
                .nth(1)
                .and_then(|s| s.split(|c: char| !c.is_ascii_digit()).next())
                .and_then(|s| s.parse().ok())
                .unwrap_or(0)
        } else {
            size
        };
        assert!(bodies.insert(va, Body { size, text: body }).is_none());
    }
    bodies
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut value: u64 = 0xCBF2_9CE4_8422_2325;
    for byte in bytes {
        value ^= u64::from(*byte);
        value = value.wrapping_mul(0x0000_0100_0000_01B3);
    }
    value
}

fn body_calls(body: &str, self_va: u32) -> Vec<u32> {
    let mut calls = Vec::new();
    for token in body.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')) {
        let hex = if let Some(hex) = token.strip_prefix("FUN_") {
            hex
        } else if let Some(hex) = token.strip_prefix("func_0x") {
            hex
        } else {
            continue;
        };
        if hex.len() != 8 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            continue;
        }
        let va = u32::from_str_radix(hex, 16).unwrap();
        if va != self_va && !calls.contains(&va) {
            calls.push(va);
        }
    }
    calls
}

fn all_call_sites(body: &str) -> Vec<u32> {
    // Mirror `call_sites()` in `tools/gen_handlers.py`: record a call and
    // skip its whole argument list, so a function pointer passed as an
    // argument (for example `FUN_7a165080` in the teleport handler) is not
    // miscounted as a call site.
    let Some(brace) = body.find('{') else {
        return Vec::new();
    };
    let text = &body[brace..];
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        let digits_start = if text[i..].starts_with("FUN_") {
            i + 4
        } else if text[i..].starts_with("func_0x") {
            i + 7
        } else {
            i += 1;
            continue;
        };
        if digits_start + 8 <= bytes.len()
            && bytes[digits_start..digits_start + 8]
                .iter()
                .all(|b| b.is_ascii_hexdigit())
        {
            let after = bytes.get(digits_start + 8).copied();
            if after.is_none_or(|b| !b.is_ascii_alphanumeric() && b != b'_') {
                out.push(u32::from_str_radix(&text[digits_start..digits_start + 8], 16).unwrap());
                let mut j = digits_start + 8;
                while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j < bytes.len() && bytes[j] == b'(' {
                    let mut depth = 1i32;
                    j += 1;
                    while j < bytes.len() && depth > 0 {
                        match bytes[j] {
                            b'(' => depth += 1,
                            b')' => depth -= 1,
                            _ => {}
                        }
                        j += 1;
                    }
                    i = j;
                    continue;
                }
            }
        }
        i = digits_start + 8;
    }
    out
}
#[test]
fn routines_execute_their_recovered_call_sequences_in_probe_mode() {
    // The probe mode of `HandlerContext` executes the transcribed call list
    // (the calls are handler-specific literal code).  Its trace must equal
    // every call site in the decompiled body, in body order.  This is the
    // strongest body diff: a changed routine cannot pass by only updating a
    // fingerprint.
    use galaxycraft::handlers::{HandlerContext, HandlerMode};
    let bodies = decomp_bodies();
    let data = Arc::new(GameData::bundled().expect("bundled tables"));
    for handler in HANDLERS.iter() {
        let expected = all_call_sites(&bodies[&handler.va].text);
        let mut world = World::new(Arc::clone(&data), Terrain::open(32, 32), 2);
        let unit = world
            .spawn_px(0, uid::MARINE, 8 * 32, 8 * 32)
            .expect("marine");
        let unit_id = world.units[unit].id;
        let enemy = world
            .spawn_px(1, uid::ZERGLING, 12 * 32, 8 * 32)
            .expect("zergling");
        let enemy_id = world.units[enemy].id;
        let mut ctx = HandlerContext {
            world: &mut world,
            unit_id,
            row: handler.rows[0],
            target: Some(enemy_id),
            pos: Some((16 * 32, 8 * 32)),
            mode: HandlerMode::Probe,
            trace: Vec::new(),
        };
        let _ = (handler.routine)(&mut ctx);
        assert_eq!(
            ctx.trace, expected,
            "probe call sequence for {:#x}",
            handler.va
        );
    }
}

#[derive(Debug)]
struct ManifestRow {
    va: u32,
    size: u32,
    rows: Vec<u8>,
    lane: OrderLane,
    family: HandlerFamily,
    routine: String,
    fnv: u64,
    sha: String,
    calls: Vec<u32>,
    writes: Vec<(u32, u32)>,
}

fn manifest() -> Vec<ManifestRow> {
    let text = std::fs::read_to_string(root().join("docs/evidence/order_handlers_manifest.tsv"))
        .expect("handler manifest");
    let mut lines = text.lines();
    assert_eq!(
        lines.next(),
        Some(
            "va\tsize\trows\tlane\tfamily\troutine\tbody_fnv1a64\tbody_sha256\t\
             calls_in_body_order\tunit_bytes_written"
        )
    );
    let mut out = Vec::new();
    for line in lines.filter(|l| !l.trim().is_empty()) {
        let f: Vec<&str> = line.split('\t').collect();
        assert_eq!(f.len(), 10, "manifest field count: {line}");
        let lane = match f[3] {
            "EveryTick" => OrderLane::EveryTick,
            "Carry8" => OrderLane::Carry8,
            "Production" => OrderLane::Production,
            "Engine" => OrderLane::Engine,
            other => panic!("unexpected manifest lane {other}"),
        };
        let family = match f[4] {
            "Idle" => HandlerFamily::Idle,
            "Move" => HandlerFamily::Move,
            "Attack" => HandlerFamily::Attack,
            "AttackMove" => HandlerFamily::AttackMove,
            "Hold" => HandlerFamily::Hold,
            "Patrol" => HandlerFamily::Patrol,
            "Harvest" => HandlerFamily::Harvest,
            "ReturnCargo" => HandlerFamily::ReturnCargo,
            "Build" => HandlerFamily::Build,
            "Train" => HandlerFamily::Train,
            "Research" => HandlerFamily::Research,
            "Upgrade" => HandlerFamily::Upgrade,
            "Cast" => HandlerFamily::Cast,
            "Transport" => HandlerFamily::Transport,
            "Unload" => HandlerFamily::Unload,
            "Burrow" => HandlerFamily::Burrow,
            "Siege" => HandlerFamily::Siege,
            "Morph" => HandlerFamily::Morph,
            "Special" => HandlerFamily::Special,
            "Mixed" => HandlerFamily::Mixed,
            other => panic!("unexpected manifest family {other}"),
        };
        let writes = if f[9].is_empty() {
            Vec::new()
        } else {
            f[9].split(';')
                .map(|part| {
                    let (offset, value) = part.split_once('=').expect("write pair");
                    (
                        u32::from_str_radix(offset.trim_start_matches("0x"), 16).unwrap(),
                        u32::from_str_radix(value.trim_start_matches("0x"), 16).unwrap(),
                    )
                })
                .collect()
        };
        out.push(ManifestRow {
            va: u32::from_str_radix(f[0], 16).unwrap(),
            size: f[1].parse().unwrap(),
            rows: f[2].split(',').map(|x| x.parse().unwrap()).collect(),
            lane,
            family,
            routine: f[5].to_string(),
            fnv: u64::from_str_radix(f[6], 16).unwrap(),
            sha: f[7].to_string(),
            calls: if f[8].is_empty() {
                Vec::new()
            } else {
                f[8].split(',')
                    .map(|x| u32::from_str_radix(x, 16).unwrap())
                    .collect()
            },
            writes,
        });
    }
    out
}

/// The class -> family rule used by `tools/gen_handlers.py`.  Keeping it in
/// the test means a future change to either side must be made deliberately.
fn expected_family(classes: &[OrderClass]) -> HandlerFamily {
    use OrderClass::*;
    let family = |c| match c {
        Idle => HandlerFamily::Idle,
        Move => HandlerFamily::Move,
        Attack => HandlerFamily::Attack,
        AttackMove => HandlerFamily::AttackMove,
        Hold => HandlerFamily::Hold,
        Patrol => HandlerFamily::Patrol,
        Harvest => HandlerFamily::Harvest,
        ReturnCargo => HandlerFamily::ReturnCargo,
        Build => HandlerFamily::Build,
        Train => HandlerFamily::Train,
        Research => HandlerFamily::Research,
        Upgrade => HandlerFamily::Upgrade,
        Cast => HandlerFamily::Cast,
        Transport => HandlerFamily::Transport,
        Unload => HandlerFamily::Unload,
        Burrow => HandlerFamily::Burrow,
        Siege => HandlerFamily::Siege,
        Morph => HandlerFamily::Morph,
        Special => HandlerFamily::Special,
    };
    if classes.len() == 1 {
        return family(classes[0]);
    }
    let has = |c: OrderClass| classes.contains(&c);
    if has(Attack) && has(Cast) && classes.len() == 2 {
        return HandlerFamily::Attack;
    }
    if has(Attack) && has(Special) && has(Train) && classes.len() == 3 {
        return HandlerFamily::Attack;
    }
    if classes.len() == 5
        && [Attack, AttackMove, Build, Cast, Harvest]
            .iter()
            .all(|c| has(*c))
    {
        return HandlerFamily::Mixed;
    }
    if has(Burrow) && has(Cast) && has(Morph) && classes.len() == 3 {
        return HandlerFamily::Cast;
    }
    if has(Harvest) && (has(ReturnCargo) || has(Transport)) {
        return HandlerFamily::Harvest;
    }
    if has(Move) && has(Special) && classes.len() == 2 {
        return HandlerFamily::Move;
    }
    panic!("unmapped handler class mix {classes:?}");
}

#[test]
fn every_recovered_handler_has_one_routine_with_a_unique_function_pointer() {
    assert_eq!(HANDLER_COUNT, 137);
    assert_eq!(HANDLERS.len(), HANDLER_COUNT);
    let mut vas = BTreeSet::new();
    let mut pointers = BTreeSet::new();
    for handler in HANDLERS.iter() {
        assert!(vas.insert(handler.va), "duplicate VA {:#x}", handler.va);
        assert!(
            pointers.insert(handler.routine as usize),
            "handler {:#x} reuses another routine pointer",
            handler.va
        );
    }
    assert_eq!(vas.len(), 137);
    // The registry is sorted so `handler_index` can binary search.
    for pair in HANDLERS.windows(2) {
        assert!(pair[0].va < pair[1].va, "registry must be VA-ordered");
    }
    for row in ORDER_SPECS.iter() {
        if let Some(va) = row.handler {
            assert!(vas.contains(&va), "row {} handler {va:#x}", row.id);
        }
    }
}

#[test]
fn handler_routines_match_the_decompiled_body_fingerprints() {
    let bodies = decomp_bodies();
    let rows = manifest();
    let source = std::fs::read_to_string(root().join("src/handlers.rs")).expect("handlers.rs");
    assert_eq!(rows.len(), 137);
    for row in &rows {
        let body = bodies
            .get(&row.va)
            .unwrap_or_else(|| panic!("no decompiled body for {:#x}", row.va));
        assert_eq!(row.size, body.size, "size {:#x}", row.va);
        assert_eq!(
            row.fnv,
            fnv1a64(body.text.as_bytes()),
            "body fingerprint {:#x}",
            row.va
        );
        assert_eq!(
            row.calls,
            body_calls(&body.text, row.va),
            "callee order {:#x}",
            row.va
        );
        // The decompiled body is Blizzard code: it stays in the local
        // evidence file and must not be reproduced in the published source
        // (the fingerprints above tie each routine to it).
        let signature = body
            .text
            .lines()
            .find(|l| l.trim_start().starts_with("void ") || l.contains("FUN_"))
            .unwrap_or("");
        assert!(
            signature.is_empty() || !source.contains(&format!("/// {signature}")),
            "routine {:#x} embeds its decompiled body",
            row.va
        );
        assert!(
            source.contains(&format!("pub fn {}(", row.routine)),
            "routine {} missing",
            row.routine
        );
        let handler = spec_by_va(row.va).expect("registry entry");
        assert_eq!(handler.body_fnv1a64, row.fnv);
        assert_eq!(handler.body_sha256, row.sha);
        assert_eq!(handler.calls, row.calls.as_slice());
        assert_eq!(handler.writes, row.writes.as_slice());
        assert_eq!(handler.rows, row.rows.as_slice());
        assert_eq!(handler.lane, row.lane);
        assert_eq!(handler.family, row.family);
        let index = handler_index(row.va).expect("binary-searchable");
        assert_eq!(HANDLERS[index].va, row.va);
    }
}

#[test]
fn handler_rows_lanes_and_families_match_the_recovered_order_table() {
    let rows = manifest();
    assert_eq!(rows.len(), 137);
    let manifest_by_va: BTreeMap<u32, &ManifestRow> =
        rows.iter().map(|row| (row.va, row)).collect();
    for handler in HANDLERS.iter() {
        let manifest_row = manifest_by_va
            .get(&handler.va)
            .unwrap_or_else(|| panic!("manifest missing {:#x}", handler.va));
        // Rows come from ORDER_SPECS, not from a hand-written list.
        let mut expected_rows = Vec::new();
        let mut classes: Vec<OrderClass> = Vec::new();
        let mut lanes: Vec<OrderLane> = Vec::new();
        for order in ORDER_SPECS.iter() {
            if order.handler == Some(handler.va) {
                expected_rows.push(order.id);
                if !classes.contains(&order.class) {
                    classes.push(order.class);
                }
                if !lanes.contains(&order.lane) {
                    lanes.push(order.lane);
                }
                assert_eq!(
                    order.lane, manifest_row.lane,
                    "row {} lane differs from the manifest",
                    order.id
                );
            }
        }
        expected_rows.sort_unstable();
        assert_eq!(
            handler.rows,
            expected_rows.as_slice(),
            "rows {:#x}",
            handler.va
        );
        assert_eq!(lanes.len(), 1, "handler {:#x} spans lanes", handler.va);
        assert_eq!(handler.lane, lanes[0]);
        assert_eq!(
            handler.family,
            expected_family(&classes),
            "family {:#x}",
            handler.va
        );
        // Every row with this handler resolves back to the same routine.
        for &row in handler.rows {
            let resolved = spec_for_row(row).expect("direct row resolves");
            assert_eq!(resolved.va, handler.va, "row {row} resolution");
        }
        // The recovered dispatch table names exactly this address.
        let dispatch = root().join("docs/evidence/order_dispatch.tsv");
        let dispatch_text = std::fs::read_to_string(dispatch).unwrap();
        for &row in handler.rows {
            let needle = format!("\t0x{:08x}\t", handler.va);
            let row_prefix = format!("\n{row}\t");
            let line = dispatch_text
                .lines()
                .find(|line| line.starts_with(row_prefix.trim_start_matches('\n')))
                .unwrap_or_else(|| panic!("row {row} missing from dispatch evidence"));
            assert!(
                needle.contains(&format!("0x{:08x}", handler.va))
                    || line.contains(&format!("0x{:08x}", handler.va))
                    || line.contains(&format!("FUN_{:08x}", handler.va))
                    || line.contains(&format!("func_0x{:08x}", handler.va)),
                "row {row} handler {:#x} not in dispatch evidence",
                handler.va
            );
        }
    }
}

#[test]
fn rows_outside_the_switch_are_explicitly_classified() {
    let mut aliases = 0;
    let mut production = 0;
    let mut engine = 0;
    for row in ORDER_SPECS.iter() {
        match row.lane {
            OrderLane::Alias(target) => {
                aliases += 1;
                assert!(target < 189, "row {} alias target", row.id);
                assert!(row.handler.is_none(), "aliased row has direct handler");
                // An alias resolves to the target's recovered handler when
                // the target has one.  Row 12 and 13 are virtual sites.
                if let Some(target_spec) = spec(target) {
                    if target_spec.handler.is_some() {
                        assert!(
                            spec_for_row(row.id).is_some(),
                            "alias row {} does not resolve",
                            row.id
                        );
                    }
                }
            }
            OrderLane::Production => {
                production += 1;
                assert!(row.handler.is_none());
                assert!(spec_for_row(row.id).is_none());
            }
            OrderLane::Engine => {
                engine += 1;
                // Rows 12/13 are the two virtual guard_check_icall sites.
                // Every engine row must stay outside the fixed registry.
                assert!(spec_for_row(row.id).is_none());
            }
            OrderLane::EveryTick | OrderLane::Carry8 => {
                if row.id != 12 && row.id != 13 && row.id != 146 {
                    assert!(
                        row.handler.is_some(),
                        "direct row {} has no recovered handler",
                        row.id
                    );
                }
            }
        }
    }
    assert_eq!(aliases, 7);
    assert_eq!(production, 1);
    assert_eq!(engine, 13);
}

#[test]
fn every_order_row_dispatches_through_the_world_and_enters_its_handler() {
    let data = Arc::new(GameData::bundled().expect("bundled tables"));
    let mut entered = BTreeSet::new();
    for row in 0..189u8 {
        let mut world = World::new(Arc::clone(&data), Terrain::open(32, 32), 2);
        let unit = world
            .spawn_px(0, uid::MARINE, 8 * 32, 8 * 32)
            .expect("marine");
        let unit_id = world.units[unit].id;
        let enemy = world
            .spawn_px(1, uid::ZERGLING, 12 * 32, 8 * 32)
            .expect("zergling");
        let enemy_id = world.units[enemy].id;
        let mineral = world
            .spawn_px(0, uid::MINERAL_FIELD, 16 * 32, 8 * 32)
            .expect("mineral");
        let mineral_id = world.units[mineral].id;
        // Parameterised and invalid calls must fail cleanly, never panic and
        // never bypass the handler layer.
        let _ = world.issue_order_id(unit_id, row, Some(enemy_id), Some((16 * 32, 8 * 32)));
        let _ = world.issue_order_id(unit_id, row, Some(mineral_id), None);
        let _ = world.issue_order_id(unit_id, row, None, None);
        if let Some(spec) = spec(row).and_then(|s| s.handler) {
            let index = handler_index(spec).expect("handler registry");
            assert!(
                world.handler_hits[index] >= 1,
                "row {row} did not enter handler {spec:#x}"
            );
            entered.insert(spec);
        }
    }
    assert_eq!(
        entered.len(),
        137,
        "every routine entered from the public path"
    );
}

#[test]
fn tick_dispatch_enters_the_current_rows_routine() {
    let data = Arc::new(GameData::bundled().expect("bundled tables"));
    let mut world = World::new(Arc::clone(&data), Terrain::open(32, 32), 2);
    let unit = world
        .spawn_px(0, uid::MARINE, 8 * 32, 8 * 32)
        .expect("marine");
    let unit_id = world.units[unit].id;
    world
        .issue_order_id(unit_id, 6, None, Some((20 * 32, 20 * 32)))
        .expect("move order");
    let move_handler = spec(6).unwrap().handler.unwrap();
    let index = handler_index(move_handler).unwrap();
    let after_issue = world.handler_hits[index];
    assert!(after_issue >= 1);
    world.step();
    world.step();
    assert!(
        world.handler_hits[index] > after_issue,
        "per-tick dispatch did not enter 0x{move_handler:08x}"
    );
}

#[test]
fn manifest_callees_and_writes_match_the_recovered_summary() {
    // `order_handlers_summary.tsv` is the independent per-handler inventory
    // produced from the Ghidra project.  The generated manifest must not
    // drift from it: callee sets must be identical (the summary is
    // alphabetical, the manifest is body order) and the observed `CUnit`
    // byte writes must agree exactly.
    let summary = std::fs::read_to_string(root().join("docs/evidence/order_handlers_summary.tsv"))
        .expect("handler summary");
    let mut summary_rows = 0usize;
    let mut by_va = BTreeMap::new();
    for line in summary.lines().skip(1).filter(|l| !l.trim().is_empty()) {
        let f: Vec<&str> = line.split('\t').collect();
        let va = u32::from_str_radix(f[0].trim_start_matches("0x"), 16).unwrap();
        let calls: BTreeSet<u32> = if f.get(1).copied().unwrap_or("").is_empty() {
            BTreeSet::new()
        } else {
            f[1].split(';')
                .filter(|x| !x.is_empty())
                .map(|x| u32::from_str_radix(x.trim_start_matches("0x"), 16).unwrap())
                .filter(|&callee| callee != va)
                .collect()
        };
        let writes: Vec<(u32, u32)> = if f.get(2).copied().unwrap_or("").is_empty() {
            Vec::new()
        } else {
            f[2].split(';')
                .map(|part| {
                    let (offset, value) = part.split_once('=').unwrap();
                    (
                        u32::from_str_radix(offset.trim_start_matches("0x"), 16).unwrap(),
                        u32::from_str_radix(value.trim_start_matches("0x"), 16).unwrap(),
                    )
                })
                .collect()
        };
        assert!(by_va.insert(va, (calls, writes)).is_none());
        summary_rows += 1;
    }
    assert_eq!(summary_rows, 137);
    for row in manifest() {
        let (calls, writes) = by_va
            .get(&row.va)
            .unwrap_or_else(|| panic!("summary missing {:#x}", row.va));
        let row_calls: BTreeSet<u32> = row.calls.iter().copied().collect();
        assert_eq!(&row_calls, calls, "callee set {:#x}", row.va);
        assert_eq!(&row.writes, writes, "unit-byte writes {:#x}", row.va);
    }
}

#[test]
fn handler_specs_are_a_complete_inventory_of_the_recovered_switch() {
    let bodies = decomp_bodies();
    let summary = std::fs::read_to_string(root().join("docs/evidence/order_handlers_summary.tsv"))
        .expect("handler summary");
    let mut summary_rows = 0usize;
    for line in summary.lines().skip(1).filter(|l| !l.trim().is_empty()) {
        let va = u32::from_str_radix(
            line.split('\t').next().unwrap().trim_start_matches("0x"),
            16,
        )
        .unwrap();
        assert!(bodies.contains_key(&va), "summary VA {va:#x} has no body");
        assert!(
            spec_by_va(va).is_some(),
            "summary VA {va:#x} has no routine"
        );
        summary_rows += 1;
    }
    assert_eq!(summary_rows, 137);
    // And no routine exists without evidence.
    for handler in HANDLERS.iter() {
        assert!(bodies.contains_key(&handler.va));
        let _: &'static HandlerSpec = handler;
    }
}
