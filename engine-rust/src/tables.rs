//! Additional column-major tables recovered from the shipped CASC data.
//!
//! Layouts follow the executable's loaders (see `docs/EVIDENCE.md`) and were
//! verified against the copies the game has mapped into memory at the
//! addresses recorded there.
//!
//! * `orders.dat`   - 189 orders, 22 bytes each (label + 15 flags/bytes +
//!   2x u16 + 1 byte).
//! * `techdata.dat` - 44 technologies, 4x u16, one u32, 2x u16, u8, u16.
//! * `upgrades.dat` - 61 upgrades, 6x u16 (base/factor pairs), u16, 2x u16,
//!   3x u8.
//! * `sprites.dat`  - 517 sprites (387 selectable + 130 non-selectable).
//! * `images.dat`   - 999 images.
//! * `rez/stat_txt.tbl` - the game's 1,539-entry string table.

use crate::data::DataError;

pub const ORDER_COUNT: usize = 189;
pub const TECH_COUNT: usize = 44;
pub const UPGRADE_COUNT: usize = 61;
pub const SPRITE_COUNT: usize = 517;
pub const SPRITE_SELECTABLE_COUNT: usize = 387;
pub const IMAGE_COUNT: usize = 999;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderType {
    pub label: u16,
    /// The order wants an enemy target.
    pub targets_enemies: bool,
    /// Renders the order's target as a background marker.
    pub background: bool,
    pub valid_for_turret: bool,
    pub can_be_interrupted: bool,
    pub can_be_queued: bool,
    pub can_be_obstructed: bool,
    /// Weapon used when this order is issued as an attack.
    pub weapon: u8,
    /// Tech/ability identifier used by the order.
    pub tech_type: u8,
    pub animation: u8,
    /// Highlight colour/ID used by the UI.
    pub highlight: i16,
    /// Dependency index into the order table (0xffff for none).
    pub dep_index: u16,
    /// Order to fall back to when this one finishes (0xff for none).
    pub target_order: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TechType {
    pub mineral_cost: u16,
    pub gas_cost: u16,
    /// Research time in the table's native unit.
    pub research_time: u16,
    pub energy_cost: u16,
    pub unknown: u32,
    pub icon: u16,
    /// Index into `stat_txt.tbl`.
    pub label: u16,
    /// 0 = Zerg, 1 = Terran, 2 = Protoss, 3 = independent.
    pub race: u8,
    pub flags: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpgradeType {
    pub mineral_cost_base: u16,
    pub mineral_cost_factor: u16,
    pub gas_cost_base: u16,
    pub gas_cost_factor: u16,
    pub time_cost_base: u16,
    pub time_cost_factor: u16,
    pub unknown: u16,
    pub icon: u16,
    pub label: u16,
    pub race: u8,
    pub max_level: u8,
    pub is_broodwar: bool,
}

impl UpgradeType {
    /// Cost of the next level (1-based).  The game stores base + factor*level.
    pub fn mineral_cost(&self, level: u8) -> u32 {
        self.mineral_cost_base as u32 + self.mineral_cost_factor as u32 * level as u32
    }

    pub fn gas_cost(&self, level: u8) -> u32 {
        self.gas_cost_base as u32 + self.gas_cost_factor as u32 * level as u32
    }

    pub fn time_cost(&self, level: u8) -> u32 {
        self.time_cost_base as u32 + self.time_cost_factor as u32 * level as u32
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpriteType {
    pub image: u16,
    pub health_bar_size: u8,
    pub unknown: u8,
    pub visible: bool,
    pub selection_circle: u8,
    pub selection_circle_vpos: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageType {
    pub grp_filename_index: u32,
    pub has_directional_frames: bool,
    pub is_clickable: bool,
    pub has_iscript_animations: bool,
    pub always_visible: bool,
    pub modifier: u8,
    pub color_shift: u8,
    pub iscript_id: u32,
    pub shield_filename_index: u32,
    pub attack_filename_index: u32,
    pub damage_filename_index: u32,
    pub special_filename_index: u32,
    pub landing_dust_filename_index: u32,
    pub lift_off_filename_index: u32,
}

/// The `stat_txt.tbl` string table.  Entry 0 is a format-version sentinel in
/// some releases; the game's text IDs are used directly as indices.
#[derive(Clone, Debug, Default)]
pub struct StringTable {
    strings: Vec<String>,
}

impl StringTable {
    pub fn parse(d: &[u8]) -> Result<Self, DataError> {
        if d.len() < 2 {
            return Err(DataError::ShortFile {
                which: "stat_txt.tbl",
                expected: 2,
                actual: d.len(),
            });
        }
        let count = u16::from_le_bytes([d[0], d[1]]) as usize;
        let header = 2 + count * 2;
        if d.len() < header {
            return Err(DataError::ShortFile {
                which: "stat_txt.tbl",
                expected: header,
                actual: d.len(),
            });
        }
        let mut offsets = Vec::with_capacity(count);
        for i in 0..count {
            offsets.push(u16::from_le_bytes([d[2 + i * 2], d[3 + i * 2]]) as usize);
        }
        let mut strings = Vec::with_capacity(count);
        for i in 0..count {
            let start = offsets[i];
            let end = offsets.get(i + 1).copied().unwrap_or(d.len());
            if start > d.len() || end > d.len() || start > end {
                strings.push(String::new());
                continue;
            }
            let bytes = &d[start..end];
            let nul = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
            strings.push(String::from_utf8_lossy(&bytes[..nul]).into_owned());
        }
        Ok(Self { strings })
    }

    pub fn len(&self) -> usize {
        self.strings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.strings.is_empty()
    }

    pub fn get(&self, id: u16) -> Option<&str> {
        self.strings.get(id as usize).map(String::as_str)
    }

    pub fn text(&self, id: u16) -> &str {
        self.get(id).unwrap_or("")
    }

    /// Strip StarCraft's colour/format control bytes from a display string.
    pub fn display(&self, id: u16) -> String {
        let mut out = String::new();
        for ch in self.text(id).chars() {
            if ch.is_control() {
                continue;
            }
            out.push(ch);
        }
        out
    }
}

pub fn parse_orders(d: &[u8]) -> Result<Vec<OrderType>, DataError> {
    let expected = ORDER_COUNT * 22;
    if d.len() < expected {
        return Err(DataError::ShortFile {
            which: "orders.dat",
            expected,
            actual: d.len(),
        });
    }
    // Column layout, in bytes from the start of the file.
    let labels = 0;
    let flags = ORDER_COUNT * 2;
    let width = ORDER_COUNT;
    let field = |index: usize, i: usize| d[flags + index * width + i] != 0;
    let raw = |index: usize, i: usize| d[flags + index * width + i];
    let highlight = ORDER_COUNT * 2 + 15 * ORDER_COUNT;
    let dep_index = highlight + ORDER_COUNT * 2;
    let target_order = dep_index + ORDER_COUNT * 2;

    let mut orders = Vec::with_capacity(ORDER_COUNT);
    for i in 0..ORDER_COUNT {
        orders.push(OrderType {
            label: u16::from_le_bytes([d[labels + i * 2], d[labels + i * 2 + 1]]),
            targets_enemies: field(0, i),
            background: field(1, i),
            valid_for_turret: field(3, i),
            can_be_interrupted: field(5, i),
            can_be_queued: field(7, i),
            can_be_obstructed: field(9, i),
            weapon: raw(12, i),
            tech_type: raw(13, i),
            animation: raw(14, i),
            highlight: i16::from_le_bytes([d[highlight + i * 2], d[highlight + i * 2 + 1]]),
            dep_index: u16::from_le_bytes([d[dep_index + i * 2], d[dep_index + i * 2 + 1]]),
            target_order: d[target_order + i],
        });
    }
    Ok(orders)
}

pub fn parse_techs(d: &[u8]) -> Result<Vec<TechType>, DataError> {
    let expected = TECH_COUNT * 19;
    if d.len() < expected {
        return Err(DataError::ShortFile {
            which: "techdata.dat",
            expected,
            actual: d.len(),
        });
    }
    let u16_at = |base: usize, i: usize| u16::from_le_bytes([d[base + i * 2], d[base + i * 2 + 1]]);
    let mineral = 0;
    let gas = mineral + TECH_COUNT * 2;
    let time = gas + TECH_COUNT * 2;
    let energy = time + TECH_COUNT * 2;
    let unknown = energy + TECH_COUNT * 2;
    let icon = unknown + TECH_COUNT * 4;
    let label = icon + TECH_COUNT * 2;
    let race = label + TECH_COUNT * 2;
    let flags = race + TECH_COUNT;
    let mut techs = Vec::with_capacity(TECH_COUNT);
    for i in 0..TECH_COUNT {
        let o = unknown + i * 4;
        techs.push(TechType {
            mineral_cost: u16_at(mineral, i),
            gas_cost: u16_at(gas, i),
            research_time: u16_at(time, i),
            energy_cost: u16_at(energy, i),
            unknown: u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]]),
            icon: u16_at(icon, i),
            label: u16_at(label, i),
            race: d[race + i],
            flags: u16_at(flags, i),
        });
    }
    Ok(techs)
}

pub fn parse_upgrades(d: &[u8]) -> Result<Vec<UpgradeType>, DataError> {
    let expected = UPGRADE_COUNT * 21;
    if d.len() < expected {
        return Err(DataError::ShortFile {
            which: "upgrades.dat",
            expected,
            actual: d.len(),
        });
    }
    let u16_at = |base: usize, i: usize| u16::from_le_bytes([d[base + i * 2], d[base + i * 2 + 1]]);
    let mut bases = [0usize; 9];
    bases[0] = 0;
    for i in 1..6 {
        bases[i] = bases[i - 1] + UPGRADE_COUNT * 2;
    }
    bases[6] = bases[5] + UPGRADE_COUNT * 2; // unknown u16
    bases[7] = bases[6] + UPGRADE_COUNT * 2; // icon
    bases[8] = bases[7] + UPGRADE_COUNT * 2; // label
    let race = bases[8] + UPGRADE_COUNT * 2;
    let max_level = race + UPGRADE_COUNT;
    let broodwar = max_level + UPGRADE_COUNT;

    let mut upgrades = Vec::with_capacity(UPGRADE_COUNT);
    for i in 0..UPGRADE_COUNT {
        upgrades.push(UpgradeType {
            mineral_cost_base: u16_at(bases[0], i),
            mineral_cost_factor: u16_at(bases[1], i),
            gas_cost_base: u16_at(bases[2], i),
            gas_cost_factor: u16_at(bases[3], i),
            time_cost_base: u16_at(bases[4], i),
            time_cost_factor: u16_at(bases[5], i),
            unknown: u16_at(bases[6], i),
            icon: u16_at(bases[7], i),
            label: u16_at(bases[8], i),
            race: d[race + i],
            max_level: d[max_level + i],
            is_broodwar: d[broodwar + i] != 0,
        });
    }
    Ok(upgrades)
}

pub fn parse_sprites(d: &[u8]) -> Result<Vec<SpriteType>, DataError> {
    let expected = SPRITE_COUNT * 2
        + SPRITE_SELECTABLE_COUNT
        + SPRITE_COUNT
        + SPRITE_COUNT
        + SPRITE_SELECTABLE_COUNT * 2;
    if d.len() < expected {
        return Err(DataError::ShortFile {
            which: "sprites.dat",
            expected,
            actual: d.len(),
        });
    }
    let image = 0;
    let health_bar = image + SPRITE_COUNT * 2;
    let unknown = health_bar + SPRITE_SELECTABLE_COUNT;
    let visible = unknown + SPRITE_COUNT;
    let selection = visible + SPRITE_COUNT;
    let selection_vpos = selection + SPRITE_SELECTABLE_COUNT;
    let mut sprites = Vec::with_capacity(SPRITE_COUNT);
    for i in 0..SPRITE_COUNT {
        let selectable = i < SPRITE_SELECTABLE_COUNT;
        sprites.push(SpriteType {
            image: u16::from_le_bytes([d[image + i * 2], d[image + i * 2 + 1]]),
            health_bar_size: if selectable { d[health_bar + i] } else { 0 },
            unknown: d[unknown + i],
            visible: d[visible + i] != 0,
            selection_circle: if selectable { d[selection + i] } else { 0 },
            selection_circle_vpos: if selectable { d[selection_vpos + i] } else { 0 },
        });
    }
    Ok(sprites)
}

pub fn parse_images(d: &[u8]) -> Result<Vec<ImageType>, DataError> {
    let expected = IMAGE_COUNT * 38;
    if d.len() < expected {
        return Err(DataError::ShortFile {
            which: "images.dat",
            expected,
            actual: d.len(),
        });
    }
    let u32_at = |base: usize, i: usize| {
        let o = base + i * 4;
        u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
    };
    let grp = 0;
    let directional = grp + IMAGE_COUNT * 4;
    let clickable = directional + IMAGE_COUNT;
    let iscript_anims = clickable + IMAGE_COUNT;
    let always_visible = iscript_anims + IMAGE_COUNT;
    let modifier = always_visible + IMAGE_COUNT;
    let color_shift = modifier + IMAGE_COUNT;
    let iscript = color_shift + IMAGE_COUNT;
    let shield = iscript + IMAGE_COUNT * 4;
    let attack = shield + IMAGE_COUNT * 4;
    let damage = attack + IMAGE_COUNT * 4;
    let special = damage + IMAGE_COUNT * 4;
    let landing = special + IMAGE_COUNT * 4;
    let lift_off = landing + IMAGE_COUNT * 4;

    let mut images = Vec::with_capacity(IMAGE_COUNT);
    for i in 0..IMAGE_COUNT {
        images.push(ImageType {
            grp_filename_index: u32_at(grp, i),
            has_directional_frames: d[directional + i] != 0,
            is_clickable: d[clickable + i] != 0,
            has_iscript_animations: d[iscript_anims + i] != 0,
            always_visible: d[always_visible + i] != 0,
            modifier: d[modifier + i],
            color_shift: d[color_shift + i],
            iscript_id: u32_at(iscript, i),
            shield_filename_index: u32_at(shield, i),
            attack_filename_index: u32_at(attack, i),
            damage_filename_index: u32_at(damage, i),
            special_filename_index: u32_at(special, i),
            landing_dust_filename_index: u32_at(landing, i),
            lift_off_filename_index: u32_at(lift_off, i),
        });
    }
    Ok(images)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_names_are_recovered_from_the_string_table() {
        let orders = parse_orders(include_bytes!("../fixtures/orders.dat")).unwrap();
        let text = StringTable::parse(include_bytes!("../fixtures/stat_txt.tbl")).unwrap();
        // Order 0 is Fizzle (label 903), 7 is Attack (label 913), 10 is
        // Attack Fixed Range (label 915), 15 is Build (label 934).
        assert_eq!(text.display(orders[0].label), "Fizzle");
        assert_eq!(text.display(orders[7].label), "Attack");
        assert_eq!(text.display(orders[10].label), "Attack Fixed Range");
        assert_eq!(text.display(orders[15].label), "Build");
    }

    #[test]
    fn tech_costs_match_the_shipped_table() {
        let techs = parse_techs(include_bytes!("../fixtures/techdata.dat")).unwrap();
        // Stim Packs: 100/100, 1200 research time, Terran race.
        assert_eq!(techs[0].mineral_cost, 100);
        assert_eq!(techs[0].gas_cost, 100);
        assert_eq!(techs[0].research_time, 1200);
        assert_eq!(techs[0].race, 1);
        assert_eq!(techs[0].label, 313);
    }

    #[test]
    fn upgrade_levels_scale_base_and_factor() {
        let ups = parse_upgrades(include_bytes!("../fixtures/upgrades.dat")).unwrap();
        // Infantry Weapons: 100 + 75*level minerals, 100 + 75*level gas.
        assert_eq!(ups[0].mineral_cost(1), 175);
        assert_eq!(ups[0].mineral_cost(2), 250);
        assert_eq!(ups[0].gas_cost(3), 325);
        assert_eq!(ups[0].max_level, 3);
        assert_eq!(ups[0].race, 1);
    }

    #[test]
    fn sprite_and_image_counts() {
        let sprites = parse_sprites(include_bytes!("../fixtures/sprites.dat")).unwrap();
        let images = parse_images(include_bytes!("../fixtures/images.dat")).unwrap();
        assert_eq!(sprites.len(), SPRITE_COUNT);
        assert_eq!(images.len(), IMAGE_COUNT);
    }
}
