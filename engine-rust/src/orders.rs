//! Explicit per-row `orders.dat` dispatch specification.
//!
//! The retail order executor is the switch at runtime `0x7a178340` (see
//! `docs/evidence/order_dispatch_vm.txt`).  It has three dispatch layers:
//!
//! * a four-case fast switch (`case 0/0x30/0x81/0xae`) that returns after
//!   running the handler every logical tick;
//! * a 36-case switch executed every tick (attack, guard, worker and cast
//!   rows);
//! * a 128-case switch gated by the per-unit 8-tick carry counter at
//!   `CUnit+0x85`, which is reset to 8 after each pass.
//!
//! Every one of the 189 shipped rows is listed here with its recovered
//! handler address (when the switch has a direct case) and its lane.  Rows
//! that the engine resolves through another row (for example Harvest 78 is
//! resolved to the mineral/gas worker states 84/85) are marked `Alias`.
//! Rows that never reach the order dispatcher (production timers) or belong
//! to the campaign/hero machine are marked explicitly too, so no row falls
//! through to a guessed behaviour.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OrderClass {
    Idle,
    Move,
    Attack,
    AttackMove,
    Hold,
    Patrol,
    Harvest,
    ReturnCargo,
    Build,
    Train,
    Research,
    Upgrade,
    Cast,
    Transport,
    Unload,
    Burrow,
    Siege,
    Morph,
    Special,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrderLane {
    /// Handler runs on every logical tick.
    EveryTick,
    /// Handler runs when the 8-tick order-carry counter reaches zero.
    Carry8,
    /// The row is resolved to another row before dispatch.
    Alias(u8),
    /// Production timer row, handled by the build/research/upgrade queues.
    Production,
    /// Engine-internal row with no direct case in the recovered switch.
    Engine,
}

#[derive(Clone, Copy, Debug)]
pub struct OrderSpec {
    pub id: u8,
    pub name: &'static str,
    /// Runtime VA of the direct switch target, when the row has one.
    pub handler: Option<u32>,
    pub lane: OrderLane,
    pub class: OrderClass,
}

/// One entry for every row in the shipped `orders.dat` (189 rows).
pub const ORDER_SPECS: [OrderSpec; 189] = [
    OrderSpec {
        id: 0,
        name: "Fizzle",
        handler: Some(0x7a13f3b0),
        lane: OrderLane::EveryTick,
        class: OrderClass::Idle,
    },
    OrderSpec {
        id: 1,
        name: "Guard",
        handler: Some(0x7a1547e0),
        lane: OrderLane::Carry8,
        class: OrderClass::Hold,
    },
    OrderSpec {
        id: 2,
        name: "Player Guard",
        handler: Some(0x7a13f880),
        lane: OrderLane::Carry8,
        class: OrderClass::Hold,
    },
    OrderSpec {
        id: 3,
        name: "Turret Guard",
        handler: Some(0x7a140080),
        lane: OrderLane::Carry8,
        class: OrderClass::Hold,
    },
    OrderSpec {
        id: 4,
        name: "Bunker Guard",
        handler: Some(0x7a140520),
        lane: OrderLane::EveryTick,
        class: OrderClass::Hold,
    },
    OrderSpec {
        id: 5,
        name: "Ignore",
        handler: None,
        lane: OrderLane::Engine,
        class: OrderClass::Idle,
    },
    OrderSpec {
        id: 6,
        name: "Move (row label: Carrier Ignore)",
        handler: Some(0x7a154430),
        lane: OrderLane::Carry8,
        class: OrderClass::Move,
    },
    OrderSpec {
        id: 7,
        name: "Attack",
        handler: Some(0x7a131930),
        lane: OrderLane::Carry8,
        class: OrderClass::Attack,
    },
    OrderSpec {
        id: 8,
        name: "Attack Unit",
        handler: None,
        lane: OrderLane::Alias(9),
        class: OrderClass::Attack,
    },
    OrderSpec {
        id: 9,
        name: "Attack Unit",
        handler: Some(0x7a1537a0),
        lane: OrderLane::Carry8,
        class: OrderClass::Attack,
    },
    OrderSpec {
        id: 10,
        name: "Attack Fixed Range",
        handler: Some(0x7a13f350),
        lane: OrderLane::Carry8,
        class: OrderClass::Attack,
    },
    OrderSpec {
        id: 11,
        name: "Move Attack Unit",
        handler: Some(0x7a13f1b0),
        lane: OrderLane::Carry8,
        class: OrderClass::Attack,
    },
    OrderSpec {
        id: 12,
        name: "Hover",
        handler: None,
        lane: OrderLane::Carry8,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 13,
        name: "Attack Move",
        handler: None,
        lane: OrderLane::Carry8,
        class: OrderClass::AttackMove,
    },
    OrderSpec {
        id: 14,
        name: "Atk Move EP",
        handler: Some(0x7a13f250),
        lane: OrderLane::Carry8,
        class: OrderClass::AttackMove,
    },
    OrderSpec {
        id: 15,
        name: "Build",
        handler: Some(0x7a17cd00),
        lane: OrderLane::Carry8,
        class: OrderClass::Build,
    },
    OrderSpec {
        id: 16,
        name: "Drone Start Build",
        handler: None,
        lane: OrderLane::Alias(23),
        class: OrderClass::Build,
    },
    OrderSpec {
        id: 17,
        name: "Siege Mode",
        handler: Some(0x7a153260),
        lane: OrderLane::Carry8,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 18,
        name: "Vulture Mine",
        handler: Some(0x7a1402e0),
        lane: OrderLane::Carry8,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 19,
        name: "Vulture Mine",
        handler: Some(0x7a140330),
        lane: OrderLane::Carry8,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 20,
        name: "Carrier Attack",
        handler: Some(0x7a1742c0),
        lane: OrderLane::Carry8,
        class: OrderClass::Attack,
    },
    OrderSpec {
        id: 21,
        name: "Turret Attack",
        handler: Some(0x7a1546a0),
        lane: OrderLane::Carry8,
        class: OrderClass::Attack,
    },
    OrderSpec {
        id: 22,
        name: "Nothing",
        handler: Some(0x7a140380),
        lane: OrderLane::EveryTick,
        class: OrderClass::Idle,
    },
    OrderSpec {
        id: 23,
        name: "Drone Start Build",
        handler: Some(0x7a158090),
        lane: OrderLane::Carry8,
        class: OrderClass::Build,
    },
    OrderSpec {
        id: 24,
        name: "Drone Start Build",
        handler: None,
        lane: OrderLane::Alias(23),
        class: OrderClass::Build,
    },
    OrderSpec {
        id: 25,
        name: "Drone Build",
        handler: Some(0x7a17b850),
        lane: OrderLane::Carry8,
        class: OrderClass::Build,
    },
    OrderSpec {
        id: 26,
        name: "Drone Attack Unit",
        handler: Some(0x7a17b5f0),
        lane: OrderLane::EveryTick,
        class: OrderClass::Attack,
    },
    OrderSpec {
        id: 27,
        name: "Build",
        handler: Some(0x7a17da30),
        lane: OrderLane::Carry8,
        class: OrderClass::Build,
    },
    OrderSpec {
        id: 28,
        name: "Build",
        handler: Some(0x7a1537a0),
        lane: OrderLane::Carry8,
        class: OrderClass::Build,
    },
    OrderSpec {
        id: 29,
        name: "Build",
        handler: Some(0x7a17db50),
        lane: OrderLane::Carry8,
        class: OrderClass::Build,
    },
    OrderSpec {
        id: 30,
        name: "Build Protoss",
        handler: Some(0x7a12ce00),
        lane: OrderLane::EveryTick,
        class: OrderClass::Build,
    },
    OrderSpec {
        id: 31,
        name: "Build Protoss",
        handler: Some(0x7a15a440),
        lane: OrderLane::EveryTick,
        class: OrderClass::Build,
    },
    OrderSpec {
        id: 32,
        name: "Build Protoss",
        handler: Some(0x7a15aa20),
        lane: OrderLane::Carry8,
        class: OrderClass::Build,
    },
    OrderSpec {
        id: 33,
        name: "Build Protoss",
        handler: Some(0x7a12d270),
        lane: OrderLane::EveryTick,
        class: OrderClass::Build,
    },
    OrderSpec {
        id: 34,
        name: "Place Add-On",
        handler: Some(0x7a12d7e0),
        lane: OrderLane::EveryTick,
        class: OrderClass::Build,
    },
    OrderSpec {
        id: 35,
        name: "Place Add-On",
        handler: Some(0x7a1537a0),
        lane: OrderLane::Carry8,
        class: OrderClass::Build,
    },
    OrderSpec {
        id: 36,
        name: "Build Add-On",
        handler: Some(0x7a171ef0),
        lane: OrderLane::Carry8,
        class: OrderClass::Build,
    },
    OrderSpec {
        id: 37,
        name: "Train",
        handler: None,
        lane: OrderLane::Production,
        class: OrderClass::Train,
    },
    OrderSpec {
        id: 38,
        name: "Zerg Birth",
        handler: None,
        lane: OrderLane::Engine,
        class: OrderClass::Morph,
    },
    OrderSpec {
        id: 39,
        name: "CTF COP Init",
        handler: None,
        lane: OrderLane::Engine,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 40,
        name: "CTF COP Init",
        handler: None,
        lane: OrderLane::Engine,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 41,
        name: "Morph",
        handler: Some(0x7a17bbb0),
        lane: OrderLane::EveryTick,
        class: OrderClass::Morph,
    },
    OrderSpec {
        id: 42,
        name: "Zerg Building Morph",
        handler: Some(0x7a17b880),
        lane: OrderLane::EveryTick,
        class: OrderClass::Morph,
    },
    OrderSpec {
        id: 43,
        name: "Zerg Building Morph",
        handler: Some(0x7a17bd20),
        lane: OrderLane::Carry8,
        class: OrderClass::Morph,
    },
    OrderSpec {
        id: 44,
        name: "Zerg Build Self",
        handler: Some(0x7a12d160),
        lane: OrderLane::EveryTick,
        class: OrderClass::Build,
    },
    OrderSpec {
        id: 45,
        name: "Enter Nydus Canal",
        handler: Some(0x7a17b2b0),
        lane: OrderLane::EveryTick,
        class: OrderClass::Transport,
    },
    OrderSpec {
        id: 46,
        name: "Build Protoss",
        handler: Some(0x7a17ba80),
        lane: OrderLane::Carry8,
        class: OrderClass::Build,
    },
    OrderSpec {
        id: 47,
        name: "Protoss Build Self",
        handler: Some(0x7a17d6d0),
        lane: OrderLane::Carry8,
        class: OrderClass::Build,
    },
    OrderSpec {
        id: 48,
        name: "Zerg Build Self",
        handler: Some(0x7a15a6d0),
        lane: OrderLane::EveryTick,
        class: OrderClass::Build,
    },
    OrderSpec {
        id: 49,
        name: "Carrier",
        handler: Some(0x7a154060),
        lane: OrderLane::Carry8,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 50,
        name: "Carrier Fight",
        handler: Some(0x7a130f70),
        lane: OrderLane::Carry8,
        class: OrderClass::Attack,
    },
    OrderSpec {
        id: 51,
        name: "Carrier Stop",
        handler: Some(0x7a154430),
        lane: OrderLane::Carry8,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 52,
        name: "Reaver Stop",
        handler: Some(0x7a131250),
        lane: OrderLane::Carry8,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 53,
        name: "Carrier Attack Move",
        handler: Some(0x7a131000),
        lane: OrderLane::Carry8,
        class: OrderClass::AttackMove,
    },
    OrderSpec {
        id: 54,
        name: "Carrier Attack Move",
        handler: Some(0x7a1537a0),
        lane: OrderLane::Carry8,
        class: OrderClass::AttackMove,
    },
    OrderSpec {
        id: 55,
        name: "Carrier Stop",
        handler: Some(0x7a1310a0),
        lane: OrderLane::Carry8,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 56,
        name: "Reaver",
        handler: Some(0x7a130f70),
        lane: OrderLane::Carry8,
        class: OrderClass::Attack,
    },
    OrderSpec {
        id: 57,
        name: "Cloak",
        handler: Some(0x7a130f70),
        lane: OrderLane::Carry8,
        class: OrderClass::Cast,
    },
    OrderSpec {
        id: 58,
        name: "Reaver Attack",
        handler: Some(0x7a1317a0),
        lane: OrderLane::Carry8,
        class: OrderClass::Attack,
    },
    OrderSpec {
        id: 59,
        name: "Reaver Fight",
        handler: Some(0x7a131890),
        lane: OrderLane::Carry8,
        class: OrderClass::Attack,
    },
    OrderSpec {
        id: 60,
        name: "Reaver Fight",
        handler: Some(0x7a1537a0),
        lane: OrderLane::Carry8,
        class: OrderClass::Attack,
    },
    OrderSpec {
        id: 61,
        name: "Reaver Hold",
        handler: Some(0x7a1317a0),
        lane: OrderLane::Carry8,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 62,
        name: "Train Fighter",
        handler: Some(0x7a1317a0),
        lane: OrderLane::Carry8,
        class: OrderClass::Train,
    },
    OrderSpec {
        id: 63,
        name: "Strafe Unit",
        handler: None,
        lane: OrderLane::Engine,
        class: OrderClass::Attack,
    },
    OrderSpec {
        id: 64,
        name: "Scarab",
        handler: Some(0x7a131340),
        lane: OrderLane::Carry8,
        class: OrderClass::Attack,
    },
    OrderSpec {
        id: 65,
        name: "Scarab",
        handler: Some(0x7a131970),
        lane: OrderLane::EveryTick,
        class: OrderClass::Attack,
    },
    OrderSpec {
        id: 66,
        name: "Shield Battery",
        handler: Some(0x7a162210),
        lane: OrderLane::EveryTick,
        class: OrderClass::Cast,
    },
    OrderSpec {
        id: 67,
        name: "Shield Battery",
        handler: Some(0x7a162140),
        lane: OrderLane::Carry8,
        class: OrderClass::Cast,
    },
    OrderSpec {
        id: 68,
        name: "Rally Point",
        handler: None,
        lane: OrderLane::Engine,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 69,
        name: "Drone Land",
        handler: Some(0x7a131290),
        lane: OrderLane::Carry8,
        class: OrderClass::Morph,
    },
    OrderSpec {
        id: 70,
        name: "Building Land",
        handler: Some(0x7a17d3b0),
        lane: OrderLane::Carry8,
        class: OrderClass::Morph,
    },
    OrderSpec {
        id: 71,
        name: "Building Lift Off",
        handler: Some(0x7a172e60),
        lane: OrderLane::EveryTick,
        class: OrderClass::Morph,
    },
    OrderSpec {
        id: 72,
        name: "Drone Lift Off",
        handler: Some(0x7a173230),
        lane: OrderLane::EveryTick,
        class: OrderClass::Morph,
    },
    OrderSpec {
        id: 73,
        name: "Lift Off",
        handler: None,
        lane: OrderLane::Engine,
        class: OrderClass::Morph,
    },
    OrderSpec {
        id: 74,
        name: "Reasearch Tech",
        handler: Some(0x7a1733b0),
        lane: OrderLane::Carry8,
        class: OrderClass::Research,
    },
    OrderSpec {
        id: 75,
        name: "Upgrade",
        handler: Some(0x7a17a100),
        lane: OrderLane::EveryTick,
        class: OrderClass::Upgrade,
    },
    OrderSpec {
        id: 76,
        name: "Larva",
        handler: Some(0x7a17a1c0),
        lane: OrderLane::EveryTick,
        class: OrderClass::Morph,
    },
    OrderSpec {
        id: 77,
        name: "Spawning Larva",
        handler: Some(0x7a17d7c0),
        lane: OrderLane::Carry8,
        class: OrderClass::Morph,
    },
    OrderSpec {
        id: 78,
        name: "Harvest",
        handler: None,
        lane: OrderLane::Alias(84),
        class: OrderClass::Harvest,
    },
    OrderSpec {
        id: 79,
        name: "Harvest Gas",
        handler: Some(0x7a14ba20),
        lane: OrderLane::Carry8,
        class: OrderClass::Harvest,
    },
    OrderSpec {
        id: 80,
        name: "Harvest Gas",
        handler: Some(0x7a1537a0),
        lane: OrderLane::Carry8,
        class: OrderClass::Harvest,
    },
    OrderSpec {
        id: 81,
        name: "Return Gas",
        handler: Some(0x7a14ba20),
        lane: OrderLane::Carry8,
        class: OrderClass::ReturnCargo,
    },
    OrderSpec {
        id: 82,
        name: "Return Gas",
        handler: Some(0x7a14bcd0),
        lane: OrderLane::Carry8,
        class: OrderClass::ReturnCargo,
    },
    OrderSpec {
        id: 83,
        name: "Return Gas",
        handler: Some(0x7a14bb30),
        lane: OrderLane::Carry8,
        class: OrderClass::ReturnCargo,
    },
    OrderSpec {
        id: 84,
        name: "Harvest Minerals",
        handler: Some(0x7a14c1e0),
        lane: OrderLane::Carry8,
        class: OrderClass::Harvest,
    },
    OrderSpec {
        id: 85,
        name: "Return Minerals",
        handler: Some(0x7a14bed0),
        lane: OrderLane::Carry8,
        class: OrderClass::ReturnCargo,
    },
    OrderSpec {
        id: 86,
        name: "Return Minerals",
        handler: Some(0x7a14c150),
        lane: OrderLane::Carry8,
        class: OrderClass::ReturnCargo,
    },
    OrderSpec {
        id: 87,
        name: "Return Minerals",
        handler: Some(0x7a14bd20),
        lane: OrderLane::Carry8,
        class: OrderClass::ReturnCargo,
    },
    OrderSpec {
        id: 88,
        name: "Harvest Gas",
        handler: Some(0x7a14c1a0),
        lane: OrderLane::EveryTick,
        class: OrderClass::Harvest,
    },
    OrderSpec {
        id: 89,
        name: "Harvest Gas",
        handler: Some(0x7a14c1c0),
        lane: OrderLane::EveryTick,
        class: OrderClass::Harvest,
    },
    OrderSpec {
        id: 90,
        name: "Enter Transport",
        handler: Some(0x7a14c1e0),
        lane: OrderLane::Carry8,
        class: OrderClass::Transport,
    },
    OrderSpec {
        id: 91,
        name: "Harvest Gas",
        handler: Some(0x7a162410),
        lane: OrderLane::EveryTick,
        class: OrderClass::Harvest,
    },
    OrderSpec {
        id: 92,
        name: "Pick Up",
        handler: Some(0x7a174d80),
        lane: OrderLane::Carry8,
        class: OrderClass::Transport,
    },
    OrderSpec {
        id: 93,
        name: "Powerup",
        handler: Some(0x7a175270),
        lane: OrderLane::Carry8,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 94,
        name: "Powerup",
        handler: Some(0x7a174e60),
        lane: OrderLane::Carry8,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 95,
        name: "Powerup",
        handler: Some(0x7a174f00),
        lane: OrderLane::Carry8,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 96,
        name: "Powerup",
        handler: Some(0x7a1750e0),
        lane: OrderLane::Carry8,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 97,
        name: "Siege Mode",
        handler: Some(0x7a1533d0),
        lane: OrderLane::Carry8,
        class: OrderClass::Siege,
    },
    OrderSpec {
        id: 98,
        name: "Tank Mode",
        handler: Some(0x7a173fe0),
        lane: OrderLane::EveryTick,
        class: OrderClass::Siege,
    },
    OrderSpec {
        id: 99,
        name: "Watch Target",
        handler: Some(0x7a174190),
        lane: OrderLane::EveryTick,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 100,
        name: "Initing Creep Growth",
        handler: Some(0x7a154800),
        lane: OrderLane::Carry8,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 101,
        name: "Stopping Creep Growth",
        handler: Some(0x7a17d780),
        lane: OrderLane::Carry8,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 102,
        name: "Guardian Aspect",
        handler: None,
        lane: OrderLane::Engine,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 103,
        name: "Spread Creep",
        handler: Some(0x7a17fbc0),
        lane: OrderLane::Carry8,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 104,
        name: "Warping Archon",
        handler: None,
        lane: OrderLane::Engine,
        class: OrderClass::Morph,
    },
    OrderSpec {
        id: 105,
        name: "Completing Archon Summon",
        handler: Some(0x7a162460),
        lane: OrderLane::EveryTick,
        class: OrderClass::Morph,
    },
    OrderSpec {
        id: 106,
        name: "Hold Position",
        handler: Some(0x7a1642e0),
        lane: OrderLane::EveryTick,
        class: OrderClass::Hold,
    },
    OrderSpec {
        id: 107,
        name: "Cloak",
        handler: Some(0x7a13fb50),
        lane: OrderLane::Carry8,
        class: OrderClass::Cast,
    },
    OrderSpec {
        id: 108,
        name: "Cloak",
        handler: Some(0x7a15ba30),
        lane: OrderLane::Carry8,
        class: OrderClass::Cast,
    },
    OrderSpec {
        id: 109,
        name: "Decloak",
        handler: None,
        lane: OrderLane::Alias(57),
        class: OrderClass::Cast,
    },
    OrderSpec {
        id: 110,
        name: "Unload",
        handler: None,
        lane: OrderLane::Alias(111),
        class: OrderClass::Unload,
    },
    OrderSpec {
        id: 111,
        name: "Move Unload",
        handler: Some(0x7a1752c0),
        lane: OrderLane::Carry8,
        class: OrderClass::Unload,
    },
    OrderSpec {
        id: 112,
        name: "Fire Yamato Gun",
        handler: Some(0x7a174f30),
        lane: OrderLane::Carry8,
        class: OrderClass::Cast,
    },
    OrderSpec {
        id: 113,
        name: "Magna Pulse",
        handler: Some(0x7a16bea0),
        lane: OrderLane::Carry8,
        class: OrderClass::Cast,
    },
    OrderSpec {
        id: 114,
        name: "Magna Pulse",
        handler: Some(0x7a1537a0),
        lane: OrderLane::Carry8,
        class: OrderClass::Cast,
    },
    OrderSpec {
        id: 115,
        name: "Burrow",
        handler: Some(0x7a16bea0),
        lane: OrderLane::Carry8,
        class: OrderClass::Burrow,
    },
    OrderSpec {
        id: 116,
        name: "Burrowed",
        handler: Some(0x7a17ced0),
        lane: OrderLane::Carry8,
        class: OrderClass::Burrow,
    },
    OrderSpec {
        id: 117,
        name: "Unburrow",
        handler: Some(0x7a17d140),
        lane: OrderLane::Carry8,
        class: OrderClass::Burrow,
    },
    OrderSpec {
        id: 118,
        name: "Cast Parasite",
        handler: Some(0x7a17d1c0),
        lane: OrderLane::Carry8,
        class: OrderClass::Cast,
    },
    OrderSpec {
        id: 119,
        name: "rParasite",
        handler: Some(0x7a16bea0),
        lane: OrderLane::Carry8,
        class: OrderClass::Cast,
    },
    OrderSpec {
        id: 120,
        name: "Summon Broodlings",
        handler: Some(0x7a16bea0),
        lane: OrderLane::Carry8,
        class: OrderClass::Cast,
    },
    OrderSpec {
        id: 121,
        name: "EMP Shockwave",
        handler: Some(0x7a16bea0),
        lane: OrderLane::Carry8,
        class: OrderClass::Cast,
    },
    OrderSpec {
        id: 122,
        name: "Lockdown",
        handler: Some(0x7a16bea0),
        lane: OrderLane::Carry8,
        class: OrderClass::Cast,
    },
    OrderSpec {
        id: 123,
        name: "Nuke Train",
        handler: None,
        lane: OrderLane::Engine,
        class: OrderClass::Cast,
    },
    OrderSpec {
        id: 124,
        name: "Nuke Launch",
        handler: Some(0x7a171e80),
        lane: OrderLane::EveryTick,
        class: OrderClass::Cast,
    },
    OrderSpec {
        id: 125,
        name: "Nuke Paint",
        handler: Some(0x7a173890),
        lane: OrderLane::Carry8,
        class: OrderClass::Cast,
    },
    OrderSpec {
        id: 126,
        name: "Nuke Unit",
        handler: Some(0x7a173b50),
        lane: OrderLane::Carry8,
        class: OrderClass::Cast,
    },
    OrderSpec {
        id: 127,
        name: "Nuke Ground",
        handler: Some(0x7a13fe40),
        lane: OrderLane::Carry8,
        class: OrderClass::Cast,
    },
    OrderSpec {
        id: 128,
        name: "Nuke Track",
        handler: Some(0x7a13fbe0),
        lane: OrderLane::Carry8,
        class: OrderClass::Cast,
    },
    OrderSpec {
        id: 129,
        name: "Initializing Arbiter",
        handler: Some(0x7a173b80),
        lane: OrderLane::EveryTick,
        class: OrderClass::Cast,
    },
    OrderSpec {
        id: 130,
        name: "Cloaking nearby units",
        handler: Some(0x7a162100),
        lane: OrderLane::EveryTick,
        class: OrderClass::Cast,
    },
    OrderSpec {
        id: 131,
        name: "Place Mine",
        handler: None,
        lane: OrderLane::Engine,
        class: OrderClass::Cast,
    },
    OrderSpec {
        id: 132,
        name: "Right Click Action",
        handler: Some(0x7a173c80),
        lane: OrderLane::Carry8,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 133,
        name: "Left Click Action",
        handler: Some(0x7a1656e0),
        lane: OrderLane::Carry8,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 134,
        name: "Sap Location",
        handler: Some(0x7a1401c0),
        lane: OrderLane::Carry8,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 135,
        name: "Teleport",
        handler: Some(0x7a140120),
        lane: OrderLane::Carry8,
        class: OrderClass::Transport,
    },
    OrderSpec {
        id: 136,
        name: "Cloak",
        handler: Some(0x7a15ba30),
        lane: OrderLane::Carry8,
        class: OrderClass::Cast,
    },
    OrderSpec {
        id: 137,
        name: "Teleport to Location",
        handler: Some(0x7a1645f0),
        lane: OrderLane::Carry8,
        class: OrderClass::Transport,
    },
    OrderSpec {
        id: 138,
        name: "Place Scanner",
        handler: None,
        lane: OrderLane::Engine,
        class: OrderClass::Cast,
    },
    OrderSpec {
        id: 139,
        name: "Scanner",
        handler: Some(0x7a173dd0),
        lane: OrderLane::Carry8,
        class: OrderClass::Cast,
    },
    OrderSpec {
        id: 140,
        name: "Defensive Matrix",
        handler: Some(0x7a173fc0),
        lane: OrderLane::Carry8,
        class: OrderClass::Cast,
    },
    OrderSpec {
        id: 141,
        name: "Reset Collision",
        handler: Some(0x7a1723a0),
        lane: OrderLane::Carry8,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 142,
        name: "lHallucination",
        handler: Some(0x7a16bea0),
        lane: OrderLane::Carry8,
        class: OrderClass::Cast,
    },
    OrderSpec {
        id: 143,
        name: "yYamato Gun",
        handler: Some(0x7a16bea0),
        lane: OrderLane::Carry8,
        class: OrderClass::Cast,
    },
    OrderSpec {
        id: 144,
        name: "cConsume",
        handler: Some(0x7a16bea0),
        lane: OrderLane::Carry8,
        class: OrderClass::Cast,
    },
    OrderSpec {
        id: 145,
        name: "uConsume",
        handler: Some(0x7a16bea0),
        lane: OrderLane::Carry8,
        class: OrderClass::Cast,
    },
    OrderSpec {
        id: 146,
        name: "Burrow: Evolve at Hatchery",
        handler: None,
        lane: OrderLane::Carry8,
        class: OrderClass::Burrow,
    },
    OrderSpec {
        id: 147,
        name: "rArchon Warp",
        handler: Some(0x7a16bea0),
        lane: OrderLane::Carry8,
        class: OrderClass::Morph,
    },
    OrderSpec {
        id: 148,
        name: "Recall",
        handler: Some(0x7a15af50),
        lane: OrderLane::Carry8,
        class: OrderClass::Cast,
    },
    OrderSpec {
        id: 149,
        name: "Recall",
        handler: None,
        lane: OrderLane::Alias(148),
        class: OrderClass::Cast,
    },
    OrderSpec {
        id: 150,
        name: "Reset Collision",
        handler: Some(0x7a12dd40),
        lane: OrderLane::EveryTick,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 151,
        name: "Patrol",
        handler: Some(0x7a160fb0),
        lane: OrderLane::EveryTick,
        class: OrderClass::Patrol,
    },
    OrderSpec {
        id: 152,
        name: "Computer AI",
        handler: Some(0x7a13fe80),
        lane: OrderLane::Carry8,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 153,
        name: "CTF COP",
        handler: Some(0x7a1305f0),
        lane: OrderLane::Carry8,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 154,
        name: "Critter",
        handler: Some(0x7a1305c0),
        lane: OrderLane::Carry8,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 155,
        name: "Critter",
        handler: Some(0x7a1306b0),
        lane: OrderLane::EveryTick,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 156,
        name: "Guard Post",
        handler: Some(0x7a1dba50),
        lane: OrderLane::Carry8,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 157,
        name: "Harass Move",
        handler: Some(0x7a13f290),
        lane: OrderLane::Carry8,
        class: OrderClass::AttackMove,
    },
    OrderSpec {
        id: 158,
        name: "AI Patrol",
        handler: Some(0x7a13fa10),
        lane: OrderLane::Carry8,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 159,
        name: "Tower",
        handler: Some(0x7a1dbc00),
        lane: OrderLane::Carry8,
        class: OrderClass::Attack,
    },
    OrderSpec {
        id: 160,
        name: "Rescue Passive",
        handler: Some(0x7a13f950),
        lane: OrderLane::Carry8,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 161,
        name: "Neutral",
        handler: Some(0x7a1dbef0),
        lane: OrderLane::Carry8,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 162,
        name: "Computer Return",
        handler: Some(0x7a1dbcf0),
        lane: OrderLane::Carry8,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 163,
        name: "Initing Psi Provider",
        handler: Some(0x7a13f110),
        lane: OrderLane::Carry8,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 164,
        name: "Self Destructing",
        handler: Some(0x7a1621a0),
        lane: OrderLane::Carry8,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 165,
        name: "Decaying creep",
        handler: Some(0x7a157f10),
        lane: OrderLane::EveryTick,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 166,
        name: "Hidden Gun",
        handler: Some(0x7a153c30),
        lane: OrderLane::EveryTick,
        class: OrderClass::Attack,
    },
    OrderSpec {
        id: 167,
        name: "Open Door",
        handler: Some(0x7a154250),
        lane: OrderLane::Carry8,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 168,
        name: "Close Door",
        handler: Some(0x7a153ee0),
        lane: OrderLane::Carry8,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 169,
        name: "Hide Trap",
        handler: Some(0x7a153e90),
        lane: OrderLane::Carry8,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 170,
        name: "Reveal Trap",
        handler: Some(0x7a154370),
        lane: OrderLane::Carry8,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 171,
        name: "Enable Doodad",
        handler: Some(0x7a154650),
        lane: OrderLane::Carry8,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 172,
        name: "Disable Doodad",
        handler: Some(0x7a153f40),
        lane: OrderLane::Carry8,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 173,
        name: "Warp In",
        handler: Some(0x7a153d60),
        lane: OrderLane::Carry8,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 174,
        name: "Hide and Suicide",
        handler: Some(0x7a15a8d0),
        lane: OrderLane::EveryTick,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 175,
        name: "Medic Heal",
        handler: Some(0x7a173520),
        lane: OrderLane::Carry8,
        class: OrderClass::Cast,
    },
    OrderSpec {
        id: 176,
        name: "Restoration",
        handler: Some(0x7a1735c0),
        lane: OrderLane::EveryTick,
        class: OrderClass::Cast,
    },
    OrderSpec {
        id: 177,
        name: "Medic Hold Position",
        handler: Some(0x7a173680),
        lane: OrderLane::EveryTick,
        class: OrderClass::Hold,
    },
    OrderSpec {
        id: 178,
        name: "Junk Yard Dog",
        handler: Some(0x7a173780),
        lane: OrderLane::EveryTick,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 179,
        name: "Restoration",
        handler: Some(0x7a173870),
        lane: OrderLane::EveryTick,
        class: OrderClass::Cast,
    },
    OrderSpec {
        id: 180,
        name: "Cast Disruption Web",
        handler: Some(0x7a16bea0),
        lane: OrderLane::Carry8,
        class: OrderClass::Cast,
    },
    OrderSpec {
        id: 181,
        name: "Cast Mind Control",
        handler: Some(0x7a16bea0),
        lane: OrderLane::Carry8,
        class: OrderClass::Cast,
    },
    OrderSpec {
        id: 182,
        name: "Warping Dark Archon",
        handler: Some(0x7a15b1a0),
        lane: OrderLane::Carry8,
        class: OrderClass::Morph,
    },
    OrderSpec {
        id: 183,
        name: "Cast Feedback",
        handler: Some(0x7a1643a0),
        lane: OrderLane::EveryTick,
        class: OrderClass::Cast,
    },
    OrderSpec {
        id: 184,
        name: "Cast Optical Flare",
        handler: Some(0x7a15b340),
        lane: OrderLane::Carry8,
        class: OrderClass::Cast,
    },
    OrderSpec {
        id: 185,
        name: "Cast Shockwave",
        handler: Some(0x7a16bea0),
        lane: OrderLane::Carry8,
        class: OrderClass::Cast,
    },
    OrderSpec {
        id: 186,
        name: "Heal Move",
        handler: Some(0x7a16bea0),
        lane: OrderLane::Carry8,
        class: OrderClass::Cast,
    },
    OrderSpec {
        id: 187,
        name: "Terran Expansion Custom Level",
        handler: Some(0x7a154490),
        lane: OrderLane::Carry8,
        class: OrderClass::Special,
    },
    OrderSpec {
        id: 188,
        name: "Nuclear launch detected.",
        handler: None,
        lane: OrderLane::Engine,
        class: OrderClass::Special,
    },
];

/// Look up a row.  Ids outside the shipped table return `None` instead of
/// being silently reinterpreted.
pub fn spec(id: u8) -> Option<&'static OrderSpec> {
    ORDER_SPECS.get(id as usize)
}

pub fn order_class(id: u8) -> OrderClass {
    spec(id).map(|s| s.class).unwrap_or(OrderClass::Idle)
}

pub fn lane(id: u8) -> OrderLane {
    spec(id).map(|s| s.lane).unwrap_or(OrderLane::Engine)
}

/// The `target order` column of `orders.dat`, also present at runtime as the
/// byte table `0x7ab2b050` indexed by the current order byte.  The dispatcher
/// uses it when a target becomes invalid (`0x7a1537a0`): the unit re-issues
/// the mapped order instead of the row's own id.  Ten shipped rows differ
/// from the identity mapping.
pub const TARGET_ORDER: [u8; 189] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 8, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
    26, 27, 27, 29, 30, 31, 32, 33, 34, 34, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49,
    50, 51, 52, 53, 53, 55, 56, 57, 58, 59, 59, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73,
    74, 75, 76, 77, 78, 79, 79, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97,
    98, 99, 100, 101, 102, 103, 104, 105, 106, 107, 107, 109, 110, 111, 112, 113, 113, 115, 116,
    117, 118, 119, 120, 121, 122, 123, 124, 125, 126, 127, 128, 129, 130, 131, 132, 133, 134, 135,
    136, 137, 138, 139, 140, 141, 142, 143, 144, 145, 146, 147, 148, 149, 150, 151, 152, 153, 154,
    155, 156, 157, 157, 157, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 170, 171, 172, 173,
    174, 175, 176, 177, 178, 179, 180, 181, 182, 183, 184, 185, 186, 187, 188,
];

pub fn target_order(id: u8) -> u8 {
    TARGET_ORDER.get(id as usize).copied().unwrap_or(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn every_shipped_row_has_a_spec() {
        assert_eq!(ORDER_SPECS.len(), 189);
        for (i, s) in ORDER_SPECS.iter().enumerate() {
            assert_eq!(s.id as usize, i);
            assert!(!s.name.is_empty());
        }
    }

    #[test]
    fn dispatcher_lanes_match_the_recovered_switch() {
        // The three recovered switch case sets from `0x7a178340`.
        let fast = [0u8, 48, 129, 174];
        let every = [
            4u8, 22, 26, 30, 31, 33, 34, 41, 42, 44, 45, 65, 66, 71, 72, 75, 76, 88, 89, 91, 98,
            99, 105, 106, 124, 130, 150, 151, 155, 165, 166, 176, 177, 178, 179, 183,
        ];
        let mut fast_rows = BTreeSet::new();
        let mut every_rows = BTreeSet::new();
        let mut carry_rows = BTreeSet::new();
        for s in &ORDER_SPECS {
            match s.lane {
                OrderLane::Carry8 => {
                    carry_rows.insert(s.id);
                }
                OrderLane::EveryTick => {
                    every_rows.insert(s.id);
                }
                _ => {}
            }
            if let Some(f) = s.handler
                && fast.contains(&s.id)
            {
                fast_rows.insert(s.id);
                assert!(f != 0);
            }
        }
        for id in fast {
            assert!(fast_rows.contains(&id), "fast case {id} missing");
        }
        // Aliased rows are not in the direct sets; every other direct row must
        // be in exactly one of the two remaining switches.
        let mut expected_every: BTreeSet<u8> = every.iter().copied().collect();
        expected_every.extend(fast.iter().copied());
        assert_eq!(every_rows, expected_every);
        assert_eq!(carry_rows.len(), 128);
        for id in 0..189u8 {
            if !every_rows.contains(&id) && !carry_rows.contains(&id) {
                assert!(
                    matches!(
                        lane(id),
                        OrderLane::Alias(_) | OrderLane::Production | OrderLane::Engine
                    ),
                    "row {id} has no recovered lane"
                );
            }
        }
    }

    #[test]
    fn target_order_column_matches_the_shipped_rows() {
        let g = crate::data::GameData::bundled().unwrap();
        for id in 0..189u8 {
            assert_eq!(
                target_order(id),
                g.order(id).unwrap().target_order,
                "target order for row {id}"
            );
        }
        // The ten non-identity rows recovered from `0x7ab2b050`.
        let non_identity: Vec<u8> = (0..189u8).filter(|&i| target_order(i) != i).collect();
        assert_eq!(
            non_identity,
            vec![9, 28, 35, 54, 60, 80, 108, 114, 158, 159]
        );
        assert_eq!(target_order(9), 8);
    }

    #[test]
    fn every_row_has_an_explicit_class() {
        // Rows that the older range-folding classified as generic Special but
        // that are gameplay families in their own right.
        assert_eq!(order_class(53), OrderClass::AttackMove);
        assert_eq!(order_class(84), OrderClass::Harvest);
        assert_eq!(order_class(85), OrderClass::ReturnCargo);
        assert_eq!(order_class(97), OrderClass::Siege);
        assert_eq!(order_class(98), OrderClass::Siege);
        assert_eq!(order_class(104), OrderClass::Morph);
        assert_eq!(order_class(151), OrderClass::Patrol);
        assert_eq!(order_class(177), OrderClass::Hold);
    }
}
