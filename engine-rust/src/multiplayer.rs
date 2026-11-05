//! Multiplayer (LAN) front-end rules and the end-of-game score sheet, as the
//! retail client behaves on the user's screencast of 2026-11-04 21:11:48
//! (Select Connection -> Registry -> Games -> Create -> lobby -> countdown
//! -> game) and the score screenshot of 21:14:04.  Pure state: the client
//! (`sc_view`) draws it and the network layer carries [`GameSetup`].
//!
//! FACT unless marked: every rule names the capture it was read from.

use crate::sim::{Race, World};

/// Rows of the Select Connection list (`gluConn`), in retail order.
pub const CONNECTIONS: [&str; 2] = ["Battle.net\u{ae}", "Local Area Network"];
/// Row of the Local Area Network entry in [`CONNECTIONS`].
pub const CONNECTION_LAN: usize = 1;

/// Whether Select Connection's Ok is enabled for row `sel`: grey while
/// Battle.net is selected (on entry, 3.6-5.6 s), green once Local Area
/// Network is (5.8 s).  This client has no Battle.net.
pub fn connection_ok_enabled(sel: usize) -> bool {
    sel == CONNECTION_LAN
}

/// Retail game types (`scr.common.terms.gameType.1..13`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GameType {
    Melee,
    FreeForAll,
    OneOnOne,
    CaptureTheFlag,
    Greed,
    Slaughter,
    SuddenDeath,
    Ladder,
    UseMapSettings,
    TeamMelee,
    TeamFreeForAll,
    TeamCaptureTheFlag,
    TopVsBottom,
}

impl GameType {
    pub const ALL: [GameType; 13] = [
        GameType::Melee,
        GameType::FreeForAll,
        GameType::OneOnOne,
        GameType::CaptureTheFlag,
        GameType::Greed,
        GameType::Slaughter,
        GameType::SuddenDeath,
        GameType::Ladder,
        GameType::UseMapSettings,
        GameType::TeamMelee,
        GameType::TeamFreeForAll,
        GameType::TeamCaptureTheFlag,
        GameType::TopVsBottom,
    ];

    pub fn name(self) -> &'static str {
        match self {
            GameType::Melee => "Melee",
            GameType::FreeForAll => "Free For All",
            GameType::OneOnOne => "One on One",
            GameType::CaptureTheFlag => "Capture The Flag",
            GameType::Greed => "Greed",
            GameType::Slaughter => "Slaughter",
            GameType::SuddenDeath => "Sudden Death",
            GameType::Ladder => "Ladder",
            GameType::UseMapSettings => "Use Map Settings",
            GameType::TeamMelee => "Team Melee",
            GameType::TeamFreeForAll => "Team Free For All",
            GameType::TeamCaptureTheFlag => "Team Capture The Flag",
            GameType::TopVsBottom => "Top vs Bottom",
        }
    }

    pub fn tag(self) -> u8 {
        Self::ALL.iter().position(|&g| g == self).unwrap_or(0) as u8
    }

    pub fn from_tag(tag: u8) -> Option<Self> {
        Self::ALL.get(tag as usize).copied()
    }
}

/// Top vs Bottom "Teams" choices for a map with `slots` start locations:
/// `a vs slots - a` for `a` in `1..slots` (retail string `tvbSIDES`
/// "%d vs %d").  INFERENCE: the order of the list; the capture shows only
/// the selected value.
pub fn team_splits(slots: usize) -> Vec<(u8, u8)> {
    (1..slots.max(2))
        .map(|a| (a as u8, (slots.max(2) - a) as u8))
        .collect()
}

/// Default Top vs Bottom split: the even one.  FACT: "1 vs 1" on the
/// 2-player map, and "4 vs 4" as soon as (8)The Hunters is selected (19.2 s).
pub fn default_split(slots: usize) -> (u8, u8) {
    let n = slots.max(2);
    ((n / 2) as u8, (n - n / 2) as u8)
}

/// Who takes a lobby slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Occupant {
    Open,
    Closed,
    Computer,
    /// A player; `peer` 0 is the host, LAN clients are 1.. in join order.
    Human {
        name: String,
        peer: u8,
    },
}

impl Occupant {
    /// Label of the slot's player dropdown.
    pub fn label(&self) -> &str {
        match self {
            Occupant::Open => "Open",
            Occupant::Closed => "Closed",
            Occupant::Computer => "Computer",
            Occupant::Human { name, .. } => name,
        }
    }

    pub fn plays(&self) -> bool {
        matches!(self, Occupant::Computer | Occupant::Human { .. })
    }
}

/// The three entries a player dropdown offers for an empty or computer
/// slot (`lobbyScreen_open/closed/computer`).
pub const OCCUPANT_CHOICES: [&str; 3] = ["Open", "Closed", "Computer"];

/// A slot's race pick (`lobbyScreen_*`).  `Unselected` is the host's grey
/// "Select Race" on entry (24.5 s).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RaceChoice {
    Unselected,
    Zerg,
    Terran,
    Protoss,
    Random,
}

impl RaceChoice {
    /// Entries of the race dropdown, in retail order.
    pub const CHOICES: [RaceChoice; 4] = [
        RaceChoice::Zerg,
        RaceChoice::Terran,
        RaceChoice::Protoss,
        RaceChoice::Random,
    ];

    pub fn label(self) -> &'static str {
        match self {
            RaceChoice::Unselected => "Select Race",
            RaceChoice::Zerg => "Zerg",
            RaceChoice::Terran => "Terran",
            RaceChoice::Protoss => "Protoss",
            RaceChoice::Random => "Random",
        }
    }
}

/// Retail lobby colour names (`lobbyScreen_<colour>`), "Random" first.
pub const COLOR_CHOICES: [&str; 23] = [
    "Random",
    "Pink",
    "Red",
    "Tan",
    "Orange",
    "Brown",
    "Pale Yellow",
    "Yellow",
    "Olive",
    "Lime",
    "Pale Green",
    "Green",
    "Cyan",
    "Teal",
    "Blue",
    "Navy",
    "Dark Aqua",
    "Bluish Grey",
    "Purple",
    "Magenta",
    "White",
    "Grey",
    "Black",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LobbySlot {
    pub occupant: Occupant,
    pub race: RaceChoice,
    /// Index into [`COLOR_CHOICES`] (0 = Random).
    pub color: usize,
}

/// The game lobby (`glugamelobby`): one slot per start location, grouped
/// into teams for team game types, plus four observer seats.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lobby {
    pub game_name: String,
    pub map_name: String,
    pub game_type: GameType,
    pub split: (u8, u8),
    pub slots: Vec<LobbySlot>,
    pub observers: [Occupant; 4],
}

impl Lobby {
    /// A freshly created game: the host in slot 0 with "Select Race", every
    /// other slot Open, observers Closed (24.5 s).  Computer slots join as
    /// Random (48.0 s).
    pub fn new_host(
        host: &str,
        map_name: &str,
        slots: usize,
        game_type: GameType,
        split: (u8, u8),
    ) -> Self {
        let mut list: Vec<LobbySlot> = (0..slots.max(1))
            .map(|_| LobbySlot {
                occupant: Occupant::Open,
                race: RaceChoice::Random,
                color: 0,
            })
            .collect();
        list[0] = LobbySlot {
            occupant: Occupant::Human {
                name: host.to_string(),
                peer: 0,
            },
            race: RaceChoice::Unselected,
            color: 0,
        };
        Self {
            game_name: host.to_string(),
            map_name: map_name.to_string(),
            game_type,
            split,
            slots: list,
            observers: [
                Occupant::Closed,
                Occupant::Closed,
                Occupant::Closed,
                Occupant::Closed,
            ],
        }
    }

    /// Team headers and their slot ranges.  Top vs Bottom: "Home Team" over
    /// the first `split.0` slots and "Away Team" over the rest (24.5 s);
    /// other game types list the slots without headers.
    pub fn teams(&self) -> Vec<(&'static str, std::ops::Range<usize>)> {
        let n = self.slots.len();
        match self.game_type {
            GameType::TopVsBottom => {
                let a = (self.split.0 as usize).min(n);
                vec![("Home Team", 0..a), ("Away Team", a..n)]
            }
            _ => vec![("", 0..n)],
        }
    }

    /// Team index of slot `i` (the slot's own index outside team types).
    pub fn team_of(&self, i: usize) -> u8 {
        match self.game_type {
            GameType::TopVsBottom => u8::from(i >= self.split.0 as usize),
            _ => i as u8,
        }
    }

    /// Set slot `i` to `occupant`; a computer starts on Random.
    pub fn set_occupant(&mut self, i: usize, occupant: Occupant) {
        if let Some(s) = self.slots.get_mut(i) {
            if matches!(s.occupant, Occupant::Human { .. }) {
                return;
            }
            if occupant == Occupant::Computer {
                s.race = RaceChoice::Random;
            }
            s.occupant = occupant;
        }
    }

    /// First Open slot (where a joining LAN player sits).
    pub fn first_open(&self) -> Option<usize> {
        self.slots.iter().position(|s| s.occupant == Occupant::Open)
    }

    /// Ok (start) is enabled once every human picked a race and players
    /// sit on at least two sides.  FACT: grey with "Select Race" (24.5 s),
    /// green after Protoss was picked and computers were added (48.0 s).
    /// INFERENCE: the two-side minimum (a game with no opponent would end
    /// on its first frame; the capture only shows a valid lobby).
    pub fn can_start(&self) -> bool {
        let humans_ready = self
            .slots
            .iter()
            .filter(|s| matches!(s.occupant, Occupant::Human { .. }))
            .all(|s| s.race != RaceChoice::Unselected);
        let mut sides: Vec<u8> = (0..self.slots.len())
            .filter(|&i| self.slots[i].occupant.plays())
            .map(|i| self.team_of(i))
            .collect();
        sides.sort_unstable();
        sides.dedup();
        humans_ready && sides.len() >= 2
    }

    /// The game the lobby starts: random races resolved with `seed`, teams
    /// from the game type.  Every peer derives the same setup from the
    /// host's broadcast copy, so the result is part of the lockstep state.
    pub fn resolve(&self, seed: u64) -> GameSetup {
        let mut rng = seed ^ 0x9e37_79b9_7f4a_7c15;
        let mut next = || {
            rng = rng
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (rng >> 33) as u32
        };
        let slots = self
            .slots
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let occupant = match &s.occupant {
                    Occupant::Computer => SetupOccupant::Computer,
                    Occupant::Human { peer, .. } => SetupOccupant::Human { peer: *peer },
                    Occupant::Open | Occupant::Closed => SetupOccupant::Empty,
                };
                let race = match s.race {
                    RaceChoice::Zerg => Race::Zerg,
                    RaceChoice::Terran => Race::Terran,
                    RaceChoice::Protoss => Race::Protoss,
                    RaceChoice::Random | RaceChoice::Unselected => {
                        [Race::Zerg, Race::Terran, Race::Protoss][(next() % 3) as usize]
                    }
                };
                SetupSlot {
                    occupant,
                    race,
                    team: self.team_of(i),
                }
            })
            .collect();
        GameSetup {
            game_type: self.game_type,
            slots,
        }
    }
}

/// Longest player/game/map name carried on the wire.
pub const MAX_NAME: usize = 32;

fn put_str(out: &mut Vec<u8>, s: &str) {
    let b = s.as_bytes();
    let mut n = b.len().min(MAX_NAME);
    while !s.is_char_boundary(n) {
        n -= 1;
    }
    out.push(n as u8);
    out.extend_from_slice(&b[..n]);
}

fn take_str(b: &[u8], o: &mut usize) -> Option<String> {
    let n = *b.get(*o)? as usize;
    if n > MAX_NAME {
        return None;
    }
    let s = b.get(*o + 1..*o + 1 + n)?;
    *o += 1 + n;
    String::from_utf8(s.to_vec()).ok()
}

fn take_u8(b: &[u8], o: &mut usize) -> Option<u8> {
    let v = *b.get(*o)?;
    *o += 1;
    Some(v)
}

fn occupant_tag(o: &Occupant) -> (u8, u8) {
    match o {
        Occupant::Open => (0, 0),
        Occupant::Closed => (1, 0),
        Occupant::Computer => (2, 0),
        Occupant::Human { peer, .. } => (3, *peer),
    }
}

fn race_choice_tag(r: RaceChoice) -> u8 {
    match r {
        RaceChoice::Unselected => 0,
        RaceChoice::Zerg => 1,
        RaceChoice::Terran => 2,
        RaceChoice::Protoss => 3,
        RaceChoice::Random => 4,
    }
}

/// Inverse of the race-choice wire tag.
pub fn race_choice_from(tag: u8) -> Option<RaceChoice> {
    Some(match tag {
        0 => RaceChoice::Unselected,
        1 => RaceChoice::Zerg,
        2 => RaceChoice::Terran,
        3 => RaceChoice::Protoss,
        4 => RaceChoice::Random,
        _ => return None,
    })
}

/// Wire tag of a race choice (the `LobbyRace` message).
pub fn race_choice_byte(r: RaceChoice) -> u8 {
    race_choice_tag(r)
}

impl Lobby {
    /// Wire form the host sends joined players so they see the lobby:
    /// names, game type, split, then per slot `[kind][peer][race][colour]`
    /// (+ name for humans) and the four observer seats.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        put_str(&mut out, &self.game_name);
        put_str(&mut out, &self.map_name);
        out.extend_from_slice(&[self.game_type.tag(), self.split.0, self.split.1]);
        let n = self.slots.len().min(MAX_SLOTS);
        out.push(n as u8);
        for s in self.slots.iter().take(n) {
            let (kind, peer) = occupant_tag(&s.occupant);
            out.extend_from_slice(&[kind, peer, race_choice_tag(s.race), s.color.min(255) as u8]);
            if let Occupant::Human { name, .. } = &s.occupant {
                put_str(&mut out, name);
            }
        }
        for o in &self.observers {
            out.push(occupant_tag(o).0.min(2));
        }
        out
    }

    /// Inverse of [`Self::encode`]; `None` on any malformed byte.
    pub fn decode(b: &[u8]) -> Option<Self> {
        let mut o = 0;
        let game_name = take_str(b, &mut o)?;
        let map_name = take_str(b, &mut o)?;
        let game_type = GameType::from_tag(take_u8(b, &mut o)?)?;
        let split = (take_u8(b, &mut o)?, take_u8(b, &mut o)?);
        let n = take_u8(b, &mut o)? as usize;
        if n == 0 || n > MAX_SLOTS {
            return None;
        }
        let mut slots = Vec::with_capacity(n);
        for _ in 0..n {
            let kind = take_u8(b, &mut o)?;
            let peer = take_u8(b, &mut o)?;
            let race = race_choice_from(take_u8(b, &mut o)?)?;
            let color = take_u8(b, &mut o)? as usize;
            if color >= COLOR_CHOICES.len() {
                return None;
            }
            let occupant = match kind {
                0 => Occupant::Open,
                1 => Occupant::Closed,
                2 => Occupant::Computer,
                3 => Occupant::Human {
                    name: take_str(b, &mut o)?,
                    peer,
                },
                _ => return None,
            };
            slots.push(LobbySlot {
                occupant,
                race,
                color,
            });
        }
        let mut observers = [
            Occupant::Closed,
            Occupant::Closed,
            Occupant::Closed,
            Occupant::Closed,
        ];
        for seat in observers.iter_mut() {
            *seat = match take_u8(b, &mut o)? {
                0 => Occupant::Open,
                1 => Occupant::Closed,
                _ => return None,
            };
        }
        if o != b.len() {
            return None;
        }
        Some(Self {
            game_name,
            map_name,
            game_type,
            split,
            slots,
            observers,
        })
    }

    /// Slot of LAN peer `peer`.
    pub fn slot_of_peer(&self, peer: u8) -> Option<usize> {
        self.slots
            .iter()
            .position(|s| matches!(s.occupant, Occupant::Human { peer: p, .. } if p == peer))
    }

    /// Peer `peer` left the lobby: its slot opens and later peers move down
    /// one index (the host's join queue renumbers them the same way).
    pub fn remove_peer(&mut self, peer: u8) {
        for s in &mut self.slots {
            match &mut s.occupant {
                Occupant::Human { peer: p, .. } if *p == peer => {
                    s.occupant = Occupant::Open;
                    s.race = RaceChoice::Random;
                }
                Occupant::Human { peer: p, .. } if *p > peer => *p -= 1,
                _ => {}
            }
        }
    }
}

/// Who plays a resolved slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetupOccupant {
    Empty,
    Computer,
    Human { peer: u8 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SetupSlot {
    pub occupant: SetupOccupant,
    pub race: Race,
    pub team: u8,
}

/// A started lobby game: what the world is built from and what saves,
/// replays and LAN peers must agree on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameSetup {
    pub game_type: GameType,
    pub slots: Vec<SetupSlot>,
}

/// Upper bound on encoded slots (twelve CHK players).
const MAX_SLOTS: usize = 12;

impl GameSetup {
    pub fn races(&self) -> Vec<Race> {
        self.slots.iter().map(|s| s.race).collect()
    }

    pub fn occupied(&self) -> Vec<bool> {
        self.slots
            .iter()
            .map(|s| s.occupant != SetupOccupant::Empty)
            .collect()
    }

    /// Slot of LAN peer `peer`.
    pub fn slot_of_peer(&self, peer: u8) -> Option<usize> {
        self.slots
            .iter()
            .position(|s| s.occupant == SetupOccupant::Human { peer })
    }

    /// Apply teams, shared vision, computer flags and empty slots to a world
    /// built from the same setup (`World::from_scenario_occupied`).  Team
    /// game types ally each team and share its vision (retail Top vs Bottom).
    pub fn apply(&self, world: &mut World) {
        let team_game = self.game_type == GameType::TopVsBottom;
        for (i, s) in self.slots.iter().enumerate() {
            let Some(p) = world.players.get_mut(i) else {
                continue;
            };
            p.team = if team_game { s.team } else { i as u8 };
            p.is_computer = s.occupant == SetupOccupant::Computer;
            if s.occupant == SetupOccupant::Empty {
                p.defeated = true;
            }
            p.vision_from = 0;
        }
        if team_game {
            for i in 0..self.slots.len().min(world.players.len()) {
                let mask = self
                    .slots
                    .iter()
                    .enumerate()
                    .filter(|(j, s)| *j != i && s.team == self.slots[i].team)
                    .fold(0u16, |m, (j, _)| m | (1 << j));
                world.players[i].vision_from = mask;
            }
        }
    }

    /// `[game_type u8][count u8]` then `[kind u8][peer u8][race u8][team u8]`
    /// per slot (kind 0 empty, 1 computer, 2 human).
    pub fn encode(&self) -> Vec<u8> {
        let mut out = vec![self.game_type.tag(), self.slots.len().min(MAX_SLOTS) as u8];
        for s in self.slots.iter().take(MAX_SLOTS) {
            let (kind, peer) = match s.occupant {
                SetupOccupant::Empty => (0, 0),
                SetupOccupant::Computer => (1, 0),
                SetupOccupant::Human { peer } => (2, peer),
            };
            out.extend_from_slice(&[kind, peer, race_tag(s.race), s.team]);
        }
        out
    }

    /// Inverse of [`Self::encode`]; `None` on any malformed byte.
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        let (&gt, rest) = bytes.split_first()?;
        let (&n, rest) = rest.split_first()?;
        let n = n as usize;
        if n > MAX_SLOTS || rest.len() != n * 4 {
            return None;
        }
        let game_type = GameType::from_tag(gt)?;
        let mut slots = Vec::with_capacity(n);
        for c in rest.as_chunks::<4>().0 {
            let occupant = match c[0] {
                0 => SetupOccupant::Empty,
                1 => SetupOccupant::Computer,
                2 => SetupOccupant::Human { peer: c[1] },
                _ => return None,
            };
            let race = match c[2] {
                0 => Race::Zerg,
                1 => Race::Terran,
                2 => Race::Protoss,
                _ => return None,
            };
            slots.push(SetupSlot {
                occupant,
                race,
                team: c[3],
            });
        }
        Some(Self { game_type, slots })
    }
}

fn race_tag(r: Race) -> u8 {
    match r {
        Race::Zerg => 0,
        Race::Terran => 1,
        Race::Protoss => 2,
        Race::Neutral => 1,
    }
}

/// The lobby countdown after the host presses Ok, timed on the 30 fps
/// capture: "Starting in: 5 seconds" at 52.20 s, then one step per second
/// to "0" at 57.23 s; Cancel greys at "3" (54.2 s), the dropdowns at "2"
/// (55.2 s), and the panels swish out 5.1 s after the start (57.30 s).
pub mod countdown {
    /// Seconds from Ok to the swish out.
    pub const DURATION: f32 = 5.1;

    /// The number shown `elapsed` seconds after Ok.
    pub fn seconds_left(elapsed: f32) -> u8 {
        (5.0 - elapsed.max(0.0).floor()).clamp(0.0, 5.0) as u8
    }

    /// "Starting in: N seconds" (plural for every N, as retail).
    pub fn text(elapsed: f32) -> String {
        format!("Starting in: {} seconds", seconds_left(elapsed))
    }

    pub fn cancel_enabled(elapsed: f32) -> bool {
        seconds_left(elapsed) > 3
    }

    pub fn dropdowns_enabled(elapsed: f32) -> bool {
        seconds_left(elapsed) > 2
    }

    pub fn finished(elapsed: f32) -> bool {
        elapsed >= DURATION
    }
}

/// Score screen tabs (`gluScore` radio buttons) and their column headers
/// (`scoreU*`, `scoreS*`, `scoreR*` in `gluall.txt`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScoreTab {
    Overview,
    Units,
    Structures,
    Resources,
}

impl ScoreTab {
    pub const ALL: [ScoreTab; 4] = [
        ScoreTab::Overview,
        ScoreTab::Units,
        ScoreTab::Structures,
        ScoreTab::Resources,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ScoreTab::Overview => "Overview",
            ScoreTab::Units => "Units",
            ScoreTab::Structures => "Structures",
            ScoreTab::Resources => "Resources",
        }
    }

    pub fn columns(self) -> [&'static str; 3] {
        match self {
            ScoreTab::Overview => ["Units", "Structures", "Resources"],
            ScoreTab::Units => ["Produced", "Killed", "Lost"],
            ScoreTab::Structures => ["Constructed", "Razed", "Lost"],
            ScoreTab::Resources => ["Minerals Mined", "Gas Mined", "Total Spent"],
        }
    }
}

/// One row of the score screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScoreLine {
    pub player: u8,
    pub race: Race,
    /// The tab's three columns.
    pub values: [i32; 3],
    /// Total Score (the Overview sum, shown on every tab).
    pub total: i32,
}

/// Overview columns: Units = unit + kill score, Structures = building +
/// razing score, Resources = minerals and gas gathered including the
/// starting stock.  FACT (21:14:04): a Protoss start that never mined reads
/// 200 / 400 / 50, total 650; every total is the sum of its row.
fn overview(world: &World, p: usize) -> [i32; 3] {
    let pl = &world.players[p];
    [
        pl.scores[1] + pl.scores[4],
        pl.scores[2] + pl.scores[5],
        pl.minerals_gathered + pl.gas_gathered + pl.start_minerals + pl.start_gas,
    ]
}

/// Units/structures `p` lost, from its per-type death counters.
fn losses(world: &World, p: usize) -> (i32, i32) {
    let mut units = 0;
    let mut structures = 0;
    for (t, &n) in world.players[p].death_counts.iter().enumerate() {
        if n == 0 {
            continue;
        }
        let flags = world.data.unit(t as u16).group_flags;
        if flags & crate::sim::GROUP_MEN != 0 {
            units += n as i32;
        } else if flags & crate::sim::GROUP_BUILDING != 0 {
            structures += n as i32;
        }
    }
    (units, structures)
}

fn tab_values(world: &World, p: usize, tab: ScoreTab) -> [i32; 3] {
    let pl = &world.players[p];
    match tab {
        ScoreTab::Overview => overview(world, p),
        ScoreTab::Units => [
            pl.units_produced as i32,
            pl.kills.saturating_sub(pl.razings) as i32,
            losses(world, p).0,
        ],
        ScoreTab::Structures => [
            pl.structures_built as i32,
            pl.razings as i32,
            losses(world, p).1,
        ],
        ScoreTab::Resources => {
            let minerals = pl.minerals_gathered + pl.start_minerals;
            let gas = pl.gas_gathered + pl.start_gas;
            // Spent = everything owned minus what is still in the bank.
            let spent = (minerals - pl.minerals).max(0) + (gas - pl.gas).max(0);
            [minerals, gas, spent]
        }
    }
}

/// Score rows for the players that took part, highest total first (ties by
/// player order), with the columns of `tab`.
pub fn score_table(world: &World, players: &[u8], tab: ScoreTab) -> Vec<ScoreLine> {
    let mut lines: Vec<ScoreLine> = players
        .iter()
        .filter(|&&p| (p as usize) < world.players.len())
        .map(|&p| ScoreLine {
            player: p,
            race: world.players[p as usize].race,
            values: tab_values(world, p as usize, tab),
            total: overview(world, p as usize).iter().sum(),
        })
        .collect();
    lines.sort_by(|a, b| b.total.cmp(&a.total).then(a.player.cmp(&b.player)));
    lines
}

/// Overview rows (see [`score_table`]).
pub fn score_lines(world: &World, players: &[u8]) -> Vec<ScoreLine> {
    score_table(world, players, ScoreTab::Overview)
}

/// Fill fraction of a score bar: the value against the column's highest
/// value (retail 21:14:04: 400 of 550 fills ~0.73 of the box).
pub fn bar_fraction(value: i32, column_max: i32) -> f32 {
    if column_max <= 0 {
        0.0
    } else {
        (value.max(0) as f32 / column_max as f32).clamp(0.0, 1.0)
    }
}

/// "Elapsed Time: MM:SS" (`scoreTIME_s`) for `ticks` game frames at
/// Fastest (42 ms each).
pub fn elapsed_text(ticks: u64) -> String {
    let secs = ticks * 42 / 1000;
    format!("Elapsed Time: {:02}:{:02}", secs / 60, secs % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_lan_row_enables_ok() {
        assert_eq!(CONNECTIONS[CONNECTION_LAN], "Local Area Network");
        assert!(!connection_ok_enabled(0));
        assert!(connection_ok_enabled(CONNECTION_LAN));
    }

    #[test]
    fn top_vs_bottom_splits_default_to_the_even_one() {
        assert_eq!(default_split(2), (1, 1));
        assert_eq!(default_split(8), (4, 4));
        assert_eq!(team_splits(8).len(), 7);
        assert!(team_splits(8).contains(&(4, 4)));
        assert_eq!(GameType::TopVsBottom.name(), "Top vs Bottom");
        for g in GameType::ALL {
            assert_eq!(GameType::from_tag(g.tag()), Some(g));
        }
    }

    fn hunters_lobby() -> Lobby {
        Lobby::new_host("LinKa", "The Hunters", 8, GameType::TopVsBottom, (4, 4))
    }

    #[test]
    fn a_new_lobby_looks_like_the_capture() {
        let l = hunters_lobby();
        assert_eq!(l.slots[0].occupant.label(), "LinKa");
        assert_eq!(l.slots[0].race.label(), "Select Race");
        assert!(l.slots[1..].iter().all(|s| s.occupant == Occupant::Open));
        assert!(l.observers.iter().all(|o| *o == Occupant::Closed));
        let teams = l.teams();
        assert_eq!(teams[0], ("Home Team", 0..4));
        assert_eq!(teams[1], ("Away Team", 4..8));
        assert!(!l.can_start(), "Ok is grey with Select Race");
    }

    #[test]
    fn ok_needs_a_race_and_a_second_player() {
        let mut l = hunters_lobby();
        l.slots[0].race = RaceChoice::Protoss;
        assert!(!l.can_start(), "alone");
        l.set_occupant(1, Occupant::Computer);
        assert_eq!(l.slots[1].race, RaceChoice::Random);
        assert!(!l.can_start(), "a teammate is not an opponent");
        l.set_occupant(4, Occupant::Computer);
        assert!(l.can_start());
        // The host's own slot cannot be replaced.
        l.set_occupant(0, Occupant::Closed);
        assert_eq!(l.slots[0].occupant.label(), "LinKa");
    }

    #[test]
    fn resolve_is_deterministic_and_keeps_teams() {
        let mut l = hunters_lobby();
        l.slots[0].race = RaceChoice::Protoss;
        for i in 1..8 {
            l.set_occupant(i, Occupant::Computer);
        }
        let a = l.resolve(7);
        assert_eq!(a, l.resolve(7));
        assert_eq!(a.slots[0].race, Race::Protoss);
        assert_eq!(a.slots[0].occupant, SetupOccupant::Human { peer: 0 });
        assert!(
            a.slots[1..]
                .iter()
                .all(|s| s.occupant == SetupOccupant::Computer)
        );
        assert_eq!(
            a.slots.iter().map(|s| s.team).collect::<Vec<_>>(),
            [0, 0, 0, 0, 1, 1, 1, 1]
        );
        assert_eq!(GameSetup::decode(&a.encode()), Some(a.clone()));
        // Corrupt input is rejected, never panics.
        let mut bad = a.encode();
        bad[2] = 9;
        assert_eq!(GameSetup::decode(&bad), None);
        assert_eq!(GameSetup::decode(&bad[..5]), None);
        assert_eq!(GameSetup::decode(&[]), None);
        assert_eq!(
            GameSetup::decode(&[13, 1, 0, 0, 0, 0]),
            None,
            "bad game type"
        );
        assert_eq!(GameSetup::decode(&[0, 13]), None, "too many slots");
    }

    #[test]
    fn the_lobby_crosses_the_wire_and_peers_renumber() {
        let mut l = hunters_lobby();
        l.slots[0].race = RaceChoice::Protoss;
        l.set_occupant(1, Occupant::Computer);
        l.slots[2].occupant = Occupant::Human {
            name: "Raynor".into(),
            peer: 1,
        };
        l.slots[3].occupant = Occupant::Human {
            name: "Kerrigan".into(),
            peer: 2,
        };
        l.observers[1] = Occupant::Open;
        let back = Lobby::decode(&l.encode()).expect("decodes");
        assert_eq!(back, l);
        assert_eq!(back.slot_of_peer(2), Some(3));
        // Corrupt or truncated input never panics.
        let bytes = l.encode();
        for cut in 0..bytes.len() {
            assert!(Lobby::decode(&bytes[..cut]).is_none(), "cut at {cut}");
        }
        let mut extra = bytes.clone();
        extra.push(0);
        assert!(Lobby::decode(&extra).is_none());
        // Peer 1 leaves: its slot opens and peer 2 becomes peer 1.
        l.remove_peer(1);
        assert_eq!(l.slots[2].occupant, Occupant::Open);
        assert_eq!(l.slot_of_peer(1), Some(3));
        assert_eq!(l.slot_of_peer(2), None);
    }

    #[test]
    fn countdown_matches_the_capture() {
        use countdown::*;
        assert_eq!(text(0.0), "Starting in: 5 seconds");
        assert_eq!(text(4.0), "Starting in: 1 seconds");
        assert_eq!(seconds_left(5.03), 0);
        assert!(cancel_enabled(1.9) && !cancel_enabled(2.0));
        assert!(dropdowns_enabled(2.9) && !dropdowns_enabled(3.0));
        assert!(!finished(5.0) && finished(5.1));
    }

    #[test]
    fn elapsed_time_reads_like_the_score_screen() {
        // 1:18 at 42 ms per frame.
        assert_eq!(elapsed_text(78 * 1000 / 42 + 1), "Elapsed Time: 01:18");
    }
}
