//! Print and validate the recovered `orders.dat` dispatch table.
//!
//! ```text
//! sc_orders            # summary
//! sc_orders --full     # all 189 recovered rows
//! ```

use galaxycraft::handlers::{HANDLERS, spec_by_va};
use galaxycraft::orders::{ORDER_SPECS, OrderLane, lane, target_order};

fn main() {
    let full = std::env::args().any(|a| a == "--full");
    let mut every = 0;
    let mut carry = 0;
    let mut alias = 0;
    let mut production = 0;
    let mut engine = 0;
    let mut handlers = 0;
    let mut target_remaps = 0;
    for spec in &ORDER_SPECS {
        if spec.handler.is_some() {
            handlers += 1;
        }
        if target_order(spec.id) != spec.id {
            target_remaps += 1;
        }
        match spec.lane {
            OrderLane::EveryTick => every += 1,
            OrderLane::Carry8 => carry += 1,
            OrderLane::Alias(_) => alias += 1,
            OrderLane::Production => production += 1,
            OrderLane::Engine => engine += 1,
        }
        if full {
            println!(
                "{:3}  {:<32} {:<10} {:<12} {}",
                spec.id,
                spec.name,
                format!("{:?}", spec.class),
                format!("{:?}", spec.lane),
                spec.handler
                    .map(|h| format!("0x{h:08x}"))
                    .unwrap_or_else(|| "-".to_string())
            );
        }
    }
    println!(
        "orders: {} rows, {} direct handlers, {} every-tick, {} carry-8, {} aliases, {} production, {} engine; {} target-order remaps",
        ORDER_SPECS.len(),
        handlers,
        every,
        carry,
        alias,
        production,
        engine,
        target_remaps
    );
    println!(
        "transcribed handlers: {} routines, {} decompiled bodies",
        HANDLERS.len(),
        HANDLERS
            .iter()
            .filter(|h| h.body_sha256.len() == 64)
            .count()
    );
    // A direct-lane row must name its handler; a non-direct row must be
    // explicitly marked as alias/production/engine.
    let mut ok = true;
    for spec in &ORDER_SPECS {
        // Rows 12/13 dispatch through the recovered virtual
        // `guard_check_icall`; row 146 is the Hatchery burrow morph.
        if matches!(spec.lane, OrderLane::EveryTick | OrderLane::Carry8)
            && spec.handler.is_none()
            && !matches!(spec.id, 12 | 13 | 146)
        {
            eprintln!("row {} ({}) has no recovered handler", spec.id, spec.name);
            ok = false;
        }
        if let Some(va) = spec.handler
            && spec_by_va(va).is_none()
        {
            eprintln!("row {} handler {va:#x} has no transcribed routine", spec.id);
            ok = false;
        }
        if matches!(spec.lane, OrderLane::Alias(t) if t >= 189) {
            eprintln!("row {} aliases out-of-range row {}", spec.id, spec.name);
            ok = false;
        }
        let _ = lane(spec.id);
    }
    if !ok {
        std::process::exit(1);
    }
}
