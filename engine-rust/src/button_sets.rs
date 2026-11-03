//! Retail command-card button sets (generated; do not edit by hand).
//!
//! Source: the unit-indexed `{u32 count, u32 buttons, u32 ?}` array of the
//! 32-bit menu dump `research/work/sc_mem_menu_pe.exe` (file offset
//! `0xb2ddf8`, VA = offset + 0x400000; Nexus entry at `0xb2e530` ->
//! buttons at `0xb2bfa8`).  Each button is the 20-byte BW record
//! `{u16 pos, u16 icon, u32 cond, u32 act, u16 cond_var, u16 act_var,
//! u16 enabled_string, u16 disabled_string}`; `cond`/`act` are the retail
//! function addresses, classified in `crate::buttons`.

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

#[allow(clippy::too_many_arguments)]
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
// Retail data removed from the published history (Blizzard data).
pub static BUTTON_SETS: [&[Button]; 0] = [];
