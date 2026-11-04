//! Retail command-card button sets.
//!
//! The data is the unit-indexed `{u32 count, u32 buttons, u32 ?}` array of
//! the 32-bit client (file offset `0xb2ddf8` of a menu-time PE dump, VA =
//! offset + 0x400000).  Each button is the 20-byte BW record `{u16 pos, u16
//! icon, u32 cond, u32 act, u16 cond_var, u16 act_var, u16 enabled_string,
//! u16 disabled_string}`; `cond`/`act` are the retail function addresses,
//! classified in `crate::buttons`.  It is Blizzard data, so the repository
//! does not carry it: `scripts/gen_button_sets.py DUMP` writes the array
//! expression to `fixtures/button_sets.in` (empty when absent).
//!
//! FACT (2026-11-03): the on-disk 32-bit `x86/StarCraft.exe` of
//! 1.23.10.13515 holds the same table as plain `.data` (RVA `0xb2ddf8`, file
//! offset `0xb2c5f8`): the 250-entry array and all 161 button arrays are
//! byte-identical to the dump.  [`parse_exe`] reads it from the player's
//! install, so a build without fixtures (a release package) still has every
//! command card; [`set_of`] prefers that table over the embedded one.

/// One command-card button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Button {
    /// Card position 1..=9 (row-major).
    pub pos: u8,
    /// `cmdicons` frame.
    pub icon: u16,
    pub cond: u32,
    pub act: u32,
    pub cond_var: u16,
    pub act_var: u16,
    /// `stat_txt` ids of the enabled / disabled tooltip (0 = none).
    pub enabled_string: u16,
    pub disabled_string: u16,
}

// Used by the generated data (unused with the empty placeholder).
#[allow(clippy::too_many_arguments, dead_code)]
const fn b(
    pos: u8,
    icon: u16,
    cond: u32,
    act: u32,
    cond_var: u16,
    act_var: u16,
    enabled_string: u16,
    disabled_string: u16,
) -> Button {
    Button {
        pos,
        icon,
        cond,
        act,
        cond_var,
        act_var,
        enabled_string,
        disabled_string,
    }
}

/// Button set of each unit id (0..250); empty slices have no card.
pub static BUTTON_SETS: &[&[Button]] =
    include!(concat!(env!("OUT_DIR"), "/embed/fixtures/button_sets.in"));

/// RVA of the button-set array in the 32-bit 1.23.10.13515 client.
const TABLE_RVA: u32 = 0x00b2_ddf8;
const IMAGE_BASE: u32 = 0x0040_0000;
/// Unit ids with a set (`units.dat` has 228; the array has 250 slots).
const SET_SLOTS: usize = 250;
/// Longest set in the shipped table is far below this; anything larger
/// means the file is not the expected build.
const MAX_BUTTONS: u32 = 64;

/// Section `(rva, raw size, raw offset)` list of a PE32 image based at
/// `0x400000`, or `None` for anything else.
fn pe32_sections(exe: &[u8]) -> Option<Vec<(u32, u32, u32)>> {
    let u16_at = |o: usize| Some(u16::from_le_bytes(exe.get(o..o + 2)?.try_into().ok()?));
    let u32_at = |o: usize| Some(u32::from_le_bytes(exe.get(o..o + 4)?.try_into().ok()?));
    if exe.get(..2)? != b"MZ" {
        return None;
    }
    let pe = u32_at(0x3c)? as usize;
    if exe.get(pe..pe + 4)? != b"PE\0\0" || u16_at(pe + 4)? != 0x14c || u16_at(pe + 24)? != 0x10b {
        return None;
    }
    if u32_at(pe + 24 + 28)? != IMAGE_BASE {
        return None;
    }
    let count = u16_at(pe + 6)? as usize;
    let first = pe + 24 + u16_at(pe + 20)? as usize;
    (0..count)
        .map(|i| {
            let o = first + i * 40;
            Some((u32_at(o + 12)?, u32_at(o + 16)?, u32_at(o + 20)?))
        })
        .collect()
}

/// Every command-card button set read from the 32-bit client executable
/// (`x86/StarCraft.exe`), or `None` when the file is not the expected PE32
/// build or the table does not validate.
pub fn parse_exe(exe: &[u8]) -> Option<Vec<Vec<Button>>> {
    let sections = pe32_sections(exe)?;
    let offset = |rva: u32| -> Option<usize> {
        sections
            .iter()
            .find(|&&(va, raw, _)| rva >= va && rva - va < raw)
            .map(|&(va, _, at)| (at + rva - va) as usize)
    };
    let u16_at = |o: usize| Some(u16::from_le_bytes(exe.get(o..o + 2)?.try_into().ok()?));
    let u32_at = |o: usize| Some(u32::from_le_bytes(exe.get(o..o + 4)?.try_into().ok()?));
    let table = offset(TABLE_RVA)?;
    let mut out = Vec::with_capacity(SET_SLOTS);
    for unit in 0..SET_SLOTS {
        let count = u32_at(table + unit * 12)?;
        if count > MAX_BUTTONS {
            return None;
        }
        let mut set = Vec::with_capacity(count as usize);
        if count > 0 {
            let at = offset(u32_at(table + unit * 12 + 4)?.checked_sub(IMAGE_BASE)?)?;
            for i in 0..count as usize {
                let r = at + i * 20;
                let pos = u16_at(r)?;
                if !(1..=9).contains(&pos) {
                    return None;
                }
                set.push(Button {
                    pos: pos as u8,
                    icon: u16_at(r + 2)?,
                    cond: u32_at(r + 4)?,
                    act: u32_at(r + 8)?,
                    cond_var: u16_at(r + 12)?,
                    act_var: u16_at(r + 14)?,
                    enabled_string: u16_at(r + 16)?,
                    disabled_string: u16_at(r + 18)?,
                });
            }
        }
        out.push(set);
    }
    // The shipped table has 161 non-empty sets.
    (out.iter().filter(|s| !s.is_empty()).count() > 100).then_some(out)
}

static INSTALL_SETS: std::sync::OnceLock<Vec<Vec<Button>>> = std::sync::OnceLock::new();

/// Use the table read from the install (first call wins).
pub fn use_install_table(sets: Vec<Vec<Button>>) {
    let _ = INSTALL_SETS.set(sets);
}

/// Button set of a unit id: the install's table when loaded, else the
/// embedded fixture (empty in builds without fixtures).
pub fn set_of(unit: usize) -> &'static [Button] {
    match INSTALL_SETS.get() {
        Some(sets) => sets.get(unit).map(|s| &s[..]).unwrap_or(&[]),
        None => BUTTON_SETS.get(unit).copied().unwrap_or(&[]),
    }
}

/// Number of unit slots with a known set source.
pub fn slots() -> usize {
    INSTALL_SETS.get().map_or(BUTTON_SETS.len(), Vec::len)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal PE32 with one section holding a two-slot table.
    fn tiny_exe(pos: u16) -> Vec<u8> {
        let mut b = vec![0u8; 0x400];
        b[..2].copy_from_slice(b"MZ");
        b[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        b[0x80..0x84].copy_from_slice(b"PE\0\0");
        b[0x84..0x86].copy_from_slice(&0x14cu16.to_le_bytes());
        b[0x86..0x88].copy_from_slice(&1u16.to_le_bytes());
        b[0x94..0x96].copy_from_slice(&0xe0u16.to_le_bytes());
        b[0x98..0x9a].copy_from_slice(&0x10bu16.to_le_bytes());
        b[0x98 + 28..0x98 + 32].copy_from_slice(&IMAGE_BASE.to_le_bytes());
        // Section at RVA 0xb2d000 (raw 0x200 bytes at file offset 0x200).
        let sec = 0x98 + 0xe0;
        b[sec + 12..sec + 16].copy_from_slice(&0xb2_d000u32.to_le_bytes());
        b[sec + 16..sec + 20].copy_from_slice(&0x2000u32.to_le_bytes());
        b[sec + 20..sec + 24].copy_from_slice(&0x200u32.to_le_bytes());
        b.resize(0x200 + 0x2000, 0);
        let table = 0x200 + (TABLE_RVA - 0xb2_d000) as usize;
        let buttons_rva = 0xb2_d000u32 + 0x10;
        for unit in 0..SET_SLOTS {
            let o = table + unit * 12;
            if unit < 120 {
                b[o..o + 4].copy_from_slice(&1u32.to_le_bytes());
                b[o + 4..o + 8].copy_from_slice(&(IMAGE_BASE + buttons_rva).to_le_bytes());
            }
        }
        let r = 0x200 + 0x10;
        b[r..r + 2].copy_from_slice(&pos.to_le_bytes());
        b[r + 2..r + 4].copy_from_slice(&228u16.to_le_bytes());
        b[r + 4..r + 8].copy_from_slice(&0x65_f590u32.to_le_bytes());
        b
    }

    #[test]
    fn button_table_reads_from_a_pe32_image() {
        let sets = parse_exe(&tiny_exe(1)).expect("valid table");
        assert_eq!(sets.len(), SET_SLOTS);
        assert_eq!(sets[0][0].icon, 228);
        assert_eq!(sets[0][0].cond, 0x65_f590);
        assert!(sets[200].is_empty());
        // A card position outside 1..=9 means another build: rejected.
        assert!(parse_exe(&tiny_exe(12)).is_none());
        assert!(parse_exe(b"MZ not a pe").is_none());
    }
}
