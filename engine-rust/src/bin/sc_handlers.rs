//! Print and validate the 137 transcribed order-handler routines.
//!
//! ```text
//! sc_handlers          # summary
//! sc_handlers --full   # one line per recovered handler target
//! ```
//!
//! The binary is the human-readable companion to `tests/handlers.rs`.  It
//! fails when a direct-cased row has no routine, when a routine has no
//! decompiled body fingerprint, or when the registry and `ORDER_SPECS`
//! disagree about rows or lanes.

use std::collections::BTreeSet;

use galaxycraft::handlers::{HANDLERS, HandlerFamily, spec_by_va};
use galaxycraft::orders::{ORDER_SPECS, OrderLane};

fn main() {
    let full = std::env::args().any(|a| a == "--full");
    let mut rows_with_handler = 0usize;
    let mut every = 0usize;
    let mut carry = 0usize;
    let mut families = BTreeSet::new();
    let mut assigned_rows = 0usize;
    for handler in HANDLERS.iter() {
        match handler.lane {
            OrderLane::EveryTick => every += 1,
            OrderLane::Carry8 => carry += 1,
            _ => {
                eprintln!(
                    "handler {:#x} has non-direct lane {:?}",
                    handler.va, handler.lane
                );
                std::process::exit(1);
            }
        }
        families.insert(format!("{:?}", handler.family));
        for &row in handler.rows {
            assigned_rows += 1;
            let Some(spec) = galaxycraft::orders::spec(row) else {
                eprintln!("handler {:#x} names out-of-range row {row}", handler.va);
                std::process::exit(1);
            };
            if spec.handler != Some(handler.va) {
                eprintln!(
                    "row {row} names {:?}, handler {:#x} claims it",
                    spec.handler, handler.va
                );
                std::process::exit(1);
            }
            if spec.lane != handler.lane {
                eprintln!("row {row} lane differs from handler {:#x}", handler.va);
                std::process::exit(1);
            }
        }
        if handler.body_sha256.len() != 64 {
            eprintln!("handler {:#x} has incomplete evidence", handler.va);
            std::process::exit(1);
        }
        if full {
            println!(
                "{:08x}  size={:<4}  {:<9}  {:<12}  rows={:<28}  fnv=0x{:016x}  {}",
                handler.va,
                handler.size,
                format!("{:?}", handler.lane),
                format!("{:?}", handler.family),
                format!("{:?}", handler.rows),
                handler.body_fnv1a64,
                handler.rows.len()
            );
        }
    }
    for spec in ORDER_SPECS.iter() {
        if let Some(handler) = spec.handler {
            rows_with_handler += 1;
            if spec_by_va(handler).is_none() {
                eprintln!("row {} has no transcribed routine", spec.id);
                std::process::exit(1);
            }
        }
    }
    println!(
        "handlers: {} routines, {} direct-cased rows ({} assigned), {} every-tick, {} carry-8, {} families, {} decompiled bodies",
        HANDLERS.len(),
        rows_with_handler,
        assigned_rows,
        every,
        carry,
        families.len(),
        HANDLERS
            .iter()
            .filter(|h| h.body_sha256.len() == 64)
            .count()
    );
    println!(
        "families: {}",
        families.into_iter().collect::<Vec<_>>().join(", ")
    );
    let _ = HandlerFamily::Mixed;
}
